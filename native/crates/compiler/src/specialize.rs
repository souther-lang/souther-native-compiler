//! A helper whose body leaves type variables open, made into one function for each set of types a
//! call needs.
//!
//! What the language leaves to a backend (ADR-0092): a recursive helper is not expanded at its call
//! sites and stays one definition over `'a`, and the instantiation of a variable belongs to the
//! call. On the JVM that is absorbed by the representation a value runs in. Here every layout, every
//! comparison and every codec is chosen by type, so a body over `'a` has no lowering until `'a` is a
//! type, and what is lowered is a copy of it for each set of types some call hands it.
//!
//! Nothing here works a type out. A call already carries what its arguments and its answer were
//! settled at, and the helper's parameters and answer say where each variable stands in those, so
//! what a variable comes to is read off the one against the other ([`Substitution::binds`]). A
//! variable no parameter and no answer reaches is one no call settles, and the helper is not lowered.
//!
//! A copy is keyed by the helper and what its variables come to, so every call needing the same
//! types reaches one function. A copy's body is the helper's with its variables replaced, and a call
//! inside it is resolved the same way, so the self-call of `foldFrom<Int, Int>` reaches
//! `foldFrom<Int, Int>` again. Which copy a call reaches is answered here, once, by the call it is;
//! the lowering reads that answer and does not work it out a second time from a name and some types.
//!
//! A helper that calls itself at other types than it was called at is refused. Each copy would need
//! another after it, and there is no number of them that is enough.

use crate::index;
use crate::transport::{Body, Carrier, Held, Node, Owner, Reaches, Ty};
use crate::{Lowered, Runs, not_lowered};
use std::collections::HashMap;

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
            (Ty::Prim { prim }, Ty::Prim { prim: also }) => prim == also,
            (Ty::Declared { declared }, Ty::Declared { declared: also }) => declared == also,
            (Ty::Union { union }, Ty::Union { union: also }) => union == also,
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
            (
                Ty::Prim { .. }
                | Ty::Declared { .. }
                | Ty::Union { .. }
                | Ty::Option { .. }
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
        Some(match ty {
            Ty::Var { var } => self.0.get(*var)?.clone()?,
            Ty::Prim { .. } | Ty::Declared { .. } | Ty::Union { .. } => ty.clone(),
            Ty::Option { option } => Ty::Option {
                option: Box::new(self.applied(option)?),
            },
            Ty::List { list } => Ty::List {
                list: Box::new(self.applied(list)?),
            },
            Ty::Set { set } => Ty::Set {
                set: Box::new(self.applied(set)?),
            },
            Ty::Tuple { tuple } => Ty::Tuple {
                tuple: tuple
                    .iter()
                    .map(|it| self.applied(it))
                    .collect::<Option<_>>()?,
            },
            Ty::Fn { fn_ } => Ty::Fn {
                fn_: crate::transport::FnSignature {
                    takes: fn_
                        .takes
                        .iter()
                        .map(|it| self.applied(it))
                        .collect::<Option<_>>()?,
                    answers: Box::new(self.applied(&fn_.answers)?),
                },
            },
            Ty::Map { map } => Ty::Map {
                map: crate::transport::MapTy {
                    key: Box::new(self.applied(&map.key)?),
                    value: Box::new(self.applied(&map.value)?),
                },
            },
        })
    }

    /// What each of the first `count` variables came to, where every one of them is bound.
    fn settled(&self, count: usize) -> Option<Vec<Ty>> {
        (0..count).map(|at| self.0.get(at)?.clone()).collect()
    }

    fn of(types: &[Ty]) -> Self {
        Substitution(types.iter().cloned().map(Some).collect())
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
    /// What each variable came to, by its number. Empty for a helper that leaves none open.
    pub types: Vec<Ty>,
    /// Which copy of the helper this is, counted from nought among the copies of the one helper,
    /// which is what tells the functions of one helper apart.
    pub ordinal: usize,
    body: Settled<'p>,
    /// The copy whose body called for this one, where one did.
    called_from: Option<InstanceId>,
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
            .map(|parameter| {
                settled
                    .applied(&parameter.ty)
                    .expect("a copy settles every variable its helper leaves open")
            })
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
    by_key: HashMap<(&'p str, &'p str, Vec<Ty>), InstanceId>,
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
            reached: HashMap::new(),
        };
        let mut helpers: HashMap<(&'p str, &'p str), Body<'p>> = HashMap::new();
        for body in runs.bodies() {
            if let Owner::Helper(held) = body.owner {
                index::unique(
                    &mut helpers,
                    (body.carrier().module(), held.reached.as_str()),
                    body,
                );
            }
        }
        let mut roots = Vec::new();
        for body in runs.bodies() {
            match body.owner {
                Owner::Helper(held) if held.variables() == 0 => {
                    specializations.copy(body.carrier(), held, Vec::new(), None)?;
                }
                // A helper over variables is lowered as its copies and never as itself.
                Owner::Helper(_) => {}
                Owner::Value(_)
                | Owner::Entry(_)
                | Owner::Definition(_)
                | Owner::Example(_)
                | Owner::Invariant { .. }
                | Owner::Ensures { .. } => roots.push(body),
            }
        }
        for body in roots {
            specializations.resolve(&helpers, body.carrier(), calls_in(body.node), None)?;
        }
        // Each copy's body, once, in the order the copies were made: a copy made while one is
        // read is read after it.
        let mut at = 0;
        while at < specializations.instances.len() {
            let instance = &specializations.instances[at];
            let carrier = instance.carrier;
            let calls = calls_in(instance.body());
            specializations.resolve(&helpers, carrier, calls, Some(InstanceId(at)))?;
            at += 1;
        }
        Ok(specializations)
    }

    /// Every call reaching a helper in `node`, each to the copy it needs.
    fn resolve(
        &mut self,
        helpers: &HashMap<(&'p str, &'p str), Body<'p>>,
        carrier: Carrier<'p>,
        calls: Vec<Called>,
        within: Option<InstanceId>,
    ) -> Lowered<()> {
        for call in calls {
            let helper = helpers
                .get(&(carrier.module(), call.reached.as_str()))
                .expect("`Coherent` held every helper a call reaches to be one its module holds");
            let Owner::Helper(held) = helper.owner else {
                unreachable!("gathered from the helpers' bodies alone");
            };
            let reached = &held.reached;
            let handed: Vec<&Ty> = call.handed.iter().collect();
            let bound = called(held, &handed, &call.answers)
                .expect("`Coherent` held every call of a helper to fit what the helper takes");
            let types = bound.settled(held.variables()).ok_or_else(|| {
                not_lowered(format!(
                    "{reached}, whose body leaves open a type no call of it settles"
                ))
            })?;
            let id = self.copy(helper.carrier(), held, types, within)?;
            index::unique(&mut self.reached, call.at, id);
        }
        Ok(())
    }

    /// The copy of `held` whose variables come to `types`, made where there is none yet.
    fn copy(
        &mut self,
        carrier: Carrier<'p>,
        held: &'p Held,
        types: Vec<Ty>,
        called_from: Option<InstanceId>,
    ) -> Lowered<InstanceId> {
        let key = (carrier.module(), held.reached.as_str(), types);
        if let Some(id) = self.by_key.get(&key) {
            return Ok(*id);
        }
        let (_, _, types) = key;
        // A helper met again on the way to this copy, at other types: each copy would call for
        // another.
        let mut from = called_from;
        while let Some(InstanceId(at)) = from {
            let before = &self.instances[at];
            if std::ptr::eq(before.held, held) {
                return Err(not_lowered(format!(
                    "{}, which calls itself at other types than it was called at",
                    held.reached
                )));
            }
            from = before.called_from;
        }
        let body = if types.is_empty() {
            Settled::AsHeld(&held.body)
        } else {
            let mut written = false;
            held.body.each(&mut |node| {
                written |= matches!(node, Node::Block { .. });
            });
            if written {
                return Err(not_lowered(format!(
                    "a function value written inside {}, which leaves type variables open",
                    held.reached
                )));
            }
            let mut body = held.body.clone();
            settle(&mut body, &Substitution::of(&types));
            Settled::Rewritten(Box::new(body))
        };
        let ordinal = self
            .instances
            .iter()
            .filter(|it| it.carrier == carrier && std::ptr::eq(it.held, held))
            .count();
        let id = InstanceId(self.instances.len());
        self.instances.push(Instance {
            carrier,
            held,
            types: types.clone(),
            ordinal,
            body,
            called_from,
        });
        index::unique(
            &mut self.by_key,
            (carrier.module(), held.reached.as_str(), types),
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
    reached: String,
    handed: Vec<Ty>,
    answers: Ty,
}

/// Every call reaching a helper in `node`, a function value's body included.
fn calls_in(node: &Node) -> Vec<Called> {
    let mut calls = Vec::new();
    node.each(&mut |node| {
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

/// Every type `node` writes, and every node under it writes, with its variables replaced.
fn settle(node: &mut Node, settled: &Substitution) {
    for ty in node.types_mut() {
        *ty = settled
            .applied(ty)
            .expect("a copy settles every variable its helper leaves open");
    }
    for child in node.children_mut() {
        settle(child, settled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{FnSignature, Prim};

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
        node.each(&mut |node| {
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
            r#"{{"transport":18,"declarations":[],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{}],"values":[{{"module":"m","name":"v","handovers":[],"body":{body}}}],"entries":[],"definitions":[],"examples":[]}}]}}"#,
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
            r#"{{"reached":"{reached}","parameters":[{}],"body":{body}}}"#,
            parameters.join(",")
        )
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
                r#""reaches":{{"is":"helper","reached":"{reached}"}},"arguments":[{}]"#,
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
            let types: Vec<&[Ty]> = copies.iter().map(|(_, it)| it.types.as_slice()).collect();
            assert_eq!(
                types,
                [&[int.clone(), int.clone()][..], &[text, int][..]],
                "one copy for each set of types"
            );
            for (id, copy) in &copies {
                assert_eq!(copy.held.reached, "List.foldFrom");
                let own = calls(copy.body());
                assert_eq!(own.len(), 1, "foldFrom calls itself once");
                assert_eq!(
                    specializations.callee(own[0]),
                    *id,
                    "and reaches its own copy"
                );
                let mut open = 0;
                copy.body().each(&mut |node| {
                    open += node.types().iter().filter(|ty| ty.variables() > 0).count();
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
            let copies: Vec<(String, Vec<Ty>)> = specializations
                .iter()
                .map(|(_, it)| (it.held.reached.clone(), it.types.clone()))
                .collect();
            assert_eq!(
                copies,
                [
                    ("m.first".to_string(), vec![Ty::Prim { prim: Prim::Int }]),
                    (
                        "m.second".to_string(),
                        vec![Ty::Prim { prim: Prim::String }]
                    ),
                ]
            );
        })
        .expect("a document every relation of which holds");
    }

    /// A helper handing itself a list of what it was handed would need a copy for every depth.
    #[test]
    fn a_helper_calling_itself_at_other_types_is_not_lowered() {
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
        assert!(refused.to_string().contains("other types"), "{refused}");
    }

    /// What a function value written in a helper over variables carries is laid out for no copy.
    #[test]
    fn a_function_value_inside_a_helper_over_variables_is_not_lowered() {
        let made = r#"{"fn":{"takes":[],"answers":{"var":0}}}"#;
        let block = node(
            "block",
            &format!(r#""site":0,"parameters":[],"body":{}"#, read(0, VAR)),
            made,
        );
        let helpers = [helper("m.k", &[VAR], &block)];
        let settled = r#"{"fn":{"takes":[],"answers":{"prim":"INT"}}}"#;
        let refused = specialized(
            &holding(&helpers, &call("m.k", &[number(1)], settled)),
            |_, _| (),
        )
        .expect_err("a closure over a variable");
        assert!(
            refused.downcast_ref::<crate::NotLowered>().is_some(),
            "{refused}"
        );
        assert!(refused.to_string().contains("function value"), "{refused}");
    }

    /// A variable nothing a call hands over or answers reaches is one no call settles.
    #[test]
    fn a_variable_no_call_settles_is_not_lowered() {
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
        let refused = specialized(
            &holding(&helpers, &call("m.h", &[number(1)], INT)),
            |_, _| (),
        )
        .expect_err("nothing settles the variable");
        assert!(
            refused.downcast_ref::<crate::NotLowered>().is_some(),
            "{refused}"
        );
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
        assert!(refused.to_string().contains("m.same"), "{refused}");
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
        assert_eq!(bound.settled(2), Some(vec![int(), text]));
        let disagreeing = Ty::Fn {
            fn_: FnSignature {
                takes: vec![int(), int()],
                answers: Box::new(Ty::Prim { prim: Prim::Bool }),
            },
        };
        assert!(!Substitution::default().binds(&template, &disagreeing));
    }
}
