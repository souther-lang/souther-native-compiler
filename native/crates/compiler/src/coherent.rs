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

use crate::closures::ClosureSites;
use crate::transport::{
    Answers, Case, Definition, Held, Node, Op, Prim, Program, Reaches, Routing, Selects, Target,
    Ty, Value,
};
use crate::{Declared, Targets, not_lowered, spelt};
use anyhow::{Result, anyhow, bail};
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

        let mut locals: HashMap<&str, &Definition> = HashMap::new();
        for written in &program.modules {
            for definition in &written.definitions {
                if locals.insert(definition.declared(), definition).is_some() {
                    bail!(
                        "two local definitions are both written {}",
                        definition.declared()
                    );
                }
            }
        }

        let mut owed = Owed::default();
        // From the local definition's side, exhaustively. The declaration loop in `object_for` asks
        // the other direction — that a target answering `Body` or `Composed` has a local
        // definition at all — which is existence and not kind; a target answering `Injected`,
        // `Elsewhere` or `Unwritten` with a local definition under its name anyway never reaches
        // that loop's arm for one.
        for (&name, &local) in &locals {
            let target = targets.named(name)?;
            agrees_with_its_target(name, target, local, &targets, &declared, &mut owed)?;
        }

        let reached = Reached::of(program);
        for written in &program.modules {
            let mut read = Reading {
                carrier: &written.name,
                targets: &targets,
                declared: &declared,
                reached: &reached,
                owed: &mut owed,
            };
            for held in &written.helpers {
                read.body(held.declared.clone(), held.takes(), &held.body)?;
            }
            for value in &written.values {
                let handed = value.handovers.iter().map(|it| it.ty.clone()).collect();
                read.body(value.declared(), handed, &value.body)?;
            }
            for entry in &written.entries {
                let owner = format!("the entry for {}", entry.value.declared());
                read.body(owner, Vec::new(), &entry.body)?;
            }
            for local in &written.definitions {
                if let Definition::Body { declared, body, .. } = local {
                    let target = targets.named(declared)?;
                    read.body(declared.clone(), target.takes(), body)?;
                }
            }
            for example in &written.examples {
                let behavior = format!("{}.{}", written.name, example.behavior);
                let target = targets.named(&behavior)?;
                let owner = format!("row {} of {behavior}", example.at);
                read.body(owner.clone(), Vec::new(), &example.body)?;
                read.owed.fits(
                    format!("{owner} answers what its behavior answers"),
                    example.body.ty(),
                    &target.answers(),
                );
            }
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
            return Err(not_lowered(first));
        }
        Ok(())
    }
}

/// Every definition a call can reach by name, by the key the call writes it under.
struct Reached<'a> {
    /// A helper, by the module holding the copy and the name it was declared under.
    helpers: HashMap<(&'a str, &'a str), &'a Held>,
    /// A value's home, by the module it runs in and its joined name.
    values: HashMap<(&'a str, String), &'a Value>,
    /// A published value's entry body, by the value's module and name, where this document
    /// carries the module that publishes it.
    entries: HashMap<(&'a str, &'a str), &'a Node>,
}

impl<'a> Reached<'a> {
    fn of(program: &'a Program) -> Self {
        let mut reached = Reached {
            helpers: HashMap::new(),
            values: HashMap::new(),
            entries: HashMap::new(),
        };
        for written in &program.modules {
            for held in &written.helpers {
                reached
                    .helpers
                    .insert((&written.name, &held.declared), held);
            }
            for value in &written.values {
                reached
                    .values
                    .insert((&written.name, value.declared()), value);
            }
            for entry in &written.entries {
                reached
                    .entries
                    .insert((&entry.value.module, &entry.value.name), &entry.body);
            }
        }
        reached
    }
}

/// What reading any body of one module needs.
struct Reading<'w, 'a> {
    carrier: &'a str,
    targets: &'w Targets<'a>,
    declared: &'w Declared<'a>,
    reached: &'w Reached<'a>,
    owed: &'w mut Owed,
}

impl<'a> Reading<'_, 'a> {
    /// A top-level body, its parameters bound under the numbers their positions say.
    fn body(&mut self, owner: String, takes: Vec<Ty>, body: &'a Node) -> Result<()> {
        Walk {
            owner,
            carrier: self.carrier,
            targets: self.targets,
            declared: self.declared,
            reached: self.reached,
            bound: takes.into_iter().enumerate().collect(),
            owed: self.owed,
        }
        .node(body)
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

    /// `node` read with `binding` in force at `ty`, and whatever it shadowed back afterwards.
    fn under(&mut self, binding: usize, ty: &Ty, node: &'a Node) -> Result<()> {
        let shadowed = self.bound.insert(binding, ty.clone());
        let read = self.node(node);
        match shadowed {
            Some(before) => self.bound.insert(binding, before),
            None => self.bound.remove(&binding),
        };
        read
    }

    /// No arm standing for the rest: a node added upstream is a node whose value this has not
    /// yet said the source of, and the lowering would read its type on trust.
    fn node(&mut self, node: &'a Node) -> Result<()> {
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
            Node::Unit { declared, ty, .. } => self.same(
                &format!("the unit {declared}"),
                ty,
                &Ty::Declared {
                    declared: declared.clone(),
                },
                "what it names",
            ),
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
                ..
            } => {
                self.node(left)?;
                self.node(right)?;
                let (l, r) = (left.ty(), right.ty());
                let what = format!("what {} answers", op.spelt());
                match op {
                    Op::And | Op::Or => {
                        self.same("a side of a truth operator", l, &bool_, "what it asks")?;
                        self.same("a side of a truth operator", r, &bool_, "what it asks")?;
                        self.same(&what, ty, &bool_, "what the operator answers")
                    }
                    Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                        self.same(&what, ty, &bool_, "what the operator answers")
                    }
                    // What is added to what, and whether the two may differ, is the checker's; what
                    // this holds is only what a lowering of two values of one type answers.
                    Op::Add | Op::Sub | Op::Mul if l == r => {
                        self.same(&what, ty, l, "what its operands are")
                    }
                    Op::Concat if matches!(l, Ty::Prim { prim: Prim::String }) && l == r => {
                        self.same(&what, ty, l, "what its operands are")
                    }
                    Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Concat => Ok(()),
                }
            }
            Node::Neg { operand, ty, .. } => {
                self.node(operand)?;
                self.same("a negation", ty, operand.ty(), "what it negates")
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
                self.under(*binding, binds, body)?;
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
                    match (arm.binding, &arm.binds) {
                        (None, _) => self.node(&arm.body)?,
                        (Some(_), None) => bail!(
                            "{}: an arm binds a value and does not say what it reads it as",
                            self.owner
                        ),
                        (Some(binding), Some(binds)) => {
                            self.arm_binds(subject.ty(), &arm.selects, binds)?;
                            self.under(binding, binds, &arm.body)?;
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
                ..
            } => {
                for argument in arguments {
                    self.node(argument)?;
                }
                self.call(reaches, arguments, ty)
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
                let mut shadowed = Vec::with_capacity(parameters.len());
                for (parameter, taken) in parameters.iter().zip(&fn_.takes) {
                    shadowed.push((
                        parameter.binding,
                        self.bound.insert(parameter.binding, taken.clone()),
                    ));
                }
                let read = self.node(body);
                for (binding, before) in shadowed.into_iter().rev() {
                    match before {
                        Some(before) => self.bound.insert(binding, before),
                        None => self.bound.remove(&binding),
                    };
                }
                read?;
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
    fn call(&mut self, reaches: &Reaches, arguments: &[Node], ty: &Ty) -> Result<()> {
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
                    None => Ok(()),
                }
            }
            Reaches::Kernel { kernel } => match kernel.as_str() {
                "int.add" => {
                    let int = Ty::Prim { prim: Prim::Int };
                    self.arity("a call of int.add", arguments.len(), 2)?;
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
