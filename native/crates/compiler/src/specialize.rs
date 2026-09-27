//! A helper whose body leaves type variables open, made into one function for each set of types a
//! call needs.
//!
//! What the language leaves to a backend (ADR-0092): a recursive helper is not expanded at its call
//! sites and stays one definition over `'a`, and the instantiation of a variable belongs to the
//! call. On the JVM that is absorbed by the representation a value runs in. Here every layout, every
//! comparison and every codec is chosen by type, so what a body over `'a` does with a value of `'a`
//! has no lowering until `'a` is a type, and what is lowered is a copy of it for each set of types
//! some call hands it.
//!
//! Nothing here works a type out. A call already carries what its arguments and its answer were
//! settled at, and the helper's parameters and answer say where each variable stands in those, so
//! what a variable comes to is read off the one against the other ([`Substitution::binds`]). A
//! variable no parameter and no answer reaches is one no call settles, and it stays as written in
//! the copy. That is no refusal of the helper: a list held over it is a pointer whatever it holds,
//! and `None` of it is a tag. What asks of it what it is, a layout, a comparison or a form, refuses
//! there ([`crate::open_type`]), so what is refused is decided by what asks and not by where a
//! variable stands.
//!
//! A copy is keyed by the helper and what its variables come to, so every call needing the same
//! types reaches one function. A copy's body is the helper's with its variables replaced, and a call
//! inside it is resolved the same way, so the self-call of `foldFrom<Int, Int>` reaches
//! `foldFrom<Int, Int>` again. Which copy a call reaches is answered here, once, by the call it is;
//! the lowering reads that answer and does not work it out a second time from a name and some types.
//!
//! A recursion is no different: the copies are made as calls reach them, and a call inside a copy
//! that reaches a copy already made reaches that one. `h<'a, 'b>` calling `h<'b, 'a>` is two copies
//! calling one another, and `f<'a>` calling `f<Int>` is at most two, so any recursion that reaches
//! finitely many sets of types closes by itself. What does not is `f<'a>` calling `f<List<'a>>`,
//! which needs a copy after every copy. A copy remembers the copy whose body made it, and one that
//! would be the [`DEPTH`]th of its helper along that chain, or the [`REENTERED`]th made from a copy
//! of its own helper anywhere, is refused as not converging. That is a
//! limit of this backend on what it spends and no statement about the language: what a copy is
//! made for is not affected by it, and a program under it is lowered the same at any other bound.
//!
//! A call a copy makes to itself is a call to the same function, which is what lets it be a jump
//! ([`Specializations::callee`] is asked, and only a call reaching the copy it stands in is one).
//! No helper the standard library writes needs more, and no model can write a type variable.

use crate::index;
use crate::transport::{Body, Carrier, Held, Node, Owner, Reaches, Reference, Ty};
use crate::{Lowered, Runs, not_lowered};

/// How many copies of one helper may be made one from the body of another before the making is
/// taken not to converge.
const DEPTH: usize = 16;

/// How many copies of one helper may be made from the body of a copy of itself, over the whole
/// program. A chain is bounded by [`DEPTH`], but a body calling itself at two growing types makes
/// two copies at each step, which is two to the depth of them before the chain ends.
const REENTERED: usize = 64;
use std::collections::HashMap;

/// A helper, by the module holding the copy and the reference a call there reaches it by.
type HelperKey<'p> = (&'p str, &'p Reference);

/// What each variable of a helper comes to at one call, by the variable's number.
#[derive(Default, Debug)]
pub(crate) struct Substitution(Vec<Option<Ty>>);

impl Substitution {
    /// Whether `actual` is `template` with each variable of `template` standing for a type,
    /// binding a variable the first time it is met and holding it to that every time after.
    ///
    /// Only what is written in the two is compared: a variable binds whatever stands where it
    /// stands, and anything else has to be the same type made the same way. Nothing is widened and
    /// nothing is inferred, since what stands in `actual` is what the checker settled.
    pub(crate) fn binds(&mut self, template: &Ty, actual: &Ty) -> bool {
        // A template that writes no variable binds nothing, and is the same type or not: asked
        // whole, so a type added to the document is compared as itself and not by an arm each
        // kind of type has to be given here.
        if !template.is_open() {
            return template == actual;
        }
        match (template, actual) {
            (Ty::Var { var }, _) => {
                if self.0.len() <= *var {
                    self.0.resize(var + 1, None);
                }
                match &self.0[*var] {
                    Some(already) => already == actual,
                    None => {
                        self.0[*var] = Some(actual.clone());
                        true
                    }
                }
            }
            (Ty::Option { option: held }, Ty::Option { option: also })
            | (Ty::List { list: held }, Ty::List { list: also })
            | (Ty::Set { set: held }, Ty::Set { set: also }) => self.binds(held, also),
            (Ty::Tuple { tuple }, Ty::Tuple { tuple: also }) => {
                tuple.len() == also.len() && tuple.iter().zip(also).all(|(t, a)| self.binds(t, a))
            }
            (Ty::Fn { fn_ }, Ty::Fn { fn_: also }) => {
                fn_.takes.len() == also.takes.len()
                    && fn_
                        .takes
                        .iter()
                        .zip(&also.takes)
                        .all(|(t, a)| self.binds(t, a))
                    && self.binds(&fn_.answers, &also.answers)
            }
            (Ty::Map { map }, Ty::Map { map: also }) => {
                self.binds(&map.key, &also.key) && self.binds(&map.value, &also.value)
            }
            // What holds no type writes no variable, and was compared whole above.
            (
                Ty::Prim { .. }
                | Ty::Ref { .. }
                | Ty::Union { .. }
                | Ty::Nothing { .. }
                | Ty::Never { .. },
                _,
            ) => unreachable!("a template writing no variable is compared whole"),
            (
                Ty::Option { .. }
                | Ty::List { .. }
                | Ty::Set { .. }
                | Ty::Tuple { .. }
                | Ty::Fn { .. }
                | Ty::Map { .. },
                _,
            ) => false,
        }
    }

    /// `ty` with each variable replaced by what it came to, where every variable it writes is bound.
    pub(crate) fn applied(&self, ty: &Ty) -> Option<Ty> {
        self.replaced(ty, false)
    }

    /// `ty` with each variable a call settled replaced by what it came to, and each one none did
    /// left as written. What asks what a variable is refuses there ([`crate::open_type`]); a body
    /// that never asks is lowered as it is.
    pub(crate) fn partly_applied(&self, ty: &Ty) -> Ty {
        self.replaced(ty, true)
            .expect("a variable left as written is not a reason to give no type")
    }

    fn replaced(&self, ty: &Ty, leaving: bool) -> Option<Ty> {
        let each = |ty: &Ty| self.replaced(ty, leaving);
        Some(match ty {
            Ty::Var { var } => match self.0.get(*var).cloned().flatten() {
                Some(came_to) => came_to,
                None if leaving => ty.clone(),
                None => return None,
            },
            Ty::Prim { .. }
            | Ty::Ref { .. }
            | Ty::Union { .. }
            | Ty::Nothing { .. }
            | Ty::Never { .. } => ty.clone(),
            Ty::Option { option } => Ty::Option {
                option: Box::new(each(option)?),
            },
            Ty::List { list } => Ty::List {
                list: Box::new(each(list)?),
            },
            Ty::Set { set } => Ty::Set {
                set: Box::new(each(set)?),
            },
            Ty::Tuple { tuple } => Ty::Tuple {
                tuple: tuple.iter().map(each).collect::<Option<_>>()?,
            },
            Ty::Fn { fn_ } => Ty::Fn {
                fn_: crate::transport::FnSignature {
                    takes: fn_.takes.iter().map(each).collect::<Option<_>>()?,
                    answers: Box::new(each(&fn_.answers)?),
                },
            },
            Ty::Map { map } => Ty::Map {
                map: crate::transport::MapTy {
                    key: Box::new(each(&map.key)?),
                    value: Box::new(each(&map.value)?),
                },
            },
        })
    }

    /// What each of the first `count` variables came to, `None` for one nothing bound.
    fn by_number(&self, count: usize) -> Vec<Option<Ty>> {
        (0..count)
            .map(|at| self.0.get(at).cloned().flatten())
            .collect()
    }

    fn of(types: &[Option<Ty>]) -> Self {
        Substitution(types.to_vec())
    }
}

/// What a call of `held` settles its variables to, read off what it hands each parameter and what
/// it answers. `None` where the two do not fit what the helper takes and answers, which [`Coherent`]
/// refuses as the two halves disagreeing.
///
/// [`Coherent`]: crate::coherent::Coherent
pub(crate) fn called(held: &Held, handed: &[&Ty], answers: &Ty) -> Option<Substitution> {
    if handed.len() != held.parameters.len() {
        return None;
    }
    let mut bound = Substitution::default();
    for (parameter, argument) in held.parameters.iter().zip(handed) {
        if !bound.binds(&parameter.ty, argument) {
            return None;
        }
    }
    bound.binds(held.answers(), answers).then_some(bound)
}

/// One copy of a helper, by where it is in [`Specializations`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct InstanceId(usize);

/// A helper with each of its variables replaced by one type: a function the lowering defines.
pub(crate) struct Instance<'p> {
    /// Where the helper stands, which is where a call from its body is resolved.
    pub carrier: Carrier<'p>,
    pub held: &'p Held,
    /// What each variable came to, by its number. Empty for a helper that leaves none open, and
    /// `None` for a variable no call settles, which the copy leaves as written.
    pub types: Vec<Option<Ty>>,
    /// Which copy of the helper this is, counted from nought among the copies of the one helper,
    /// which is what tells the functions of one helper apart.
    pub ordinal: usize,
    /// The copy whose body a call made this one, where a call in one did.
    parent: Option<InstanceId>,
    body: Settled<'p>,
}

/// A copy's body: the helper's own where it leaves nothing open, and the helper's rewritten
/// otherwise. Boxed, so that where a call node stands does not move when another copy is added.
enum Settled<'p> {
    AsHeld(&'p Node),
    Rewritten(Box<Node>),
}

impl<'p> Instance<'p> {
    pub fn body(&self) -> &Node {
        match &self.body {
            Settled::AsHeld(node) => node,
            Settled::Rewritten(node) => node,
        }
    }

    /// What it takes, in the order its parameters are bound.
    pub fn takes(&self) -> Vec<Ty> {
        let settled = Substitution::of(&self.types);
        self.held
            .parameters
            .iter()
            .map(|parameter| settled.partly_applied(&parameter.ty))
            .collect()
    }

    /// What it answers: its body's type.
    pub fn answers(&self) -> &Ty {
        self.body().ty()
    }
}

/// Every copy of a helper this object defines, and which of them each call reaches.
pub(crate) struct Specializations<'p> {
    instances: Vec<Instance<'p>>,
    by_key: HashMap<(&'p str, &'p Reference, Vec<Option<Ty>>), InstanceId>,
    /// How many copies of each helper were made from the body of a copy of the same helper.
    reentered: HashMap<HelperKey<'p>, usize>,
    /// How many copies of each helper are made, which is the ordinal of the next.
    made: HashMap<HelperKey<'p>, usize>,
    /// Which copy each call reaching a helper reaches, by where the call stands.
    reached: HashMap<*const Node, InstanceId>,
}

impl<'p> Specializations<'p> {
    /// Every copy a body this object runs needs, and every copy those need in turn.
    ///
    /// A helper leaving nothing open is one copy, defined whether a call reaches it or not, as it
    /// always was. One leaving variables open is a copy for each set of types a call hands it, and
    /// nothing where no call does.
    pub fn of(runs: &Runs<'p>) -> Lowered<Self> {
        let mut specializations = Specializations {
            instances: Vec::new(),
            by_key: HashMap::new(),
            reentered: HashMap::new(),
            made: HashMap::new(),
            reached: HashMap::new(),
        };
        let mut helpers: HashMap<HelperKey<'p>, Body<'p>> = HashMap::new();
        for body in runs.bodies() {
            if let Some(held) = body.owner.helper() {
                index::unique(&mut helpers, (body.carrier().module(), &held.reached), body);
            }
        }
        let mut roots = Vec::new();
        for body in runs.bodies() {
            match body.owner {
                Owner::Helper(held) if held.variables() == 0 => {
                    specializations.copy(None, body.carrier(), held, Vec::new())?;
                }
                // A helper over variables is lowered as its copies and never as itself.
                Owner::Helper(_) => {}
                Owner::Value(_)
                | Owner::Entry(_)
                | Owner::Definition(_)
                | Owner::Example(_)
                | Owner::StoodIn { .. }
                | Owner::Invariant { .. }
                | Owner::Ensures { .. } => roots.push(body),
            }
        }
        for body in roots {
            specializations.resolve(&helpers, None, body.carrier(), calls_in(body.node))?;
        }
        // Each copy's body, once, in the order the copies were made: a copy made while one is
        // read is read after it.
        let mut at = 0;
        while at < specializations.instances.len() {
            let instance = &specializations.instances[at];
            let carrier = instance.carrier;
            let calls = calls_in(instance.body());
            specializations.resolve(&helpers, Some(InstanceId(at)), carrier, calls)?;
            at += 1;
        }
        Ok(specializations)
    }

    /// Every call reaching a helper in `node`, each to the copy it needs.
    fn resolve(
        &mut self,
        helpers: &HashMap<HelperKey<'p>, Body<'p>>,
        parent: Option<InstanceId>,
        carrier: Carrier<'p>,
        calls: Vec<Called>,
    ) -> Lowered<()> {
        for call in calls {
            let helper = helpers
                .get(&(carrier.module(), &call.reached))
                .expect("`Coherent` held every helper a call reaches to be one its module holds");
            let Owner::Helper(held) = helper.owner else {
                unreachable!("gathered from the helpers' bodies alone");
            };
            let handed: Vec<&Ty> = call.handed.iter().collect();
            let bound = called(held, &handed, &call.answers).expect(
                "`Coherent` held every call of a helper to fit what the helper takes, and \
                     refused one that fits only through `Nothing` wherever it is lowered, which is \
                     everywhere this reads (`unrun::each_lowered`)",
            );
            let types = bound.by_number(held.variables());
            let id = self.copy(parent, helper.carrier(), held, types)?;
            index::unique(&mut self.reached, call.at, id);
        }
        Ok(())
    }

    /// The copy of `held` whose variables come to `types`, made where there is none yet.
    fn copy(
        &mut self,
        parent: Option<InstanceId>,
        carrier: Carrier<'p>,
        held: &'p Held,
        types: Vec<Option<Ty>>,
    ) -> Lowered<InstanceId> {
        let key = (carrier.module(), &held.reached, types);
        if let Some(id) = self.by_key.get(&key) {
            return Ok(*id);
        }
        let (_, _, types) = key;
        // How many of this helper's copies the chain of copies each made from the body of the one
        // before already holds, this one not yet among them.
        let mut along = 0;
        let mut at = parent;
        while let Some(id) = at {
            let ancestor = &self.instances[id.0];
            if ancestor.carrier == carrier && std::ptr::eq(ancestor.held, held) {
                along += 1;
            }
            at = ancestor.parent;
        }
        if along > 0 {
            let made = self
                .reentered
                .entry((carrier.module(), &held.reached))
                .or_insert(0);
            *made += 1;
            if *made > REENTERED {
                return Err(not_lowered(format!(
                    "{}, whose copies would be made one from the body of another without end: \
                     the types it is called at keep growing",
                    held.reached.rendered()
                )));
            }
        }
        if along >= DEPTH {
            return Err(not_lowered(format!(
                "{}, whose copies would be made one from the body of another without end: the \
                 types it is called at keep growing",
                held.reached.rendered()
            )));
        }
        let body = if types.is_empty() {
            Settled::AsHeld(&held.body)
        } else {
            let mut body = held.body.clone();
            settle(&mut body, &Substitution::of(&types));
            Settled::Rewritten(Box::new(body))
        };
        let ordinal = {
            let made = self
                .made
                .entry((carrier.module(), &held.reached))
                .or_insert(0);
            *made += 1;
            *made - 1
        };
        let id = InstanceId(self.instances.len());
        self.instances.push(Instance {
            carrier,
            held,
            types: types.clone(),
            ordinal,
            parent,
            body,
        });
        index::unique(
            &mut self.by_key,
            (carrier.module(), &held.reached, types),
            id,
        );
        Ok(id)
    }

    /// Every copy, in the order it was made.
    pub fn iter(&self) -> impl Iterator<Item = (InstanceId, &Instance<'p>)> {
        self.instances
            .iter()
            .enumerate()
            .map(|(at, instance)| (InstanceId(at), instance))
    }

    /// The copy `call`, a call reaching a helper in a body this object lowers, reaches.
    pub fn callee(&self, call: &Node) -> InstanceId {
        *self
            .reached
            .get(&std::ptr::from_ref(call))
            .expect("every call reaching a helper in a body lowered here was resolved to a copy")
    }
}

/// A call reaching a helper, by where it stands, with what it names and the types it was settled
/// at: read out of a body before the copies it needs are made, since making one may add the body
/// of another to what is being read.
struct Called {
    at: *const Node,
    reached: Reference,
    handed: Vec<Ty>,
    answers: Ty,
}

/// Every call reaching a helper that is lowered where `node` is, a function value's body included:
/// a call only in the step of a walk that never runs needs no copy, and none is made for it.
fn calls_in(node: &Node) -> Vec<Called> {
    let mut calls = Vec::new();
    crate::unrun::each_lowered(node, &mut |node| {
        if let Node::Call {
            reaches: Reaches::Helper { reached },
            arguments,
            ty,
            ..
        } = node
        {
            calls.push(Called {
                at: std::ptr::from_ref(node),
                reached: reached.clone(),
                handed: arguments.iter().map(|it| it.ty().clone()).collect(),
                answers: ty.clone(),
            });
        }
    });
    calls
}

/// Every type `node` writes, and every node lowered under it writes, with its variables replaced.
///
/// A variable no call settles is left as it is written ([`Substitution::partly_applied`]), and so
/// is a function a call never applies: nothing reads it again.
fn settle(node: &mut Node, settled: &Substitution) {
    for ty in node.types_mut() {
        *ty = settled.partly_applied(ty);
    }
    for child in crate::unrun::lowered_children_mut(node) {
        settle(child, settled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{FnSignature, Prim};

    fn string() -> Ty {
        Ty::Prim { prim: Prim::String }
    }

    fn int() -> Ty {
        Ty::Prim { prim: Prim::Int }
    }

    fn var(var: usize) -> Ty {
        Ty::Var { var }
    }

    fn list(of: Ty) -> Ty {
        Ty::List { list: Box::new(of) }
    }

    use crate::coherent::Coherent;
    use crate::transport::Program;

    const FOLDING: &str = include_str!("../tests/folding.transport.json");

    /// What `document` makes into copies, or why it makes none.
    fn specialized<T>(
        document: &str,
        asked: impl FnOnce(&Specializations, &Program) -> T,
    ) -> anyhow::Result<T> {
        let program = Program::read(document)?;
        let coherent = Coherent::of(&program)?;
        let specializations = Specializations::of(&coherent.runs)?;
        Ok(asked(&specializations, &program))
    }

    /// Every call reaching a helper in `node`.
    fn calls(node: &Node) -> Vec<&Node> {
        let mut found = Vec::new();
        node.each_written(&mut |node| {
            if let Node::Call {
                reaches: Reaches::Helper { .. },
                ..
            } = node
            {
                found.push(node);
            }
        });
        found
    }

    /// A module `m` holding `helpers` and building the one value `v`, whose body is `body`.
    fn holding(helpers: &[String], body: &str) -> String {
        format!(
            r#"{{"transport":27,"declarations":[],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{}],"values":[{{"module":"m","name":"v","handovers":[],"body":{body}}}],"entries":[],"definitions":[],"examples":[]}}]}}"#,
            helpers.join(",")
        )
    }

    fn helper(reached: &str, takes: &[&str], body: &str) -> String {
        let parameters: Vec<String> = takes
            .iter()
            .enumerate()
            .map(|(at, ty)| format!(r#"{{"name":"p{at}","type":{ty}}}"#))
            .collect();
        format!(
            r#"{{"reached":{},"parameters":[{}],"body":{body}}}"#,
            own(reached),
            parameters.join(",")
        )
    }

    /// A declaration of `m`, written `m.name`, as `m` reaches it: as its own.
    fn own(reached: &str) -> String {
        let (module, name) = reached
            .rsplit_once('.')
            .expect("a helper written module.name");
        format!(r#"{{"is":"own","module":"{module}","name":"{name}"}}"#)
    }

    fn node(core: &str, fields: &str, ty: &str) -> String {
        let fields = if fields.is_empty() {
            String::new()
        } else {
            format!("{fields},")
        };
        format!(r#"{{"core":"{core}",{fields}"type":{ty},"aborts":[]}}"#)
    }

    fn read(binding: usize, ty: &str) -> String {
        node("read", &format!(r#""binding":{binding}"#), ty)
    }

    fn call(reached: &str, arguments: &[String], ty: &str) -> String {
        node(
            "call",
            &format!(
                r#""reaches":{{"is":"helper","reached":{}}},"arguments":[{}]"#,
                own(reached),
                arguments.join(",")
            ),
            ty,
        )
    }

    const INT: &str = r#"{"prim":"INT"}"#;
    const STRING: &str = r#"{"prim":"STRING"}"#;
    const VAR: &str = r#"{"var":0}"#;

    fn number(value: i64) -> String {
        node("int", &format!(r#""value":{value}"#), INT)
    }

    fn text(value: &str) -> String {
        node("string", &format!(r#""value":"{value}""#), STRING)
    }

    fn tuple(members: &[String], types: &[&str]) -> String {
        node(
            "tuple",
            &format!(r#""members":[{}]"#, members.join(",")),
            &format!(r#"{{"tuple":[{}]}}"#, types.join(",")),
        )
    }

    /// `List.foldFrom` called at `Int` twice and at `String` once is two copies, and each copy's
    /// call to itself reaches that copy.
    #[test]
    fn a_fold_is_one_copy_for_each_set_of_types_it_is_called_at() {
        specialized(FOLDING, |specializations, _| {
            let copies: Vec<_> = specializations.iter().collect();
            let int = Ty::Prim { prim: Prim::Int };
            let text = Ty::Prim { prim: Prim::String };
            let types: Vec<&[Option<Ty>]> =
                copies.iter().map(|(_, it)| it.types.as_slice()).collect();
            let (int, text) = (Some(int), Some(text));
            assert_eq!(
                types,
                [&[int.clone(), int.clone()][..], &[text, int][..]],
                "one copy for each set of types"
            );
            for (id, copy) in &copies {
                assert_eq!(copy.held.reached.rendered(), "List.foldFrom");
                let own = calls(copy.body());
                assert_eq!(own.len(), 1, "foldFrom calls itself once");
                assert_eq!(
                    specializations.callee(own[0]),
                    *id,
                    "and reaches its own copy"
                );
                let mut open = 0;
                copy.body().each_written(&mut |node| {
                    open += node.types().iter().filter(|ty| ty.is_open()).count();
                });
                assert_eq!(open, 0, "no variable is left in a copy");
            }
        })
        .expect("the writer's own document");
    }

    /// Two calls at the same types reach one copy.
    #[test]
    fn two_folds_at_one_set_of_types_reach_one_copy() {
        specialized(FOLDING, |specializations, program| {
            let mut reached = Vec::new();
            for definition in &program.modules[0].definitions {
                let crate::transport::Definition::Body { body, .. } = definition else {
                    continue;
                };
                for call in calls(body) {
                    reached.push(specializations.callee(call));
                }
            }
            assert_eq!(reached.len(), 3);
            assert_eq!(
                reached[0], reached[1],
                "summed and again fold at Int into Int"
            );
            assert_ne!(reached[0], reached[2], "spelt folds at Int into String");
        })
        .expect("the writer's own document");
    }

    /// A variable is a helper's own: `0` in one helper and `0` in another come to different types.
    #[test]
    fn two_helpers_numbering_a_variable_alike_do_not_share_it() {
        let helpers = [
            helper("m.first", &[VAR], &read(0, VAR)),
            helper("m.second", &[VAR], &read(0, VAR)),
        ];
        let body = tuple(
            &[
                call("m.first", &[number(1)], INT),
                call("m.second", &[text("a")], STRING),
            ],
            &[INT, STRING],
        );
        specialized(&holding(&helpers, &body), |specializations, _| {
            let copies: Vec<(String, Vec<Option<Ty>>)> = specializations
                .iter()
                .map(|(_, it)| (it.held.reached.rendered(), it.types.clone()))
                .collect();
            assert_eq!(
                copies,
                [
                    (
                        "first".to_string(),
                        vec![Some(Ty::Prim { prim: Prim::Int })]
                    ),
                    (
                        "second".to_string(),
                        vec![Some(Ty::Prim { prim: Prim::String })]
                    ),
                ]
            );
        })
        .expect("a document every relation of which holds");
    }

    /// The copies of each helper of the program, by the helper and what its variables came to, in
    /// the order they were made.
    fn copies_of(document: &str) -> anyhow::Result<Vec<(String, Vec<Option<Ty>>)>> {
        specialized(document, |specializations, _| {
            specializations
                .iter()
                .map(|(_, it)| (it.held.reached.rendered(), it.types.clone()))
                .collect()
        })
    }

    /// A helper handing itself a list of what it was handed would need a copy after every copy, and
    /// is refused as not converging, at the copy after the last that is made.
    #[test]
    fn a_helper_calling_itself_at_ever_larger_types_is_not_lowered() {
        let listed = r#"{"list":{"var":0}}"#;
        let nested = node("list", &format!(r#""elements":[{}]"#, read(0, VAR)), listed);
        let helpers = [helper("m.nest", &[VAR], &call("m.nest", &[nested], INT))];
        let refused = specialized(
            &holding(&helpers, &call("m.nest", &[number(1)], INT)),
            |_, _| (),
        )
        .expect_err("no number of copies is enough");
        assert!(
            refused.downcast_ref::<crate::NotLowered>().is_some(),
            "{refused}"
        );
        assert!(refused.to_string().contains("without end"), "{refused}");
    }

    /// A body calling itself at two growing types makes two copies at each step: refused at a count
    /// of copies and not only at a depth, which would be two to the depth of them.
    #[test]
    fn a_helper_calling_itself_at_two_growing_types_is_refused_without_making_them_all() {
        let listed = r#"{"list":{"var":0}}"#;
        let optional = r#"{"option":{"var":0}}"#;
        let a = node("list", &format!(r#""elements":[{}]"#, read(0, VAR)), listed);
        let b = node("some", &format!(r#""value":{}"#, read(0, VAR)), optional);
        let both = node(
            "let",
            &format!(
                r#""binding":9,"binds":{INT},"value":{},"body":{}"#,
                call("m.g", &[a], INT),
                call("m.g", &[b], INT)
            ),
            INT,
        );
        let helpers = [helper("m.g", &[VAR], &both)];
        let started = std::time::Instant::now();
        let refused = specialized(
            &holding(&helpers, &call("m.g", &[number(1)], INT)),
            |_, _| (),
        )
        .expect_err("no number of copies is enough");
        assert!(refused.to_string().contains("without end"), "{refused}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "refused after {:?}",
            started.elapsed()
        );
    }

    /// A function value written in a helper over variables is in each copy of it, at the types that
    /// copy has: two copies, and no refusal of the helper for holding one.
    #[test]
    fn a_function_value_inside_a_helper_over_variables_is_in_each_copy() {
        let made = r#"{"fn":{"takes":[],"answers":{"var":0}}}"#;
        let block = node(
            "block",
            &format!(r#""site":0,"parameters":[],"body":{}"#, read(0, VAR)),
            made,
        );
        let helpers = [helper("m.k", &[VAR], &block)];
        let at = |ty: &str| format!(r#"{{"fn":{{"takes":[],"answers":{ty}}}}}"#);
        let both = tuple(
            &[
                call("m.k", &[number(1)], &at(INT)),
                call("m.k", &[text("a")], &at(STRING)),
            ],
            &[&at(INT), &at(STRING)],
        );
        let types = specialized(&holding(&helpers, &both), |specializations, _| {
            specializations
                .iter()
                .map(|(_, it)| it.types.clone())
                .collect::<Vec<_>>()
        })
        .expect("a function value is no reason to refuse the helper");
        assert_eq!(types, vec![vec![Some(int())], vec![Some(string())]]);
    }

    const OTHER: &str = r#"{"var":1}"#;

    /// `h<'a, 'b>` calling `h<'b, 'a>` closes over two copies, whichever of them a body asks for
    /// first and however many it asks for: the copy a call inside one reaches is the other, and
    /// the other's is the first.
    #[test]
    fn a_recursion_swapping_its_types_closes_over_two_copies() {
        let swapped = [helper(
            "m.h",
            &[VAR, OTHER],
            &call("m.h", &[read(1, OTHER), read(0, VAR)], INT),
        )];
        let int_text = call("m.h", &[number(1), text("a")], INT);
        let text_int = call("m.h", &[text("a"), number(1)], INT);
        for body in [
            int_text.clone(),
            tuple(&[int_text.clone(), text_int.clone()], &[INT, INT]),
            tuple(&[text_int, int_text], &[INT, INT]),
        ] {
            let mut copies: Vec<Vec<Option<Ty>>> = copies_of(&holding(&swapped, &body))
                .expect("a recursion that closes")
                .into_iter()
                .map(|(_, types)| types)
                .collect();
            copies.sort_by_key(|types| format!("{types:?}"));
            assert_eq!(
                copies,
                [
                    vec![Some(int()), Some(string())],
                    vec![Some(string()), Some(int())]
                ]
            );
        }
    }

    /// `f<'a>` calling `f<Int>` stops at two copies: the one it was entered at, and the one it
    /// settles to, which calls itself.
    #[test]
    fn a_recursion_settling_its_type_to_one_type_closes_over_two_copies() {
        let helpers = [helper("m.f", &[VAR], &call("m.f", &[number(1)], INT))];
        let copies = copies_of(&holding(&helpers, &call("m.f", &[text("a")], INT)))
            .expect("a recursion that closes");
        assert_eq!(
            copies,
            [
                ("f".to_string(), vec![Some(string())]),
                ("f".to_string(), vec![Some(int())])
            ]
        );
    }

    /// Two helpers calling each other, each handing the other its own variable of each number,
    /// run at the types they were entered at: one copy of each.
    #[test]
    fn a_recursion_through_two_helpers_at_its_own_types_is_one_copy_of_each() {
        let helpers = [
            helper("m.even", &[VAR], &call("m.odd", &[read(0, VAR)], INT)),
            helper("m.odd", &[VAR], &call("m.even", &[read(0, VAR)], INT)),
        ];
        specialized(
            &holding(&helpers, &call("m.even", &[text("a")], INT)),
            |specializations, _| {
                let copies: Vec<(String, Vec<Option<Ty>>)> = specializations
                    .iter()
                    .map(|(_, it)| (it.held.reached.rendered(), it.types.clone()))
                    .collect();
                let text = vec![Some(Ty::Prim { prim: Prim::String })];
                assert_eq!(
                    copies,
                    [
                        ("even".to_string(), text.clone()),
                        ("odd".to_string(), text)
                    ]
                );
            },
        )
        .expect("a recursion at its own types");
    }

    /// A helper leaving nothing open, in a recursion with one that does, calls it back at a type of
    /// its own choosing: a copy at that type, which calls back the one it was made from.
    #[test]
    fn a_recursion_calling_back_at_a_fixed_type_closes_over_the_copies_it_reaches() {
        let helpers = [
            helper("m.open", &[VAR], &call("m.shut", &[], INT)),
            helper("m.shut", &[], &call("m.open", &[number(1)], INT)),
        ];
        let copies = copies_of(&holding(&helpers, &call("m.open", &[text("a")], INT)))
            .expect("a recursion that closes");
        assert_eq!(
            copies,
            [
                ("shut".to_string(), vec![]),
                ("open".to_string(), vec![Some(string())]),
                ("open".to_string(), vec![Some(int())]),
            ]
        );
    }

    /// The writer numbers a helper's variables from nought as it meets each, and what reads one
    /// takes its number as a place in a table: a number past the ones below it is refused before
    /// anything is made that long.
    #[test]
    fn a_helper_numbering_its_variables_with_a_gap_is_the_halves_disagreeing() {
        let helpers = [helper("m.gap", &[OTHER], &read(0, OTHER))];
        let refused = specialized(
            &holding(&helpers, &call("m.gap", &[number(1)], INT)),
            |_, _| (),
        )
        .expect_err("a variable 1 with no variable 0");
        assert!(
            refused.downcast_ref::<crate::NotLowered>().is_none(),
            "{refused}"
        );
        assert!(
            refused.to_string().contains("numbers its type variables"),
            "{refused}"
        );
    }

    /// A variable nothing a call hands over or answers reaches is one no call settles, and a body
    /// that never asks what it is has no need of it: the copy is made, the variable left as written.
    #[test]
    fn a_variable_no_call_settles_is_left_open_where_nothing_asks_what_it_is() {
        let absent = node("none", "", r#"{"option":{"var":0}}"#);
        let body = node(
            "let",
            &format!(
                r#""binding":1,"binds":{{"option":{{"var":0}}}},"value":{absent},"body":{}"#,
                number(1)
            ),
            INT,
        );
        let helpers = [helper("m.h", &[INT], &body)];
        let (types, open) = specialized(
            &holding(&helpers, &call("m.h", &[number(1)], INT)),
            |specializations, _| {
                let mut open = 0;
                for (_, copy) in specializations.iter() {
                    copy.body().each_written(&mut |node| {
                        open += node.types().iter().filter(|ty| ty.is_open()).count();
                    });
                }
                (
                    specializations
                        .iter()
                        .map(|(_, it)| it.types.clone())
                        .collect::<Vec<_>>(),
                    open,
                )
            },
        )
        .expect("nothing asks what the variable is");
        assert_eq!(types, vec![vec![None]]);
        assert!(open > 0, "the variable is left as written");
    }

    /// A variable written only in a function a call never applies stands where nothing is lowered,
    /// so no call has to settle it: the copy is made, with that variable left as nothing settled.
    #[test]
    fn a_variable_only_a_function_never_applied_writes_is_not_asked_for() {
        let nothing = r#"{"nothing":{}}"#;
        let empty = format!(r#"{{"list":{nothing}}}"#);
        let step_ty = format!(r#"{{"fn":{{"takes":[{empty},{nothing}],"answers":{empty}}}}}"#);
        let block = node(
            "block",
            &format!(
                r#""site":0,"parameters":[{{"binding":2,"name":"acc"}},{{"binding":3,"name":"x"}}],"body":{}"#,
                read(2, &empty)
            ),
            &step_ty,
        );
        let absent = node("none", "", r#"{"option":{"var":0}}"#);
        let step = node(
            "let",
            &format!(
                r#""binding":1,"binds":{{"option":{{"var":0}}}},"value":{absent},"body":{block}"#
            ),
            &step_ty,
        );
        let walk = node(
            "call",
            &format!(
                r#""reaches":{{"is":"emitted","operation":"BUILD_LIST"}},"arguments":[{step},{},{}]"#,
                node("list", r#""elements":[]"#, &empty),
                number(0)
            ),
            &empty,
        );
        let helpers = [helper("m.h", &[INT], &walk)];
        let types = specialized(
            &holding(&helpers, &call("m.h", &[number(1)], &empty)),
            |specializations, _| {
                specializations
                    .iter()
                    .map(|(_, it)| it.types.clone())
                    .collect::<Vec<_>>()
            },
        )
        .expect("a copy that settles every variable it lowers");
        assert_eq!(types, vec![vec![None]]);
    }

    /// A variable stands in a helper's body and nowhere else.
    #[test]
    fn a_variable_outside_a_helper_is_the_halves_disagreeing() {
        let body = node("none", "", r#"{"option":{"var":0}}"#);
        let refused = specialized(&holding(&[], &body), |_, _| ())
            .expect_err("a value typed over a variable");
        assert!(
            refused.downcast_ref::<crate::NotLowered>().is_none(),
            "{refused}"
        );
        assert!(
            refused.to_string().contains("outside a helper"),
            "{refused}"
        );
    }

    /// A call handing a helper what does not fit what it takes, however its variables are bound.
    #[test]
    fn a_call_its_helper_cannot_be_bound_to_is_the_halves_disagreeing() {
        let pair = r#"{"tuple":[{"var":0},{"var":0}]}"#;
        let helpers = [helper("m.same", &[pair], &number(0))];
        let handed = tuple(&[number(1), text("a")], &[INT, STRING]);
        let refused = specialized(
            &holding(&helpers, &call("m.same", &[handed], INT)),
            |_, _| (),
        )
        .expect_err("one variable handed two types");
        assert!(
            refused.downcast_ref::<crate::NotLowered>().is_none(),
            "{refused}"
        );
        assert!(
            refused.to_string().contains("a call of m.same"),
            "{refused}"
        );
    }

    #[test]
    fn a_variable_binds_what_stands_where_it_stands() {
        let mut bound = Substitution::default();
        assert!(bound.binds(&list(var(1)), &list(int())));
        assert_eq!(bound.applied(&var(1)), Some(int()));
        assert_eq!(bound.applied(&var(0)), None);
    }

    #[test]
    fn a_variable_is_held_to_the_first_type_it_binds() {
        let mut bound = Substitution::default();
        let text = Ty::Prim { prim: Prim::String };
        assert!(bound.binds(&var(0), &int()));
        assert!(!bound.binds(&list(var(0)), &list(text)));
    }

    #[test]
    fn a_type_that_is_not_a_variable_is_the_same_type_made_the_same_way() {
        let mut bound = Substitution::default();
        assert!(!bound.binds(&list(var(0)), &int()));
        assert!(!bound.binds(&int(), &Ty::Prim { prim: Prim::Bool }));
    }

    #[test]
    fn a_function_type_binds_what_it_takes_and_what_it_answers() {
        let template = Ty::Fn {
            fn_: FnSignature {
                takes: vec![var(0), var(1)],
                answers: Box::new(var(0)),
            },
        };
        let text = Ty::Prim { prim: Prim::String };
        let settled = Ty::Fn {
            fn_: FnSignature {
                takes: vec![int(), text.clone()],
                answers: Box::new(int()),
            },
        };
        let mut bound = Substitution::default();
        assert!(bound.binds(&template, &settled));
        assert_eq!(bound.by_number(2), vec![Some(int()), Some(text)]);
        let disagreeing = Ty::Fn {
            fn_: FnSignature {
                takes: vec![int(), int()],
                answers: Box::new(Ty::Prim { prim: Prim::Bool }),
            },
        };
        assert!(!Substitution::default().binds(&template, &disagreeing));
    }
}
