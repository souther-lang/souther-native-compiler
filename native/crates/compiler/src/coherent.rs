//! What a document has to be before anything in it is lowered: every relation the checker held
//! between two of its statements, held here again.
//!
//! The lowering reads a node's own type as the type of the value it lowers that node to. It picks a
//! width from it, an instruction, the arm of a fork, the slot a field is read at. But the value is
//! not made from that type. It is made from something else the document states — the binder a read
//! reads, the signature a call reaches, the declaration a construction builds, the member a tuple
//! holds — and upstream the checker built the two to agree. On the wire they are two fields, and a
//! document where they disagree lowers to a function that writes one type where its caller reads
//! another. So the agreement is established here, once, for every node of every body, and the
//! lowering after this reads a node's type as what its value is.
//!
//! Nothing here works a type out. Each rule compares what a node says it is with what the document
//! says about where its value comes from, under the one relation the checker holds between the
//! two: the same type, where the checker copies one into the other (a read and its binder, a call
//! and the behavior it reaches), or a value of it, where the checker lets a narrower value stand
//! (a branch and the fork it answers, an argument and the parameter it is handed to). Where the
//! wire carried the same fact twice for no reason, it no longer does (transport 10): a helper's and
//! a value's answer are their body's type, a helper's parameters are named and typed together, and
//! a composition's answers are its targets'. What a let binds its name at is carried because it is
//! the one thing a read of it can be held to.
//!
//! Three passes, in an order that decides what a refusal says:
//!
//! - what is the same fact stated twice, and what cannot be looked up, is refused first, as the two
//!   halves disagreeing, without asking anything of a type's layout;
//! - then whether each value a relation asks about is a value of the type it stands in, which reads
//!   the declarations and nothing else;
//! - and only after every body has passed both, whatever this backend has no representation for is
//!   refused as not lowered.
//!
//! So a document that disagrees anywhere is never reported as a program this backend is behind on,
//! whichever of its bodies happened to be read first.
//!
//! What is held is what the checker's own objects hold and the document still states both sides
//! of: what a module owns (`CheckedModule`: its helpers, its values, entries for those values, the
//! behaviors it declares), that every name a type or a reach writes is one the document carries,
//! that a name can stand in a symbol, what every operator node answers whatever it is written
//! over, and every relation between a node's type and what its value is made from. What is not
//! held, because the document does not carry the checker's side of it:
//!
//! - which pairs an operator may be written over, and what it makes of two different ones
//!   (souther-lang/souther#1919). A pair the checker would refuse and one this backend has no
//!   lowering for are both refused as not lowered;
//! - where a value is let stand as a wider type. That is asked of [`Declared::fits`], which copies
//!   the part of the checker's rule for the types laid out here (souther-lang/souther#1916);
//! - what the writer drops: which values a module publishes beyond the entries it has, what a row
//!   expects, what a kernel call settled.

use crate::closures::ClosureSites;
use crate::index;
use crate::transport::{
    AbortKind, Answers, Case, Definition, Held, Node, Op, Owner, Prim, Program, Reaches, Routing,
    Selects, Target, Ty, Value,
};
use crate::{Declared, Targets, not_lowered, spelt};
use anyhow::{Result, anyhow, bail};
use souther_native_abi::{spells_a_module, spells_a_name};
use std::collections::HashMap;

/// A document every relation of which holds, and what reading it built.
pub(crate) struct Coherent<'a> {
    pub declared: Declared<'a>,
    pub targets: Targets<'a>,
    /// Every local definition this object holds, by the name it defines.
    pub locals: HashMap<&'a str, &'a Definition>,
    /// Every closure site the document holds, found once over the whole program.
    pub closures: ClosureSites<'a>,
}

impl<'a> Coherent<'a> {
    pub fn of(program: &'a Program) -> Result<Self> {
        let declared = Declared::of(&program.declarations)?;
        let targets = Targets::of(&program.behaviors)?;
        for target in &program.behaviors {
            if let crate::transport::BoundaryOutput::Cases { ty, cases, form } = &target.output {
                declared.settled(&target.declared(), cases, form)?;
                declared.descends_to(&target.declared(), ty, cases)?;
            }
        }
        let closures = ClosureSites::of_program(program)?;

        let reached = Reached::of(program)?;

        let mut locals: HashMap<&str, &Definition> = HashMap::new();
        for written in &program.modules {
            for definition in &written.definitions {
                let name = definition.declared();
                // A module defines its own behaviors and no other module's.
                let target = targets.named(name)?;
                if target.module != written.name {
                    bail!(
                        "{} defines {name}, which {} declares: a module defines the behaviors it \
                         declares",
                        written.name,
                        target.module
                    );
                }
                index::once(&mut locals, name, definition, || {
                    format!("two local definitions are both written {name}")
                })?;
            }
        }
        for target in &program.behaviors {
            let name = target.declared();
            if !spells_a_module(&target.module) || !spells_a_name(&target.name) {
                bail!("a behavior is written {name}, which no symbol can carry");
            }
            for ty in target.takes().iter().chain([&target.answers()]) {
                declared.resolves(&name, ty)?;
            }
            // A name defined here from both sides: what a target says a local definition is
            // (below, exhaustively over the local definitions), and that a target saying it is
            // defined here has one at all.
            if matches!(target.is, Answers::Body | Answers::Composed)
                && !locals.contains_key(name.as_str())
            {
                bail!("{name} answers with a local definition no module of this document carries");
            }
            // Implemented by another build, which is a module this document does not build.
            if target.is == Answers::Elsewhere
                && reached.modules.contains_key(target.module.as_str())
            {
                bail!(
                    "{name} is implemented by another build, and its module {} is one this \
                     document builds",
                    target.module
                );
            }
        }

        let mut owed = Owed::default();
        for (&name, &local) in &locals {
            let target = targets.named(name)?;
            agrees_with_its_target(name, target, local, &targets, &declared, &mut owed)?;
        }

        for body in program.bodies() {
            let (owner, takes) = match body.owner {
                Owner::Helper(held) => (held.declared.clone(), held.takes()),
                Owner::Value(value) => (
                    value.declared(),
                    value.handovers.iter().map(|it| it.ty.clone()).collect(),
                ),
                Owner::Entry(entry) => (
                    format!("the entry for {}", entry.value.declared()),
                    Vec::new(),
                ),
                Owner::Definition(declared) => {
                    (declared.to_string(), targets.named(declared)?.takes())
                }
                Owner::Example(example) => {
                    let behavior = format!("{}.{}", body.module, example.behavior);
                    let owner = format!("row {} of {behavior}", example.at);
                    owed.fits(
                        format!("{owner} answers what its behavior answers"),
                        body.node.ty(),
                        &targets.named(&behavior)?.answers(),
                    );
                    (owner, Vec::new())
                }
            };
            Walk {
                owner,
                carrier: body.module,
                targets: &targets,
                declared: &declared,
                reached: &reached,
                bound: HashMap::new(),
                owed: &mut owed,
            }
            .under(takes.into_iter().enumerate().collect(), body.node)?;
        }

        owed.settle(&declared)?;

        Ok(Coherent {
            declared,
            targets,
            locals,
            closures,
        })
    }
}

/// What the passes after the first still have to ask, collected as the first finds them.
#[derive(Default)]
struct Owed {
    /// Relations under which a value only has to be a value of the type it stands in.
    fits: Vec<Owing>,
    /// What this backend has no lowering for, found while reading and refused only once nothing
    /// in the document disagrees.
    not_lowered: Vec<String>,
    /// What a call of another build's published value stands at, by the value, as the first call
    /// read says it: one declaration answers one way, so every other call is held to this.
    published: HashMap<(String, String), Ty>,
}

struct Owing {
    what: String,
    actual: Ty,
    expected: Ty,
}

impl Owed {
    fn fits(&mut self, what: String, actual: &Ty, expected: &Ty) {
        self.fits.push(Owing {
            what,
            actual: actual.clone(),
            expected: expected.clone(),
        });
    }

    /// The second pass and the third: every relation that is a question of values, then what
    /// could not be asked because this backend lays no value of the types out.
    fn settle(self, declared: &Declared) -> Result<()> {
        let mut undecided = Vec::new();
        for owing in &self.fits {
            match declared.fits(&owing.actual, &owing.expected)? {
                Some(true) => {}
                Some(false) => bail!(
                    "{}: {} is not a value of {}, and the checker holds it to be one: the two \
                     halves disagree",
                    owing.what,
                    owing.actual.spelt(),
                    owing.expected.spelt()
                ),
                None => undecided.push(format!(
                    "whether a value of {} is one of {}",
                    owing.actual.spelt(),
                    owing.expected.spelt()
                )),
            }
        }
        if let Some(first) = self.not_lowered.into_iter().chain(undecided).next() {
            return Err(not_lowered(first).into());
        }
        Ok(())
    }
}

/// Every definition a call can reach by name, by the key the call writes it under, and what each
/// module of the document owns.
struct Reached<'a> {
    /// The modules this document builds, by name.
    modules: HashMap<&'a str, ()>,
    /// A helper, by the module holding the copy and the name it was declared under.
    helpers: HashMap<(&'a str, &'a str), &'a Held>,
    /// A value's home, by the module it runs in and its joined name.
    values: HashMap<(&'a str, String), &'a Value>,
    /// A published value's entry body, by the value's module and name, where this document
    /// carries the module that publishes it.
    entries: HashMap<(&'a str, &'a str), &'a Node>,
}

impl<'a> Reached<'a> {
    /// What each module owns, as `CheckedModule` holds it to: a helper it holds once; a value it
    /// declares, once, and not also as a helper; an entry for a value it builds, once; and a row
    /// once at each place. Each is reached by the name it is written under and by the module that
    /// owns it, so a second one under the name, or one under a module that does not own it, would
    /// be compiled where the checker put nothing.
    fn of(program: &'a Program) -> Result<Self> {
        let mut reached = Reached {
            modules: HashMap::new(),
            helpers: HashMap::new(),
            values: HashMap::new(),
            entries: HashMap::new(),
        };
        let mut rows = HashMap::new();
        for written in &program.modules {
            let module = written.name.as_str();
            if !spells_a_module(module) {
                bail!("a module is written {module}, which no symbol can carry");
            }
            index::once(&mut reached.modules, module, (), || {
                format!("two modules are both written {module}")
            })?;
            for held in &written.helpers {
                index::once(
                    &mut reached.helpers,
                    (module, held.declared.as_str()),
                    held,
                    || format!("{module} holds two helpers both written {}", held.declared),
                )?;
            }
            for value in &written.values {
                let declared = value.declared();
                // A value runs in the module that declares it and in no other (ADR-0074).
                if value.module != module {
                    bail!(
                        "{module} builds the value {declared}, which {} declares: a value runs in \
                         the module that declares it",
                        value.module
                    );
                }
                if !spells_a_name(&value.name) {
                    bail!("a value is written {declared}, which no symbol can carry");
                }
                if reached.helpers.contains_key(&(module, declared.as_str())) {
                    bail!("{module} holds {declared} both as a helper and as a value");
                }
                index::once(
                    &mut reached.values,
                    (module, declared.clone()),
                    value,
                    || format!("{module} builds two values both written {declared}"),
                )?;
            }
            for entry in &written.entries {
                let declared = entry.value.declared();
                if !reached.values.contains_key(&(module, declared.clone())) {
                    bail!(
                        "{module} publishes an entry for {declared}, which is not a value it \
                         builds"
                    );
                }
                index::once(
                    &mut reached.entries,
                    (entry.value.module.as_str(), entry.value.name.as_str()),
                    &entry.body,
                    || format!("two entries are both written for {declared}"),
                )?;
            }
            for example in &written.examples {
                index::once(
                    &mut rows,
                    (module, example.behavior.as_str(), example.at),
                    (),
                    || {
                        format!(
                            "{module}.{} has two rows both written at {}",
                            example.behavior, example.at
                        )
                    },
                )?;
            }
        }
        Ok(reached)
    }
}

/// One body read with what is bound where it stands.
struct Walk<'w, 'a> {
    /// Whose body this is, which every refusal names.
    owner: String,
    /// The module whose copy of a helper a call from here reaches.
    carrier: &'a str,
    targets: &'w Targets<'a>,
    declared: &'w Declared<'a>,
    reached: &'w Reached<'a>,
    /// What each binding in scope is in force at, as the node that made it says.
    bound: HashMap<usize, Ty>,
    owed: &'w mut Owed,
}

impl<'a> Walk<'_, 'a> {
    /// Refuses a node whose own type is not the one its source states.
    fn same(&self, what: &str, stated: &Ty, source: &Ty, source_is: &str) -> Result<()> {
        if stated != source {
            bail!(
                "{}: {what} is typed {} and {source_is} is {}: the two are one fact crossed twice \
                 and this document's disagree",
                self.owner,
                stated.spelt(),
                source.spelt()
            );
        }
        Ok(())
    }

    fn fits(&mut self, what: &str, actual: &Ty, expected: &Ty) {
        let what = format!("{}: {what}", self.owner);
        self.owed.fits(what, actual, expected);
    }

    fn not_lowered(&mut self, what: String) {
        self.owed.not_lowered.push(what);
    }

    fn arity(&self, what: &str, given: usize, taken: usize) -> Result<()> {
        if given != taken {
            bail!(
                "{}: {what} is handed {given} values and takes {taken}: the two halves disagree",
                self.owner
            );
        }
        Ok(())
    }

    /// `node` read with each of `bindings` in force at its type, and whatever they shadowed put
    /// back afterwards.
    fn under(&mut self, bindings: Vec<(usize, Ty)>, node: &'a Node) -> Result<()> {
        for (binding, ty) in &bindings {
            self.declared
                .resolves(&format!("{}: binding {binding}", self.owner), ty)?;
        }
        let shadowed: Vec<(usize, Option<Ty>)> = bindings
            .into_iter()
            .map(|(binding, ty)| (binding, self.scope(binding, Some(ty))))
            .collect();
        let read = self.node(node);
        for (binding, before) in shadowed.into_iter().rev() {
            self.scope(binding, before);
        }
        read
    }

    /// `binding` in force at `ty`, or at nothing, answering what it was in force at before.
    #[expect(
        clippy::disallowed_methods,
        reason = "a binder shadows whatever an enclosing one bound under its number, on purpose, \
                  and `under` puts it back when the scope closes"
    )]
    fn scope(&mut self, binding: usize, ty: Option<Ty>) -> Option<Ty> {
        match ty {
            Some(ty) => self.bound.insert(binding, ty),
            None => self.bound.remove(&binding),
        }
    }

    /// No arm standing for the rest: a node added upstream is a node whose value this has not
    /// yet said the source of, and the lowering would read its type on trust.
    fn node(&mut self, node: &'a Node) -> Result<()> {
        self.declared.resolves(&self.owner, node.ty())?;
        let bool_ = Ty::Prim { prim: Prim::Bool };
        match node {
            Node::Int { ty, .. } => self.same(
                "an integer literal",
                ty,
                &Ty::Prim { prim: Prim::Int },
                "its kind",
            ),
            Node::Bool { ty, .. } => self.same("a truth literal", ty, &bool_, "its kind"),
            Node::Str { ty, .. } => self.same(
                "a text literal",
                ty,
                &Ty::Prim { prim: Prim::String },
                "its kind",
            ),
            Node::Read { binding, ty, .. } => {
                let bound = self.bound.get(binding).ok_or_else(|| {
                    anyhow!(
                        "{}: a read of binding {binding}, which nothing in scope binds",
                        self.owner
                    )
                })?;
                self.same(
                    &format!("a read of binding {binding}"),
                    ty,
                    bound,
                    "the binder",
                )
            }
            Node::Unit { declared, ty, .. } => {
                if !matches!(
                    self.declared.shape(declared)?,
                    crate::transport::Declaration::Unit { .. }
                ) {
                    bail!(
                        "{}: {declared} is written as a unit's value and is not declared a unit",
                        self.owner
                    );
                }
                self.same(
                    &format!("the unit {declared}"),
                    ty,
                    &Ty::Declared {
                        declared: declared.clone(),
                    },
                    "what it names",
                )
            }
            Node::Construct {
                declared,
                values,
                ty,
                ..
            } => {
                self.same(
                    &format!("a construction of {declared}"),
                    ty,
                    &Ty::Declared {
                        declared: declared.clone(),
                    },
                    "what it builds",
                )?;
                let shape = self.declared.shape(declared)?;
                if !matches!(
                    shape,
                    crate::transport::Declaration::Product { .. }
                        | crate::transport::Declaration::Newtype { .. }
                ) {
                    bail!(
                        "{}: {declared} is constructed and is not declared with fields to build",
                        self.owner
                    );
                }
                if shape.field_count() != values.len() {
                    bail!(
                        "{}: {declared} is declared with {} fields and is built here from {}",
                        self.owner,
                        shape.field_count(),
                        values.len()
                    );
                }
                for (field, value) in shape.fields().iter().zip(values) {
                    self.node(value)?;
                    self.fits(
                        &format!("{declared}'s field {} is given a value", field.name),
                        value.ty(),
                        &field.codec.ty(),
                    );
                }
                Ok(())
            }
            Node::Field {
                target, field, ty, ..
            } => {
                self.node(target)?;
                let of = target.ty();
                let Ty::Declared { declared } = of else {
                    bail!(
                        "{}: a field of {}, which holds no fields",
                        self.owner,
                        of.spelt()
                    );
                };
                let shape = self.declared.shape(declared)?;
                if let crate::transport::Declaration::Sum { .. } = shape {
                    self.not_lowered(format!("a field {field} read off the sum {declared}"));
                    return Ok(());
                }
                let at = shape.position_of(field).ok_or_else(|| {
                    anyhow!("{}: {declared} declares no field {field}", self.owner)
                })?;
                self.same(
                    &format!("a read of {declared}'s field {field}"),
                    ty,
                    &shape.fields()[at].codec.ty(),
                    "the field",
                )
            }
            Node::Binary {
                op,
                left,
                right,
                ty,
                aborts,
            } => {
                self.node(left)?;
                self.node(right)?;
                self.operator(*op, left.ty(), right.ty(), ty, aborts)
            }
            Node::Neg {
                operand,
                ty,
                aborts,
            } => {
                self.node(operand)?;
                self.same("a negation", ty, operand.ty(), "what it negates")?;
                self.number("a negation", ty)?;
                // A literal's sign is folded, and nothing else about one can leave the range.
                if !matches!(operand.as_ref(), Node::Int { .. }) {
                    if aborts.is_empty() {
                        // souther-lang/souther#1878: the checker answers no reason for a negation,
                        // and this backend does not answer one on its behalf.
                        self.not_lowered(
                            "a negation of something other than a literal, whose overflow this \
                             backend does not yet trust program.abortsAt for — see \
                             souther-lang/souther#1878"
                                .to_string(),
                        );
                    } else {
                        self.overflows("a negation", aborts)?;
                    }
                }
                Ok(())
            }
            Node::Let {
                binding,
                binds,
                value,
                body,
                ty,
                ..
            } => {
                // Read before the binding is in force: a binding is not read in its own value.
                self.node(value)?;
                self.fits(
                    &format!("the value binding {binding} is given"),
                    value.ty(),
                    binds,
                );
                self.under(vec![(*binding, binds.clone())], body)?;
                self.fits("what a let answers", body.ty(), ty);
                Ok(())
            }
            Node::If {
                cond,
                then,
                els,
                ty,
                ..
            } => {
                self.node(cond)?;
                self.same("what a fork asks", cond.ty(), &bool_, "a truth")?;
                self.node(then)?;
                self.node(els)?;
                self.fits("what a fork's branch answers", then.ty(), ty);
                self.fits("what a fork's branch answers", els.ty(), ty);
                Ok(())
            }
            Node::Match {
                subject, arms, ty, ..
            } => {
                self.node(subject)?;
                for arm in arms {
                    if arm.selects.is_empty() {
                        bail!("{}: an arm tests for nothing", self.owner);
                    }
                    for selects in &arm.selects {
                        if let Selects::Which { atoms } = selects {
                            self.leaves("an arm", atoms)?;
                        }
                    }
                    match (arm.binding, &arm.binds) {
                        (None, _) => self.node(&arm.body)?,
                        (Some(_), None) => bail!(
                            "{}: an arm binds a value and does not say what it reads it as",
                            self.owner
                        ),
                        (Some(binding), Some(binds)) => {
                            self.arm_binds(subject.ty(), &arm.selects, binds)?;
                            self.under(vec![(binding, binds.clone())], &arm.body)?;
                        }
                    }
                    self.fits("what a match arm answers", arm.body.ty(), ty);
                }
                Ok(())
            }
            Node::Some { value, ty, .. } => {
                self.node(value)?;
                let Ty::Option { option } = ty else {
                    bail!(
                        "{}: a present value typed {}, which is not optional",
                        self.owner,
                        ty.spelt()
                    );
                };
                self.fits("what a present value holds", value.ty(), option);
                Ok(())
            }
            Node::None { ty, .. } => match ty {
                Ty::Option { .. } => Ok(()),
                _ => bail!(
                    "{}: an absent value typed {}, which is not optional",
                    self.owner,
                    ty.spelt()
                ),
            },
            Node::Tuple { members, ty, .. } => {
                for member in members {
                    self.node(member)?;
                }
                let made = Ty::Tuple {
                    tuple: members.iter().map(|it| it.ty().clone()).collect(),
                };
                self.same("a tuple", ty, &made, "what its members are")
            }
            Node::Member { tuple, at, ty, .. } => {
                self.node(tuple)?;
                let Ty::Tuple { tuple: members } = tuple.ty() else {
                    bail!(
                        "{}: member {at} of {}, which is not a tuple",
                        self.owner,
                        tuple.ty().spelt()
                    );
                };
                let member = members.get(*at).ok_or_else(|| {
                    anyhow!(
                        "{}: member {at} of {}, which has {}",
                        self.owner,
                        tuple.ty().spelt(),
                        members.len()
                    )
                })?;
                self.same(&format!("member {at}"), ty, member, "the tuple's")
            }
            Node::Call {
                reaches,
                arguments,
                ty,
                aborts,
            } => {
                for argument in arguments {
                    self.node(argument)?;
                }
                self.call(reaches, arguments, ty, aborts)
            }
            Node::Block {
                site,
                parameters,
                body,
                ty,
                ..
            } => {
                let Ty::Fn { fn_ } = ty else {
                    bail!(
                        "{}: closure site {site} is typed {}, which is not a function type",
                        self.owner,
                        ty.spelt()
                    );
                };
                self.arity(
                    &format!("closure site {site}"),
                    parameters.len(),
                    fn_.takes.len(),
                )?;
                let bound = parameters
                    .iter()
                    .zip(&fn_.takes)
                    .map(|(parameter, taken)| (parameter.binding, taken.clone()))
                    .collect();
                self.under(bound, body)?;
                self.same(
                    &format!("what closure site {site} answers"),
                    &fn_.answers,
                    body.ty(),
                    "its body",
                )
            }
            Node::Apply {
                function,
                arguments,
                ty,
                ..
            } => {
                self.node(function)?;
                for argument in arguments {
                    self.node(argument)?;
                }
                let Ty::Fn { fn_ } = function.ty() else {
                    bail!(
                        "{}: an application of {}, which is not a function type",
                        self.owner,
                        function.ty().spelt()
                    );
                };
                self.arity("an application", arguments.len(), fn_.takes.len())?;
                self.same(
                    "an application",
                    ty,
                    &fn_.answers,
                    "what its function answers",
                )?;
                for (argument, taken) in arguments.iter().zip(&fn_.takes) {
                    self.fits("an application's argument", argument.ty(), taken);
                }
                Ok(())
            }
        }
    }

    /// Refuses a test naming no case, or a case that is a sum: what a value is tagged with is one
    /// of the leaves a case resolved to, and the checker answers those, so a sum here would be a
    /// test this side had to descend itself.
    fn leaves(&self, what: &str, cases: &[Case]) -> Result<()> {
        leaves(self.declared, &format!("{}: {what}", self.owner), cases)
    }

    /// An operator against what it says it answers.
    ///
    /// Only what holds of every operator node the checker builds, whatever it was written over:
    /// a comparison and a truth operator answer a truth, and a truth operator asks two; `/` answers
    /// a `Rational`; and `+`, `-`, `*` and `++` over two operands of one type answer that type.
    /// Which pairs an operator may be written over, and what it makes of two different ones, is
    /// `ArithmeticCheck`'s and `BinaryElaborator`'s to say, and the checked tree does not record
    /// what they said (souther-lang/souther#1919). Answering it again here would be a copy of the
    /// checker's rule, wrong at its edges, so a pair the checker would refuse is not told apart
    /// here from one this backend has no lowering for: both are refused as not lowered where the
    /// lowering meets them.
    fn operator(
        &mut self,
        op: Op,
        left: &Ty,
        right: &Ty,
        ty: &Ty,
        aborts: &[AbortKind],
    ) -> Result<()> {
        let truth = Ty::Prim { prim: Prim::Bool };
        let what = format!("what {} answers", op.spelt());
        match op {
            Op::And | Op::Or => {
                self.same("a side of a truth operator", left, &truth, "what it asks")?;
                self.same("a side of a truth operator", right, &truth, "what it asks")?;
                self.same(&what, ty, &truth, "what the operator answers")
            }
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                self.same(&what, ty, &truth, "what the operator answers")
            }
            Op::Div => self.same(
                &what,
                ty,
                &Ty::Prim {
                    prim: Prim::Rational,
                },
                "what a quotient is",
            ),
            Op::Add | Op::Sub | Op::Mul | Op::Concat if left == right => {
                if op != Op::Concat {
                    self.number(&what, ty)?;
                }
                self.same(&what, ty, left, "what its operands are")?;
                if op != Op::Concat && matches!(left, Ty::Prim { prim: Prim::Int }) {
                    self.overflows(&format!("{} over two Ints", op.spelt()), aborts)?;
                }
                Ok(())
            }
            Op::Add | Op::Sub | Op::Mul => self.number(&what, ty),
            Op::Concat => Ok(()),
        }
    }

    /// Refuses arithmetic answering what is not a number: whatever it is written over, a sum, a
    /// difference, a product or a negation answers an `Int`, a `Decimal` or a `Rational`
    /// (`AbortSites` holds every one it classifies to that). Arithmetic over a newtype crosses as a
    /// construction over arithmetic on what it wraps.
    fn number(&self, what: &str, ty: &Ty) -> Result<()> {
        if !matches!(
            ty,
            Ty::Prim {
                prim: Prim::Int | Prim::Decimal | Prim::Rational
            }
        ) {
            bail!(
                "{}: {what} is typed {}, where arithmetic answers a number",
                self.owner,
                ty.spelt()
            );
        }
        Ok(())
    }

    /// Refuses a site that can leave its type's range and does not name exactly one reason for
    /// ending without a value: the checker and this backend would disagree about what kind of site
    /// it is, and answering a wrong value because the checker said none would be worse.
    fn overflows(&self, what: &str, aborts: &[AbortKind]) -> Result<()> {
        if aborts.len() != 1 {
            bail!(
                "{}: {what} may leave its type's range and names {} reasons for ending without a \
                 value, where it has exactly one",
                self.owner,
                aborts.len()
            );
        }
        Ok(())
    }

    /// What an arm reads the value it forks on as, against what reaches the arm.
    ///
    /// The one place a read is narrower than what it reads from, and narrower only by what the
    /// arm tested: every value that reaches the arm is a value of what it binds, and what it binds
    /// is no wider than the subject. An optional's present value is read out of the optional, and
    /// is what the optional holds.
    fn arm_binds(&mut self, subject: &Ty, selects: &[Selects], binds: &Ty) -> Result<()> {
        match selects {
            [Selects::Held] => {
                let Ty::Option { option } = subject else {
                    bail!(
                        "{}: an arm reads a present value out of {}, which is not optional",
                        self.owner,
                        subject.spelt()
                    );
                };
                self.same(
                    "what an arm binds",
                    binds,
                    option,
                    "what the optional holds",
                )
            }
            _ if selects.iter().all(|it| matches!(it, Selects::Which { .. })) => {
                let tested: Vec<Case> = selects
                    .iter()
                    .flat_map(|it| match it {
                        Selects::Which { atoms } => atoms.clone(),
                        Selects::Held | Selects::Nothing => Vec::new(),
                    })
                    .collect();
                self.fits(
                    "a case an arm tests is read as what it binds",
                    &Ty::Union { union: tested },
                    binds,
                );
                self.fits(
                    "what an arm binds is read out of its subject",
                    binds,
                    subject,
                );
                Ok(())
            }
            _ => {
                self.not_lowered(format!(
                    "an arm binding the value of {} it tests as more than a present value",
                    subject.spelt()
                ));
                Ok(())
            }
        }
    }

    /// A call's type against what it reaches answers, and its arguments against what that takes.
    fn call(
        &mut self,
        reaches: &Reaches,
        arguments: &[Node],
        ty: &Ty,
        aborts: &[AbortKind],
    ) -> Result<()> {
        match reaches {
            Reaches::Behavior { declared } => {
                let target = self.targets.named(declared)?;
                let takes = target.takes();
                self.arity(
                    &format!("a call of {declared}"),
                    arguments.len(),
                    takes.len(),
                )?;
                self.handed(declared, arguments, &takes);
                self.same(
                    &format!("a call of {declared}"),
                    ty,
                    &target.answers(),
                    "what it answers",
                )
            }
            Reaches::Helper { declared } => {
                let held = *self
                    .reached
                    .helpers
                    .get(&(self.carrier, declared.as_str()))
                    .ok_or_else(|| {
                        anyhow!(
                            "{}: a call of {declared}, which {} holds no copy of",
                            self.owner,
                            self.carrier
                        )
                    })?;
                let takes = held.takes();
                self.arity(
                    &format!("a call of {declared}"),
                    arguments.len(),
                    takes.len(),
                )?;
                self.handed(declared, arguments, &takes);
                // A call stands at the helper's declared answer, which its body only has to be a
                // value of, and the helper's answer on the wire is its body's type.
                self.fits(
                    &format!("what {declared} answers is what a call of it stands at"),
                    held.answers(),
                    ty,
                );
                Ok(())
            }
            Reaches::Value { module, name } => {
                let joined = format!("{module}.{name}");
                let value = *self
                    .reached
                    .values
                    .get(&(self.carrier, joined.clone()))
                    .ok_or_else(|| {
                        anyhow!(
                            "{}: a call of the value {joined}, which {} builds no home for",
                            self.owner,
                            self.carrier
                        )
                    })?;
                let handed: Vec<Ty> = value.handovers.iter().map(|it| it.ty.clone()).collect();
                self.arity(
                    &format!("a call of the value {joined}"),
                    arguments.len(),
                    handed.len(),
                )?;
                self.handed(&joined, arguments, &handed);
                self.same(
                    &format!("a call of the value {joined}"),
                    ty,
                    value.answers(),
                    "what it answers",
                )
            }
            Reaches::PublishedValue { module, name } => {
                if !spells_a_module(module) || !spells_a_name(name) {
                    bail!(
                        "{}: a call of {module}.{name}, which no symbol can carry",
                        self.owner
                    );
                }
                self.arity(
                    &format!("a call of `{module}`'s published value {name}"),
                    arguments.len(),
                    0,
                )?;
                match self.reached.entries.get(&(module.as_str(), name.as_str())) {
                    Some(entry) => self.same(
                        &format!("a call of `{module}`'s published value {name}"),
                        ty,
                        entry.ty(),
                        "what its entry answers",
                    ),
                    // Another build's, and held only to every other call of it.
                    None => {
                        let key = (module.clone(), name.clone());
                        match self.owed.published.get(&key).cloned() {
                            Some(first) => self.same(
                                &format!("a call of `{module}`'s published value {name}"),
                                ty,
                                &first,
                                "another call of it",
                            ),
                            None => {
                                self.owed.published.entry(key).or_insert_with(|| ty.clone());
                                Ok(())
                            }
                        }
                    }
                }
            }
            Reaches::Kernel { kernel } => match kernel.as_str() {
                "int.add" => {
                    let int = Ty::Prim { prim: Prim::Int };
                    self.arity("a call of int.add", arguments.len(), 2)?;
                    self.overflows("a call of int.add", aborts)?;
                    for argument in arguments {
                        self.same(
                            "an argument of int.add",
                            argument.ty(),
                            &int,
                            "what it takes",
                        )?;
                    }
                    self.same("a call of int.add", ty, &int, "what it answers")
                }
                // Refused where it is lowered; nothing here knows what it answers.
                _ => Ok(()),
            },
        }
    }

    fn handed(&mut self, callee: &str, arguments: &[Node], takes: &[Ty]) {
        for (at, (argument, taken)) in arguments.iter().zip(takes).enumerate() {
            self.fits(
                &format!("argument {at} handed to {callee}"),
                argument.ty(),
                taken,
            );
        }
    }
}

/// That a local definition is the one thing its own target says it is, and that what it takes and
/// answers is what the target says.
///
/// What a name answers with is on its target, written by one pass over the checker's program, and
/// its local definition's own tag is written by another. A composition's stages name targets too,
/// and hand each one what the stage before answered.
fn agrees_with_its_target(
    name: &str,
    target: &Target,
    local: &Definition,
    targets: &Targets,
    declared: &Declared,
    owed: &mut Owed,
) -> Result<()> {
    match (target.is, local) {
        // A body's parameters are the target's inputs, one for one, and what the body answers is
        // a value of what the target says it answers — the same type, or a case of it, since the
        // target states what the behavior was declared to answer.
        (
            Answers::Body,
            Definition::Body {
                parameters, body, ..
            },
        ) => {
            if parameters.len() != target.inputs.len() {
                bail!(
                    "{name} names {} parameters and its target takes {}: the two are one list \
                     crossed twice and this document's disagree",
                    parameters.len(),
                    target.inputs.len()
                );
            }
            owed.fits(
                format!("{name} answers what its target answers"),
                body.ty(),
                &target.answers(),
            );
            Ok(())
        }
        (Answers::Composed, Definition::Composed { stages, .. }) => {
            composes(name, target, stages, targets, declared, owed)
        }
        (is, _) => bail!(
            "{name} crosses as {is:?} in the table of targets, and as a different kind of local \
             definition: the two halves disagree about how it is defined"
        ),
    }
}

/// A composition's stages against the targets they name (spec §sequential-composition).
///
/// The first stage takes the composition's own arguments, so nothing is routed into it and what it
/// takes is what the composition takes. Every stage after takes one value: what the stage before
/// answered, or the cases of it the stage accepts. What it does not accept leaves the composition
/// as its answer, and so does what the last stage answers.
fn composes(
    name: &str,
    target: &Target,
    stages: &[crate::transport::Stage],
    targets: &Targets,
    declared: &Declared,
    owed: &mut Owed,
) -> Result<()> {
    let answers = target.answers();
    let (first, rest) = stages
        .split_first()
        .ok_or_else(|| anyhow!("{name} is a composition composing nothing"))?;
    if !matches!(first.routing, Routing::Always) {
        bail!(
            "{name}'s first stage is routed rather than always applied: the first stage of a \
             composition takes the composition's own arguments, and nothing is routed into it"
        );
    }
    let leads = targets.named(&first.behavior)?;
    if leads.takes() != target.takes() {
        bail!(
            "{name} takes {} and its first stage {} takes {}: a composition takes whatever its \
             first stage takes, and the two halves disagree about what that is",
            spelt(&target.takes()),
            first.behavior,
            spelt(&leads.takes())
        );
    }
    let mut running = leads.answers();
    for stage in rest {
        let reached = targets.named(&stage.behavior)?;
        let [taken] = reached.takes().try_into().map_err(|takes: Vec<Ty>| {
            anyhow!(
                "{name}'s stage {} takes {}: a stage after the first takes one value",
                stage.behavior,
                spelt(&takes)
            )
        })?;
        match &stage.routing {
            Routing::Always => owed.fits(
                format!("{name} hands what runs to its stage {}", stage.behavior),
                &running,
                &taken,
            ),
            Routing::OnCases { accepted } => {
                leaves(
                    declared,
                    &format!("{name}'s stage {}", stage.behavior),
                    accepted,
                )?;
                let running_cases = declared.cases_of(&running)?.ok_or_else(|| {
                    anyhow!(
                        "{name}'s stage {} is offered cases of {}, which has none",
                        stage.behavior,
                        running.spelt()
                    )
                })?;
                let offered = Ty::Union {
                    union: accepted.clone(),
                };
                owed.fits(
                    format!(
                        "{name}'s stage {} accepts cases of what runs",
                        stage.behavior
                    ),
                    &offered,
                    &running,
                );
                owed.fits(
                    format!(
                        "{name} hands its stage {} the cases it accepts",
                        stage.behavior
                    ),
                    &offered,
                    &taken,
                );
                let accepted = declared.leaves_of(accepted)?;
                let leaving: Vec<Case> = running_cases
                    .into_iter()
                    .filter(|case| !accepted.contains(case))
                    .collect();
                if !leaving.is_empty() {
                    owed.fits(
                        format!(
                            "what leaves {name} at its stage {} is what it answers",
                            stage.behavior
                        ),
                        &Ty::Union { union: leaving },
                        &answers,
                    );
                }
            }
        }
        running = reached.answers();
    }
    owed.fits(
        format!("what {name}'s last stage answers is what it answers"),
        &running,
        &answers,
    );
    Ok(())
}

/// Refuses a test naming no case, or naming a sum where the checker answers the leaves it descends
/// to.
fn leaves(declared: &Declared, what: &str, cases: &[Case]) -> Result<()> {
    if cases.is_empty() {
        bail!("{what} tests for no case");
    }
    for case in cases {
        if let Case::Declared { declared: key } = case
            && let crate::transport::Declaration::Sum { .. } = declared.shape(key)?
        {
            bail!(
                "{what} tests for {key}, which is a sum, where the checker answers the cases a sum \
                 descends to: the two halves disagree"
            );
        }
    }
    Ok(())
}
