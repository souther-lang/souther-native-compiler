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
//! says about where its value comes from, and inside a body the two are the same type: a read and
//! its binder, a call and the behavior it reaches, a branch and the fork it answers, an argument
//! and the parameter it is handed to. Where the checker let a narrower value stand, the value in
//! the slot is a `Widen` saying so, typed as what the slot takes, and whether its value is a value
//! of that is asked of the `Widen` alone. Where every child stands is one table, `Walk::slots`,
//! which names each child of each node, typed or not and why, and is held to name every one; a
//! node's own relations are apart from it. Where the wire carried the same fact twice for no reason,
//! it no longer does: a helper's and a value's answer are their body's type, a helper's parameters
//! are named and typed together, and a composition's answers are its targets'. What a let binds its
//! name at is carried because it is the one thing a read of it can be held to.
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
//! that a name can stand in a symbol, what every operator node answers and what its reading says
//! of its operands, what a kernel's application takes, and every relation between a node's type and
//! what its value is made from. What is not held, because the document does not carry the checker's
//! side of it:
//!
//! - which types an operator admits. A reading says what the operands were taken as and not
//!   whether the operator orders them, so an ordering over two truths read as they stand is
//!   refused as not lowered, as a pair this backend has no lowering for is;
//! - why a value may stand as a wider type. The document says where it does, and that is asked
//!   of [`Declared::fits`], which copies the part of the checker's rule for the types laid out
//!   here;
//! - what the writer drops: which values a module publishes beyond the entries it has, what a row
//!   expects.

use crate::closures::ClosureSites;
use crate::index;
use crate::kernels::{Bound, LoweredKernel};
use crate::transport::{
    AbortKind, Answers, Carrier, Case, Declaration, Definition, Ensures, Guard, Held, Node, Op,
    Owner, Prim, Program, Reaches, Reading, Routing, Selects, Target, Ty, Value,
};
use crate::{Declared, Runs, Targets, departures_taken, not_lowered, says_its_case, spelt};
use anyhow::{Result, anyhow, bail};
use souther_native_abi::{spells_a_module, spells_a_name};
use std::collections::HashMap;

/// A document every relation of which holds, and what reading it built.
pub(crate) struct Coherent<'a> {
    pub declared: Declared<'a>,
    pub targets: Targets<'a>,
    /// Every local definition this object holds, by the name it defines.
    pub locals: HashMap<&'a str, &'a Definition>,
    /// What this object runs, which is narrower than what the document says.
    pub runs: Runs<'a>,
    /// Every closure site under what this object runs.
    pub closures: ClosureSites<'a>,
    /// Which object defines each behavior's symbol, and as what, by the name it is declared
    /// under. Decided here once, from what the target says it is and whether a module this
    /// document builds declares it, and read by whatever emits or describes the behavior.
    pub defined: HashMap<String, Defined>,
}

/// Which object defines a behavior's symbol, and as what.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Defined {
    /// This object, as the local definition a module of it holds: a body or a composition.
    Here,
    /// This object, as a call to what a host registered for it on the calling thread: a module this
    /// document builds declares it with no body and nothing to depend on.
    ByTheHost,
    /// Another object: the build that implements it, or the build that declares it with no body
    /// and answers it with what a host registered. A call is the same call either way.
    Elsewhere,
    /// Nothing: it was never written.
    Nowhere,
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
        // Every site the document holds is numbered once, whether or not this object runs it: a
        // number two sites share is the two halves disagreeing wherever it stands.
        ClosureSites::of(program.bodies())?;

        let reached = Reached::of(program)?;
        for written in &program.modules {
            let mut published = HashMap::new();
            for key in &written.publishes {
                let declaration = declared.shape(key)?;
                // A module publishes what it declares and nothing another module does.
                if declaration.module() != written.name
                    || declaration.by() != crate::transport::DeclaredBy::AModule
                {
                    bail!(
                        "{} publishes {key}, which {} declares: a module publishes the data it \
                         declares",
                        written.name,
                        declaration.module()
                    );
                }
                index::once(&mut published, key.as_str(), (), || {
                    format!("{} publishes {key} twice", written.name)
                })?;
            }
        }

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
        let mut defined = HashMap::new();
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
            let declared_here = reached.modules.contains_key(target.module.as_str());
            placed(&name, target, declared_here)?;
            let definition = match (target.is, declared_here) {
                (Answers::Body | Answers::Composed, _) => Defined::Here,
                (Answers::Injected, true) => Defined::ByTheHost,
                (Answers::Injected, false) | (Answers::Elsewhere, _) => Defined::Elsewhere,
                (Answers::Unwritten, _) => Defined::Nowhere,
            };
            index::unique(&mut defined, name.clone(), definition);
        }

        let runs = Runs::of(program, &declared)?;
        let mut owed = Owed::default();
        for (&name, &local) in &locals {
            let target = targets.named(name)?;
            agrees_with_its_target(name, target, local, &targets, &declared, &mut owed)?;
        }

        for body in program.bodies() {
            // What each body is handed, bound under the number the writer gave it: a parameter's
            // is where it stands among the parameters, and a field's is the binding the checker
            // gave the field, which is not where the field sits.
            let positional =
                |takes: Vec<Ty>| -> Vec<(usize, Ty)> { takes.into_iter().enumerate().collect() };
            // What a rule tests the answer for before it reads it, against what the answer is.
            let mut guarded: Option<(&Guard, Ty)> = None;
            let (owner, bound) = match body.owner {
                Owner::Helper(held) => (held.reached.clone(), positional(held.takes())),
                Owner::Value(value) => (
                    value.declared(),
                    positional(value.handovers.iter().map(|it| it.ty.clone()).collect()),
                ),
                Owner::Entry(entry) => (
                    format!("the entry for {}", entry.value.declared()),
                    Vec::new(),
                ),
                Owner::Definition(declared) => (
                    declared.to_string(),
                    positional(targets.named(declared)?.takes()),
                ),
                Owner::Invariant { declaration, at } => {
                    let clause = &declaration.clauses().expect(
                        "a clause is listed only of a declaration that carries its clauses",
                    )[at];
                    let owner = match &clause.name {
                        Some(name) => format!("{}'s clause {name}", declaration.key()),
                        None => format!("{}'s clause {at}", declaration.key()),
                    };
                    holds_and_builds_nothing(&owner, body.node)?;
                    (owner, fields_bound(declaration))
                }
                Owner::Ensures { target, at } => {
                    let contract = target
                        .ensures
                        .contract()
                        .expect("a rule is listed only of a target that carries its contract");
                    let rule = &contract.rules[at];
                    let owner = match &rule.clause {
                        Some(name) => format!("{}'s ensures {name}", target.declared()),
                        None => format!("{}'s ensures rule {at}", target.declared()),
                    };
                    holds_and_builds_nothing(&owner, body.node)?;
                    let answers = target.answers();
                    guarded = Some((&rule.guard, answers.clone()));
                    let mut bound = positional(target.takes());
                    bound.push((rule.value, rule.guard.reads_as(&answers).clone()));
                    (owner, bound)
                }
                Owner::Example(example) => {
                    let behavior = format!("{}.{}", body.carrier().module(), example.behavior);
                    let owner = format!("row {} of {behavior}", example.at);
                    let answers = targets.named(&behavior)?.answers();
                    if body.node.ty() != &answers {
                        bail!(
                            "{owner} is typed {} and its behavior answers {}: the two halves \
                             disagree",
                            body.node.ty().spelt(),
                            answers.spelt()
                        );
                    }
                    (owner, Vec::new())
                }
            };
            let mut walk = Walk {
                owner,
                open: matches!(body.owner, Owner::Helper(_)),
                carrier: body.carrier(),
                targets: &targets,
                declared: &declared,
                reached: &reached,
                bound: HashMap::new(),
                owed: &mut owed,
                runs: runs.runs(&body),
            };
            // A rule over a case tests the answer the way an arm tests what it forks on, and reads
            // it as an arm reads what it binds.
            if let Some((Guard::Case { selects, binds }, answers)) = guarded {
                let selects = std::slice::from_ref(selects);
                walk.tests("a rule", &answers, selects)?;
                walk.arm_binds(&answers, selects, binds)?;
            }
            walk.under(bound, body.node)?;
        }

        owed.settle(&declared)?;

        // What a function value written in a helper over type variables carries is not laid out
        // for the helper as it is written: nothing of it is lowered but its copies.
        let closures = ClosureSites::of(runs.bodies().filter(|body| !body.leaves_types_open()))?;
        Ok(Coherent {
            declared,
            targets,
            locals,
            runs,
            closures,
            defined,
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
    /// Whether the relation stands in something this object runs. One that does not is still held
    /// to the checker's answer, and not refused as not lowered where this backend cannot say.
    runs: bool,
}

impl Owed {
    fn fits(&mut self, what: String, actual: &Ty, expected: &Ty) {
        self.fits_where(true, what, actual, expected);
    }

    fn fits_where(&mut self, runs: bool, what: String, actual: &Ty, expected: &Ty) {
        self.fits.push(Owing {
            what,
            actual: actual.clone(),
            expected: expected.clone(),
            runs,
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
                None if owing.runs => undecided.push(format!(
                    "whether a value of {} is one of {}",
                    owing.actual.spelt(),
                    owing.expected.spelt()
                )),
                None => {}
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
    /// A helper, by the module holding the copy and the reference a call there reaches it by.
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
                    (module, held.reached.as_str()),
                    held,
                    || format!("{module} holds two helpers both written {}", held.reached),
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
            // What a value's handover carries is another value this module builds, which the
            // method it is handed to takes already built. The lowering reads the handover's type
            // and never what it carries, so a handover naming a value nothing builds would be a
            // statement nothing held.
            for value in &written.values {
                for handover in &value.handovers {
                    let carried = handover.carries.declared();
                    if !reached.values.contains_key(&(module, carried.clone())) {
                        bail!(
                            "{}: {} is handed {carried}, which {module} builds no value of: the \
                             two halves disagree",
                            value.declared(),
                            handover.parameter
                        );
                    }
                }
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

/// Where a child of a node stands, and what the tree says of its type there.
enum Slot<'n> {
    /// At a type the node states, which the child is of exactly: where the checker let a narrower
    /// value stand there, the child is the `Widen` saying so.
    Typed {
        child: &'n Node,
        takes: Ty,
        what: String,
    },
    /// At no type the tree states, and why not.
    Untyped(
        &'n Node,
        #[expect(
            dead_code,
            reason = "stated where a child is placed, for whoever adds a node, and not read"
        )]
        Untyped,
    ),
}

impl<'n> Slot<'n> {
    fn child(&self) -> &'n Node {
        match self {
            Slot::Typed { child, .. } | Slot::Untyped(child, _) => child,
        }
    }
}

/// Why a child stands at no type the tree states. Named for whoever adds a node or an operator, who
/// has to say which of these a child is or give it a slot; nothing reads it.
#[derive(Clone, Copy)]
enum Untyped {
    /// What the node reads from or forks on: a field's target, a member's tuple, a match's
    /// subject, an applied function. The node's own relation to it is what is held.
    ReadFrom,
    /// A `Widen`'s value, which stands as another type by what the `Widen` says, and is asked of
    /// `fits` there.
    Widened,
    /// An operand of a comparison or an arithmetic operator, which the operator reads as its
    /// reading says: not a place the operand stands, since a literal beside a newtype is read as
    /// the newtype by this operator and by nothing else. What the reading holds of the pair is
    /// held with the operator.
    Operand,
}

/// One body read with what is bound where it stands.
struct Walk<'w, 'a> {
    /// Whose body this is, which every refusal names.
    owner: String,
    /// Whether a type here may be a variable: in a helper's body, which is the one place the
    /// checker leaves one open, and nowhere else.
    open: bool,
    /// The module whose copy of a helper a call from here reaches.
    carrier: Carrier<'a>,
    targets: &'w Targets<'a>,
    declared: &'w Declared<'a>,
    reached: &'w Reached<'a>,
    /// What each binding in scope is in force at, as the node that made it says.
    bound: HashMap<usize, Ty>,
    owed: &'w mut Owed,
    /// Whether this object runs the body. What this backend has no lowering for is refused only
    /// where it would be lowered; the two halves disagreeing is refused wherever it stands.
    runs: bool,
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
        self.owed.fits_where(self.runs, what, actual, expected);
    }

    /// Refuses a type naming a declaration the document does not carry, and a type variable
    /// outside a helper's body, which is a type nobody settled.
    fn resolves(&self, what: &str, ty: &Ty) -> Result<()> {
        if self.open {
            self.declared.resolves_open(what, ty)
        } else {
            self.declared.resolves(what, ty)
        }
    }

    fn not_lowered(&mut self, what: String) {
        if self.runs {
            self.owed.not_lowered.push(what);
        }
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

    /// `node` read with each of `bindings` in force at its type, and gone again afterwards.
    ///
    /// A number names one binder in a body: a binder whose number is already in force is refused,
    /// as the two halves disagreeing. The writer counts a binder where it writes it, so no document
    /// it writes shadows one, and a document that does is one whose meaning is a lexical scope this
    /// reader, the closure planning and the lowering would each have to keep in step. Refused, they
    /// agree because there is nothing for them to agree about.
    fn under(&mut self, bindings: Vec<(usize, Ty)>, node: &'a Node) -> Result<()> {
        let mut entered: Vec<usize> = Vec::new();
        for (binding, ty) in bindings {
            let bound = self
                .resolves(&format!("{}: binding {binding}", self.owner), &ty)
                .and_then(|()| {
                    index::once(&mut self.bound, binding, ty, || {
                        format!(
                            "{}: binding {binding} is bound where it is already in force: a \
                             number names one binder in a body, and the two halves disagree",
                            self.owner
                        )
                    })
                });
            if let Err(refused) = bound {
                self.leave(&entered);
                return Err(refused);
            }
            entered.push(binding);
        }
        let read = self.node(node);
        self.leave(&entered);
        read
    }

    /// Each of `bound` out of force again, the last entered first.
    fn leave(&mut self, bound: &[usize]) {
        for binding in bound.iter().rev() {
            self.bound.remove(binding);
        }
    }

    /// `node` and everything under it: what each node says it is against where its value comes
    /// from, and each child against the slot it stands in.
    fn node(&mut self, node: &'a Node) -> Result<()> {
        self.relations(node)?;
        self.hold_slots(node)
    }

    /// Every child of `node` against the slot it stands in, from the one table of them.
    ///
    /// The table names each child once, in the order the node holds them, or this compiler has
    /// written a node's slots without deciding one of them; so a child cannot stand in a slot
    /// nothing here holds by being left out of it.
    fn hold_slots(&self, node: &'a Node) -> Result<()> {
        let slots = self.slots(node)?;
        let children = node.children();
        assert!(
            slots.len() == children.len()
                && slots
                    .iter()
                    .zip(&children)
                    .all(|(slot, child)| std::ptr::eq(slot.child(), *child)),
            "the slots of a node name each of its children once, in order"
        );
        for slot in &slots {
            if let Slot::Typed { child, takes, what } = slot {
                self.same(what, child.ty(), takes, "the slot it stands in")?;
            }
        }
        Ok(())
    }

    /// Where each child of `node` stands, and what the tree says of its type there.
    ///
    /// Inside a body every slot holds a value of exactly the type it takes, and where the checker
    /// let a narrower value stand, what stands there is a `Widen` saying so. No arm standing for the
    /// rest, over the nodes, the operators and what a call reaches alike: each states every child.
    fn slots(&self, node: &'a Node) -> Result<Vec<Slot<'a>>> {
        let truth = Ty::Prim { prim: Prim::Bool };
        let typed = |child: &'a Node, takes: &Ty, what: String| Slot::Typed {
            child,
            takes: takes.clone(),
            what,
        };
        Ok(match node {
            Node::Int { .. }
            | Node::Read { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Unit { .. }
            | Node::None { .. } => Vec::new(),
            Node::Construct {
                declared, values, ..
            } => self
                .declared
                .shape(declared)?
                .fields()
                .iter()
                .zip(values)
                .map(|(field, value)| {
                    typed(
                        value,
                        &field.codec.ty(),
                        format!("the value {declared}'s field {} is given", field.name),
                    )
                })
                .collect(),
            Node::Attempt {
                declared,
                values,
                then,
                departures,
                ty,
                ..
            } => self
                .declared
                .shape(declared)?
                .fields()
                .iter()
                .zip(values)
                .map(|(field, value)| {
                    typed(
                        value,
                        &field.codec.ty(),
                        format!("the value {declared}'s field {} is given", field.name),
                    )
                })
                .chain(std::iter::once(typed(
                    then,
                    ty,
                    "what an attempted construction answers where it builds".to_string(),
                )))
                .chain(departures.bodies().into_iter().map(|body| {
                    typed(
                        body,
                        ty,
                        "what an attempted construction's departure answers".to_string(),
                    )
                }))
                .collect(),
            Node::Field { target, .. } => vec![Slot::Untyped(target, Untyped::ReadFrom)],
            Node::Binary {
                op,
                left,
                right,
                ty,
                ..
            } => {
                let sides = |takes: &Ty, of: &str| {
                    vec![
                        typed(left, takes, format!("the left side of {of}")),
                        typed(right, takes, format!("the right side of {of}")),
                    ]
                };
                match op {
                    Op::And | Op::Or => sides(&truth, op.spelt()),
                    Op::Concat => sides(ty, "++"),
                    Op::Eq
                    | Op::Ne
                    | Op::Lt
                    | Op::Le
                    | Op::Gt
                    | Op::Ge
                    | Op::Add
                    | Op::Sub
                    | Op::Mul
                    | Op::Div => vec![
                        Slot::Untyped(left, Untyped::Operand),
                        Slot::Untyped(right, Untyped::Operand),
                    ],
                }
            }
            Node::Neg { operand, ty, .. } => {
                vec![typed(operand, ty, "what a negation negates".to_string())]
            }
            Node::Let {
                binding,
                binds,
                value,
                body,
                ty,
                ..
            } => vec![
                typed(
                    value,
                    binds,
                    format!("the value binding {binding} is given"),
                ),
                typed(body, ty, "what a let's body answers".to_string()),
            ],
            Node::If {
                cond,
                then,
                els,
                ty,
                ..
            } => vec![
                typed(cond, &truth, "what a fork asks".to_string()),
                typed(then, ty, "what a fork's branch answers".to_string()),
                typed(els, ty, "what a fork's branch answers".to_string()),
            ],
            Node::Match {
                subject, arms, ty, ..
            } => std::iter::once(Slot::Untyped(subject, Untyped::ReadFrom))
                .chain(
                    arms.iter()
                        .map(|arm| typed(&arm.body, ty, "what a match arm answers".to_string())),
                )
                .collect(),
            Node::Some { value, ty, .. } => {
                let Ty::Option { option } = ty else {
                    bail!(
                        "{}: a present value typed {}, which is not optional",
                        self.owner,
                        ty.spelt()
                    );
                };
                vec![typed(
                    value,
                    option,
                    "what a present value holds".to_string(),
                )]
            }
            Node::Tuple { members, ty, .. } => {
                let Ty::Tuple { tuple } = ty else {
                    bail!(
                        "{}: a tuple typed {}, which is not a tuple",
                        self.owner,
                        ty.spelt()
                    );
                };
                self.arity("a tuple", members.len(), tuple.len())?;
                members
                    .iter()
                    .zip(tuple)
                    .enumerate()
                    .map(|(at, (member, held))| {
                        typed(member, held, format!("member {at} of a tuple"))
                    })
                    .collect()
            }
            Node::Member { tuple, .. } => vec![Slot::Untyped(tuple, Untyped::ReadFrom)],
            Node::List { elements, ty, .. } => {
                let Ty::List { list } = ty else {
                    bail!(
                        "{}: a list typed {}, which is not a list",
                        self.owner,
                        ty.spelt()
                    );
                };
                elements
                    .iter()
                    .enumerate()
                    .map(|(at, element)| typed(element, list, format!("element {at} of a list")))
                    .collect()
            }
            Node::Call {
                reaches,
                arguments,
                ty,
                ..
            } => {
                let (callee, takes) = self.parameters(reaches, arguments, ty)?;
                arguments
                    .iter()
                    .zip(takes)
                    .enumerate()
                    .map(|(at, (argument, taken))| {
                        typed(
                            argument,
                            &taken,
                            format!("argument {at} handed to {callee}"),
                        )
                    })
                    .collect()
            }
            Node::Block { body, ty, site, .. } => {
                let Ty::Fn { fn_ } = ty else {
                    bail!(
                        "{}: closure site {site} is typed {}, which is not a function type",
                        self.owner,
                        ty.spelt()
                    );
                };
                vec![typed(
                    body,
                    &fn_.answers,
                    format!("what closure site {site} answers"),
                )]
            }
            Node::Apply {
                function,
                arguments,
                ..
            } => {
                let Ty::Fn { fn_ } = function.ty() else {
                    bail!(
                        "{}: an application of {}, which is not a function type",
                        self.owner,
                        function.ty().spelt()
                    );
                };
                std::iter::once(Slot::Untyped(function, Untyped::ReadFrom))
                    .chain(arguments.iter().zip(&fn_.takes).enumerate().map(
                        |(at, (argument, taken))| {
                            typed(argument, taken, format!("argument {at} of an application"))
                        },
                    ))
                    .collect()
            }
            Node::Widen { value, .. } => vec![Slot::Untyped(value, Untyped::Widened)],
        })
    }

    /// What a call hands each argument over as: the parameters of what it reaches, by the name of
    /// that. A kernel's are what the checker settled its signature to for this application, and a
    /// helper's are what it takes with each variable it leaves open standing as what this call
    /// settles it to ([`Walk::helper_called`]).
    fn parameters(
        &self,
        reaches: &Reaches,
        arguments: &[Node],
        answers: &Ty,
    ) -> Result<(String, Vec<Ty>)> {
        Ok(match reaches {
            Reaches::Behavior { declared } => {
                (declared.clone(), self.targets.named(declared)?.takes())
            }
            Reaches::Helper { reached } => {
                let (held, bound) = self.helper_called(reached, arguments, answers)?;
                let takes = held
                    .parameters
                    .iter()
                    .map(|parameter| {
                        bound
                            .applied(&parameter.ty)
                            .expect("a parameter's variables are bound by what is handed to it")
                    })
                    .collect();
                (reached.clone(), takes)
            }
            Reaches::Value { module, name } => {
                let joined = format!("{module}.{name}");
                let value = self
                    .reached
                    .values
                    .get(&(self.carrier.module(), joined.clone()))
                    .ok_or_else(|| {
                        anyhow!(
                            "{}: a call of the value {joined}, which {} builds no home for",
                            self.owner,
                            self.carrier.module()
                        )
                    })?;
                let handed = value.handovers.iter().map(|it| it.ty.clone()).collect();
                (format!("the value {joined}"), handed)
            }
            Reaches::PublishedValue { module, name } => {
                (format!("`{module}`'s published value {name}"), Vec::new())
            }
            Reaches::Kernel { kernel, takes, .. } => (kernel.clone(), takes.clone()),
        })
    }

    /// What each node says it is against where its value comes from: a read and its binder, a
    /// call and what it reaches, a construction and its declaration. Where each child stands is
    /// [`Walk::slots`]'s, and nothing here holds a child to a slot.
    ///
    /// No arm standing for the rest: a node added upstream is a node whose value this has not
    /// yet said the source of, and the lowering would read its type on trust.
    fn relations(&mut self, node: &'a Node) -> Result<()> {
        // Every type the node writes, not only its own: one carried beside it is as much a name the
        // document says it declares.
        for ty in node.types() {
            self.resolves(&self.owner, ty)?;
        }
        // What a node can end without a value for is its kind's, and only some kinds can. A node
        // of any other kind naming a reason is one the checker does not write, and the lowering,
        // which reads a reason only where a kind has one to give, would not notice it.
        if !node.can_end_without_a_value() && !node.aborts().is_empty() {
            bail!(
                "{}: a node that ends no run without a value names {:?} as what it can end without \
                 one for: the two halves disagree",
                self.owner,
                node.aborts()
            );
        }
        match node {
            Node::Int { ty, .. } => self.same(
                "an integer literal",
                ty,
                &Ty::Prim { prim: Prim::Int },
                "its kind",
            ),
            Node::Bool { ty, .. } => self.same(
                "a truth literal",
                ty,
                &Ty::Prim { prim: Prim::Bool },
                "its kind",
            ),
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
                aborts,
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
                // A construction ends without a value where a clause does not hold, and the checker
                // says so of exactly the constructions of a type that states one. Of a type
                // another build builds the clauses are that build's, and what each is answered
                // under is carried all the same, so whether it states one is known here too.
                let owes = [AbortKind::InvariantNotHeld];
                let (holds, states) = if shape.clause_names().is_empty() {
                    (aborts.is_empty(), "states no clause")
                } else {
                    (aborts.as_slice() == owes, "states what its values owe")
                };
                if !holds {
                    bail!(
                        "{}: a construction of {declared}, whose type {states}, names {:?} as what \
                         it can end without a value for: the two halves disagree",
                        self.owner,
                        aborts
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
                for value in values {
                    self.node(value)?;
                }
                Ok(())
            }
            Node::Attempt {
                declared,
                values,
                binding,
                binds,
                then,
                departures,
                ..
            } => {
                let shape = self.declared.shape(declared)?;
                if !matches!(
                    shape,
                    crate::transport::Declaration::Product { .. }
                        | crate::transport::Declaration::Newtype { .. }
                ) {
                    bail!(
                        "{}: {declared} is attempted and is not declared with fields to build",
                        self.owner
                    );
                }
                if shape.field_count() != values.len() {
                    bail!(
                        "{}: {declared} is declared with {} fields and is attempted here from {}",
                        self.owner,
                        shape.field_count(),
                        values.len()
                    );
                }
                // What is bound is what is built, where every clause held.
                self.same(
                    &format!("what an attempted construction of {declared} binds"),
                    binds,
                    &Ty::Declared {
                        declared: declared.clone(),
                    },
                    "what it builds",
                )?;
                // A type that states no clause has no failing side, which the checker refuses to
                // attempt; and every clause it states is answered by one departure.
                let clauses = shape.clause_names();
                if clauses.is_empty() {
                    bail!(
                        "{}: {declared} is attempted and states no clause, which the checker \
                         refuses: the two halves disagree",
                        self.owner
                    );
                }
                departures_taken(&clauses, departures).map_err(|why| {
                    anyhow!(
                        "{}: an attempted construction of {declared}: {why}: the two halves \
                         disagree",
                        self.owner
                    )
                })?;
                for value in values {
                    self.node(value)?;
                }
                self.under(vec![(*binding, binds.clone())], then)?;
                for body in departures.bodies() {
                    self.node(body)?;
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
                reading,
                left,
                right,
                ty,
                aborts,
            } => {
                self.node(left)?;
                self.node(right)?;
                self.reading(*op, reading, left.ty(), right.ty())?;
                self.operator(*op, reading, (left.ty(), right.ty()), ty, aborts)
            }
            Node::Neg {
                operand,
                ty,
                aborts,
            } => {
                self.node(operand)?;
                self.number("a negation", ty)?;
                // What a negation can end without a value for is decided by the type it answers,
                // and not by what it negates: the smallest `Int` has no counterpart, so a
                // negation of an `Int` names one reason, a literal's included, and a `Decimal` or
                // a `Rational` only changes sign and names none. That the lowering folds a
                // literal's sign is its own, and says nothing of what the checker states.
                let owed: &[AbortKind] = if matches!(ty, Ty::Prim { prim: Prim::Int }) {
                    &[AbortKind::RequiredFormHasNoPlace]
                } else {
                    &[]
                };
                self.ends_for(&format!("a negation of {}", ty.spelt()), aborts, owed)?;
                // A literal is a magnitude the checker writes in `[0, Int.MAX]`: `-Int.MIN` is not
                // one a source can name, and the lowering negates it as it stands.
                if let Node::Int { value, .. } = operand.as_ref()
                    && *value == i64::MIN
                {
                    bail!(
                        "{}: a negation of the literal {value}, which is no magnitude the checker \
                         writes: the two halves disagree",
                        self.owner
                    );
                }
                Ok(())
            }
            Node::Let {
                binding,
                binds,
                value,
                body,
                ..
            } => {
                // Read before the binding is in force: a binding is not read in its own value.
                self.node(value)?;
                self.under(vec![(*binding, binds.clone())], body)
            }
            Node::If {
                cond, then, els, ..
            } => {
                self.node(cond)?;
                self.node(then)?;
                self.node(els)
            }
            Node::Match { subject, arms, .. } => {
                self.node(subject)?;
                for arm in arms {
                    if arm.selects.is_empty() {
                        bail!("{}: an arm tests for nothing", self.owner);
                    }
                    self.tests("an arm", subject.ty(), &arm.selects)?;
                    match (arm.binding, &arm.binds) {
                        (None, None) => self.node(&arm.body)?,
                        // The writer says both or neither, so an arm saying one is a statement
                        // the lowering would drop: it reads what an arm binds only from an arm
                        // that binds.
                        (None, Some(_)) => bail!(
                            "{}: an arm binds nothing and says what it reads it as: the two \
                             halves disagree",
                            self.owner
                        ),
                        (Some(_), None) => bail!(
                            "{}: an arm binds a value and does not say what it reads it as",
                            self.owner
                        ),
                        (Some(binding), Some(binds)) => {
                            self.arm_binds(subject.ty(), &arm.selects, binds)?;
                            self.under(vec![(binding, binds.clone())], &arm.body)?;
                        }
                    }
                }
                Ok(())
            }
            Node::Some { value, .. } => self.node(value),
            Node::None { ty, .. } => match ty {
                Ty::Option { .. } => Ok(()),
                _ => bail!(
                    "{}: an absent value typed {}, which is not optional",
                    self.owner,
                    ty.spelt()
                ),
            },
            Node::Tuple { members, .. }
            | Node::List {
                elements: members, ..
            } => {
                for member in members {
                    self.node(member)?;
                }
                Ok(())
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
                self.under(bound, body)
            }
            Node::Widen { value, ty, .. } => {
                if matches!(value.as_ref(), Node::Widen { .. }) {
                    bail!(
                        "{}: a value stands as {} and again as {}, where a value stands at one \
                         position once",
                        self.owner,
                        value.ty().spelt(),
                        ty.spelt()
                    );
                }
                if value.ty() == ty {
                    bail!(
                        "{}: a value of {} is said to stand as its own type, which the checker \
                         never says",
                        self.owner,
                        ty.spelt()
                    );
                }
                self.node(value)?;
                self.fits("a value standing as a wider type", value.ty(), ty);
                Ok(())
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
                )
            }
        }
    }

    /// Refuses a test naming no case, or a case that is a sum: what a value is tagged with is one
    /// of the leaves a case resolved to, and the checker answers those, so a sum here would be a
    /// test this side had to descend itself.
    fn leaves(&self, what: &str, cases: &[Case]) -> Result<()> {
        leaves(self.declared, &format!("{}: {what}", self.owner), cases)
    }

    /// What an operator's reading says of its operands.
    ///
    /// Read as they stand, they are one type. Read at their exact values, each is a number. Read
    /// in a type, that type is one the document carries. Which pairs an operator is written over,
    /// and which reading the checker gives each, is the checker's rule and is not answered again
    /// here: this holds only what a reading, once given, says.
    fn reading(&self, op: Op, reading: &Reading, left: &Ty, right: &Ty) -> Result<()> {
        // Which readings an operator can have. The checker reads a truth operator and a join as
        // their operands stand, always, and arithmetic as they stand or at their exact values:
        // only a comparison is read in a type. A document saying otherwise is one the lowering,
        // which asks the reading before the operator, would lower under a reading the operator
        // never has.
        let refused = matches!(
            (op, reading),
            (
                Op::And | Op::Or | Op::Concat,
                Reading::In { .. } | Reading::ExactNumbers
            ) | (Op::Add | Op::Sub | Op::Mul | Op::Div, Reading::In { .. })
        );
        if refused {
            bail!(
                "{}: {} is read {}, which the checker never reads it as: the two halves disagree",
                self.owner,
                op.spelt(),
                reading.spelt()
            );
        }
        match reading {
            Reading::AsTheyStand => self.same(
                &format!("the left side of {} read as it stands", op.spelt()),
                left,
                right,
                "the right side",
            ),
            Reading::ExactNumbers => {
                self.number(
                    &format!("a side of {} read at its exact value", op.spelt()),
                    left,
                )?;
                self.number(
                    &format!("a side of {} read at its exact value", op.spelt()),
                    right,
                )
            }
            // The type it is read in is one the document declares, which `Node::types` holds of
            // every type a node writes.
            Reading::In { .. } => Ok(()),
        }
    }

    /// An operator against what it says it answers. Where its operands stand is [`Walk::slots`]'s,
    /// and what its reading says of them is [`Walk::reading`]'s.
    ///
    /// A comparison and a truth operator answer a truth, and `/` a `Rational`. A sum, a difference
    /// or a product answers the type its operands are read as where they are read as they stand,
    /// and a `Rational` where they are read at their exact values.
    fn operator(
        &mut self,
        op: Op,
        reading: &Reading,
        (left, right): (&Ty, &Ty),
        ty: &Ty,
        aborts: &[AbortKind],
    ) -> Result<()> {
        let truth = Ty::Prim { prim: Prim::Bool };
        let rational = Ty::Prim {
            prim: Prim::Rational,
        };
        let what = format!("what {} answers", op.spelt());
        match op {
            Op::And | Op::Or => self.same(&what, ty, &truth, "what the operator answers"),
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                self.same(&what, ty, &truth, "what the operator answers")
            }
            Op::Div => {
                self.same(&what, ty, &rational, "what a quotient is")?;
                // A quotient ends a run for a zero divisor, and for an answer with no place where
                // an operand is already exact.
                let exact = |it: &Ty| {
                    matches!(
                        it,
                        Ty::Prim {
                            prim: Prim::Rational
                        }
                    )
                };
                let owed: &[AbortKind] = if exact(left) || exact(right) {
                    &[AbortKind::DivisionByZero, AbortKind::RequiredFormHasNoPlace]
                } else {
                    &[AbortKind::DivisionByZero]
                };
                self.ends_for(&format!("a quotient of {}", left.spelt()), aborts, owed)
            }
            // Both sides stand at what it answers, which its slots hold; that is all a join says
            // of itself.
            Op::Concat => Ok(()),
            Op::Add | Op::Sub | Op::Mul => {
                match reading {
                    Reading::AsTheyStand => {
                        self.number(&what, ty)?;
                        self.same(&what, ty, left, "what its operands are read as")?;
                    }
                    Reading::ExactNumbers => {
                        self.same(&what, ty, &rational, "what exact values come to")?;
                    }
                    Reading::In { .. } => self.number(&what, ty)?,
                }
                // A sum, a difference or a product of numbers leaves the range its answer holds,
                // whichever reading its operands have, and names the one reason for it.
                self.ends_for(
                    &format!("{} over {}", op.spelt(), left.spelt()),
                    aborts,
                    &[AbortKind::RequiredFormHasNoPlace],
                )
            }
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

    /// Refuses a site that names other than the reasons it owes for ending without a value.
    ///
    /// The reasons themselves and not how many there are: the lowering turns the one it is given
    /// into the status the run ends with, so a document naming another reason for a site would be
    /// lowered to a run that ends for a reason the checker never gave it.
    fn ends_for(&self, what: &str, aborts: &[AbortKind], owed: &[AbortKind]) -> Result<()> {
        if aborts != owed {
            bail!(
                "{}: {what} names {aborts:?} as what it can end without a value for, where the \
                 checker names {owed:?}: the two halves disagree",
                self.owner
            );
        }
        Ok(())
    }

    /// What `what` tests a value of `subject` for, against what the value can be.
    ///
    /// Held whether anything is then read out of the value or not: the lowering turns each test
    /// into one comparison of the value, and a test of absence is a comparison with no case in it,
    /// so it means something only of an optional, and a test of a case only of a value that is not
    /// one. An arm tests what it forks on this way, and a rule tests an answer.
    fn tests(&mut self, what: &str, subject: &Ty, selects: &[Selects]) -> Result<()> {
        let optional = matches!(subject, Ty::Option { .. });
        for one in selects {
            match one {
                Selects::Which { atoms } => {
                    self.leaves(what, atoms)?;
                    // What the test is lowered to reads the token at the front of the value, which
                    // a value of any other type does not have: the load would be through a number
                    // or an address laid out another way.
                    if !self.declared.has_cases(subject)? {
                        bail!(
                            "{}: {what} tests which case {} is, and only a union or a sum has cases \
                             to test: the two halves disagree",
                            self.owner,
                            subject.spelt()
                        );
                    }
                    self.fits(
                        &format!("a case {what} tests is a case of the value it tests"),
                        &Ty::Union {
                            union: atoms.clone(),
                        },
                        subject,
                    );
                }
                Selects::Held | Selects::Nothing if !optional => bail!(
                    "{}: {what} tests whether {} holds a value, which only an optional does: the \
                     two halves disagree",
                    self.owner,
                    subject.spelt()
                ),
                Selects::Held | Selects::Nothing => {}
            }
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

    /// The helper a call reaches, and what the call settles each variable the helper leaves open
    /// to: read off what it hands each parameter and what it answers, against what the helper takes
    /// and answers. A helper leaving nothing open is held to what it takes and answers exactly.
    ///
    /// Nothing is worked out here that the checker did not write: what each argument and the call
    /// were settled at is on the call, and this only reads where a variable stands in the one what
    /// stands there in the other. So a call whose types do not fit the helper's that way is the two
    /// halves disagreeing.
    fn helper_called(
        &self,
        reached: &str,
        arguments: &[Node],
        answers: &Ty,
    ) -> Result<(&'a Held, crate::specialize::Substitution)> {
        let held = *self
            .reached
            .helpers
            .get(&(self.carrier.module(), reached))
            .ok_or_else(|| {
                anyhow!(
                    "{}: a call of {reached}, which {} holds no copy of",
                    self.owner,
                    self.carrier.module()
                )
            })?;
        self.arity(
            &format!("a call of {reached}"),
            arguments.len(),
            held.parameters.len(),
        )?;
        let handed: Vec<&Ty> = arguments.iter().map(Node::ty).collect();
        let bound = crate::specialize::called(held, &handed, answers).ok_or_else(|| {
            anyhow!(
                "{}: a call of {reached} is handed {} and answers {}, where it takes {} and \
                 answers {}: the two halves disagree",
                self.owner,
                spelt(&handed.iter().map(|it| (*it).clone()).collect::<Vec<_>>()),
                answers.spelt(),
                spelt(&held.takes()),
                held.answers().spelt()
            )
        })?;
        Ok((held, bound))
    }

    /// A call's type against what it reaches answers, and how many values it hands over against
    /// how many that takes. What each argument stands as is [`Walk::slots`]'s.
    fn call(
        &mut self,
        reaches: &Reaches,
        arguments: &[Node],
        ty: &Ty,
        aborts: &[AbortKind],
    ) -> Result<()> {
        let (callee, takes) = self.parameters(reaches, arguments, ty)?;
        self.arity(&format!("a call of {callee}"), arguments.len(), takes.len())?;
        match reaches {
            Reaches::Behavior { declared } => self.same(
                &format!("a call of {declared}"),
                ty,
                &self.targets.named(declared)?.answers(),
                "what it answers",
            ),
            // What it answers stands where its variables are bound from, so a call answering
            // other than what the helper answers was refused where they were bound.
            Reaches::Helper { reached: _ } => Ok(()),
            Reaches::Value { module, name } => {
                let joined = format!("{module}.{name}");
                let value = self.reached.values[&(self.carrier.module(), joined.clone())];
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
            Reaches::Kernel {
                kernel,
                takes,
                fact,
            } => match LoweredKernel::of(kernel) {
                // A kernel this backend lowers is held to what this backend knows of it, and the
                // settlement is held to that: what the application says it takes is the checker's
                // statement about this call, and not a contract this backend has for the kernel.
                Some(known) => {
                    let contract = known.contract();
                    if takes.len() != contract.takes.len() {
                        bail!(
                            "{}: {kernel} takes {} arguments and this application says it takes \
                             {}: the two halves disagree",
                            self.owner,
                            contract.takes.len(),
                            takes.len()
                        );
                    }
                    // What was settled beside what it takes is held to the same contract: a fact
                    // the checker attaches to another kernel is not one it attaches to this.
                    if !contract.fact.accepts(fact) {
                        bail!(
                            "{}: an application of {kernel} settles {fact:?} where the kernel \
                             settles {:?}: the two halves disagree",
                            self.owner,
                            contract.fact
                        );
                    }
                    // What it takes binds the contract's variables, once each, and what it
                    // answers is then the one type the contract says it answers.
                    let mut bound = Bound::default();
                    for (settled, known_to_take) in takes.iter().zip(&contract.takes) {
                        if !known_to_take.binds(settled, &mut bound) {
                            bail!(
                                "{}: what an application of {kernel} takes is typed {} and what \
                                 it takes is {}: the two are one fact crossed twice and this \
                                 document's disagree",
                                self.owner,
                                settled.spelt(),
                                known_to_take.spelt()
                            );
                        }
                    }
                    self.ends_for(&format!("a call of {kernel}"), aborts, &contract.aborts)?;
                    let Some(answers) = contract.answers.settled(&bound) else {
                        unreachable!("what {kernel} takes binds every variable of what it answers");
                    };
                    self.same(
                        &format!("a call of {kernel}"),
                        ty,
                        &answers,
                        "what it answers",
                    )
                }
                // Refused where it is lowered; nothing here knows what it answers.
                None => Ok(()),
            },
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
        // A body's parameters are the target's inputs, one for one, and the body stands as what
        // the target says it answers: where it answers a case of it, the checker says so with a
        // `Widen` at its root.
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
            if body.ty() != &target.answers() {
                bail!(
                    "{name}'s body is typed {} and its target answers {}: the body stands as \
                     what the behavior declares, and the two halves disagree",
                    body.ty().spelt(),
                    target.answers().spelt()
                );
            }
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
        // What runs is offered by its cases exactly where it is a declared type or a union, and
        // whole otherwise: the checker's own rule (`PipelineSigs`, over `TypeOps.isDataLike`).
        // Routed on cases, it is tested by the token at its front, which nothing else has.
        let routed = matches!(stage.routing, Routing::OnCases { .. });
        if routed != says_its_case(&running) {
            bail!(
                "{name}'s stage {} is {} {}, and what runs is offered by its cases exactly where it \
                 is a declared type or a union: the two halves disagree",
                stage.behavior,
                if routed {
                    "routed on the cases of"
                } else {
                    "handed whole"
                },
                running.spelt()
            );
        }
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
                // A stage accepting a case no declaration names is one the checker's own backend
                // does not compile yet, so nothing has run one: it is not lowered until something
                // can hold what it answers to what the language says.
                if let Some(case) = accepted
                    .iter()
                    .find(|case| !matches!(case, Case::Declared { .. }))
                {
                    owed.not_lowered.push(format!(
                        "{name}'s stage {}, routed the case {} of {}",
                        stage.behavior,
                        case.spelt(),
                        running.spelt()
                    ));
                }
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

/// That what has to hold is a truth, and builds no value.
///
/// A declaration's clause observes the value being built, and a rule a behavior's answer is held to
/// observes the answer, and the checker refuses either one that constructs, through a helper as much
/// as written out (spec §invariant-expressions, which §ensures reads unchanged). So every value built
/// here is built by a body, and a construction runs clauses that build nothing in turn. A unit's
/// value is not asked about: it is built from no fields and runs no clause, and is laid out where it
/// stands (`Construction`), so naming one calls no constructor — which is what `Runs` rests on when
/// it settles what is built from the modules' bodies alone.
fn holds_and_builds_nothing(owner: &str, condition: &Node) -> Result<()> {
    let truth = Ty::Prim { prim: Prim::Bool };
    if condition.ty() != &truth {
        bail!(
            "{owner} is typed {}, where a clause is a truth",
            condition.ty().spelt()
        );
    }
    let mut built = None;
    condition.each(&mut |node| {
        if let Node::Construct { declared, .. } | Node::Attempt { declared, .. } = node {
            built.get_or_insert(declared);
        }
    });
    if let Some(declared) = built {
        bail!(
            "{owner} constructs {declared}, where a clause builds no value: the two halves \
             disagree"
        );
    }
    Ok(())
}

/// That where a behavior's answer is held to what it declares is a place the behavior has.
///
/// The checker decides it for every behavior of a module it checked, and for no other: another
/// build's behavior is one nobody here decided about, and one of this document's own is one
/// somebody did. Where it is decided, a check at the callee needs a callee whose answer is this
/// object's to hold, and a check at each crossing an answer that arrives from outside; a composition
/// carries no rule at all (spec §a-composition-carries-no-ensures).
fn placed(name: &str, target: &Target, decided_here: bool) -> Result<()> {
    let placement = match &target.ensures {
        Ensures::Callee { .. } => "at the callee",
        Ensures::Crossing { .. } => "where it crosses in",
        Ensures::None => "nowhere, since it declares nothing",
        Ensures::Undecided => "",
    };
    match (&target.ensures, decided_here, target.is) {
        (Ensures::Undecided, false, _) => {}
        (Ensures::Undecided, true, _) => bail!(
            "{name} is declared by {}, which this document builds, and nothing decided where its \
             answer is held to what it declares: the two halves disagree",
            target.module
        ),
        (_, false, _) => bail!(
            "{name} is another build's, and this document says its answer is held {placement}: \
             the two halves disagree"
        ),
        (Ensures::None, true, _)
        | (Ensures::Callee { .. }, true, Answers::Body | Answers::Unwritten)
        | (Ensures::Crossing { .. }, true, Answers::Injected | Answers::Unwritten) => {}
        (Ensures::Callee { .. } | Ensures::Crossing { .. }, true, is) => bail!(
            "{name} answers as {is:?} and its answer is held {placement}: the two halves disagree"
        ),
    }
    // That the clause relates the parameters the behavior takes is held where a target is read.
    Ok(())
}

/// Each field of `declaration` under the binding its clauses read it through, at the type it holds.
///
/// Two fields under one binding would be a clause reading one name for two values.
fn fields_bound(declaration: &Declaration) -> Vec<(usize, Ty)> {
    declaration
        .fields()
        .iter()
        .map(|field| (field.binding, field.codec.ty()))
        .collect()
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
