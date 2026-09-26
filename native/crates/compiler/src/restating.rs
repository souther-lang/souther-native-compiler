//! A value of one type held as a value of another, which is what every `Widen`, every arm's read
//! and every value a composition hands a stage asks for.
//!
//! What has to happen is decided once, by [`restatement`], over the two types together, and what
//! it decides is carried out by [`restate`] and nothing else. The checker lets a value stand as a
//! wider type through what it is made of: an optional, a list and a tuple stand as one of what
//! they hold standing wider, and a function as one taking at least what the other takes and
//! answering no more. So the plan is made the same way, position by position, and a value is held
//! the same way where every position is.
//!
//! Where one is not, the value is rebuilt. An optional, a list and a tuple are rebuilt around what
//! they hold restated, and a function is wrapped: what calls it through the wider type hands the
//! wrapper what that type takes, and the wrapper restates it, calls the function it holds, and
//! restates what that answers. Each is a function of this object's own, one per pair of types, and
//! written once every body is, as a comparator is (`equality`): a restatement reaches the types a
//! value is made of only as its function is written.
//!
//! What restating can never do is fail. A value restated is one the checker settled stands as the
//! wider type, so nothing about it is asked again; what the wrapper of a function answers other
//! than an answer is what the function it holds answered, handed on as it came.

use std::cell::RefCell;
use std::collections::HashMap;

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, AbiParam, Function, InstBuilder, types};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    ANSWERED, CARRIED, HELD, LIST_ELEMENTS, LIST_LENGTH, NOTHING, SLOT, member_at, room_for_held,
    room_for_list, room_for_members,
};

use crate::transport::{Case, FnSignature, Prim, Ty};
use crate::{
    CLOSURE_CODE, Lowered, Lowerings, POINTER, TRUSTED, accepted, built_in_case, capture_at, carry,
    into_slot, laid_out_nowhere, lifted_signature, machine_type, not_lowered, out_of_slot,
    out_slot, room_for_closure, says_its_case,
};

/// What holding a value of one type as a value of another takes, position by position.
///
/// [`Restatement::Same`] wherever the two are held alike, so a plan that is `Same` at the top is
/// one that does nothing, and one that is anything else rebuilds only down to where it has to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Restatement {
    /// Held alike: the value stands where it is.
    Same,
    /// A primitive carried with its token as a value of a type that says its case.
    Carry(Prim),
    /// A primitive read back out of what carries it, once a test has said that is its case.
    Uncarry(Prim),
    /// An optional rebuilt around what it holds, restated.
    Option(Box<Restatement>),
    /// A list rebuilt with every element restated.
    List(Box<Restatement>),
    /// A tuple rebuilt with every member restated.
    Tuple(Vec<Restatement>),
    /// A function wrapped: each of what the wider type takes restated to what the function takes,
    /// and what it answers restated to what the wider type answers.
    Function {
        takes: Vec<Restatement>,
        answers: Box<Restatement>,
    },
}

impl Restatement {
    /// A plan over what a value is made of, which does nothing where none of its parts does.
    fn over(parts: Vec<Restatement>, made: impl FnOnce(Vec<Restatement>) -> Self) -> Self {
        if parts.iter().all(|it| *it == Restatement::Same) {
            Restatement::Same
        } else {
            made(parts)
        }
    }
}

/// What holding a value of `from` as a value of `to` takes. The one place that is decided.
///
/// A direction and not a likeness: which way a value goes decides what has to be true of it. Two
/// types that say their case are held alike, whichever cases they have, since every value of
/// either is the address of something with its token at the front. A primitive and a type that
/// says its case are not: the primitive is carried one way and read back out the other. What is
/// made of other types is restated where what it is made of is, and a function goes the other way
/// in what it takes, since what the position hands it is a value of what the position takes.
///
/// The type of what has no value is held as anything: no value of it is ever made, so there is
/// none to change. That is what lets the `[]` a walk is seeded with, a `List<Nothing>`, stand as
/// the list the walk grows without being rebuilt.
///
/// Every type is named on the left, with no arm standing for the rest, so a type laid out later
/// has to say here how its values are held before one stands as another. A pair the checker never
/// lets stand one as the other has no plan, and is refused as not lowered, since what `Coherent`
/// held of the pair is that it fits and not that it can be laid out.
pub(crate) fn restatement(from: &Ty, to: &Ty) -> Lowered<Restatement> {
    if from == to || (says_its_case(from) && says_its_case(to)) {
        return Ok(Restatement::Same);
    }
    let another_way = || {
        not_lowered(format!(
            "a value of {} standing as {}, which holds what it is made of another way",
            from.spelt(),
            to.spelt()
        ))
    };
    Ok(match (from, to) {
        (Ty::Nothing { .. }, _) => Restatement::Same,
        (Ty::Prim { prim }, _) if says_its_case(to) => {
            built_in_case(&Case::Primitive { prim: *prim })?;
            Restatement::Carry(*prim)
        }
        (_, Ty::Prim { prim }) if says_its_case(from) => {
            machine_type(to)?;
            Restatement::Uncarry(*prim)
        }
        (Ty::Option { option: from }, Ty::Option { option: to }) => {
            Restatement::over(vec![restatement(from, to)?], |mut it| {
                Restatement::Option(Box::new(it.remove(0)))
            })
        }
        (Ty::List { list: from }, Ty::List { list: to }) => {
            Restatement::over(vec![restatement(from, to)?], |mut it| {
                Restatement::List(Box::new(it.remove(0)))
            })
        }
        (Ty::Tuple { tuple: from }, Ty::Tuple { tuple: to }) => {
            if from.len() != to.len() {
                return Err(another_way());
            }
            let members = from
                .iter()
                .zip(to)
                .map(|(from, to)| restatement(from, to))
                .collect::<Lowered<Vec<_>>>()?;
            Restatement::over(members, Restatement::Tuple)
        }
        (Ty::Fn { fn_: from }, Ty::Fn { fn_: to }) => {
            if from.takes.len() != to.takes.len() {
                return Err(another_way());
            }
            let mut parts = to
                .takes
                .iter()
                .zip(&from.takes)
                .map(|(handed, taken)| restatement(handed, taken))
                .collect::<Lowered<Vec<_>>>()?;
            parts.push(restatement(&from.answers, &to.answers)?);
            Restatement::over(parts, |mut parts| {
                let answers = Box::new(parts.pop().expect("what the function answers is planned"));
                Restatement::Function {
                    takes: parts,
                    answers,
                }
            })
        }
        (Ty::Var { var }, _) | (_, Ty::Var { var }) => laid_out_nowhere(*var),
        // Equal types were answered above, and so were two that say their case; what is left of
        // these is a primitive beside something else, or one kind beside another. A set and a map
        // have no layout yet, and are asked of nothing until they do.
        (
            Ty::Prim { .. }
            | Ty::Declared { .. }
            | Ty::Union { .. }
            | Ty::Option { .. }
            | Ty::List { .. }
            | Ty::Tuple { .. }
            | Ty::Fn { .. }
            | Ty::Set { .. }
            | Ty::Map { .. },
            _,
        ) => return Err(another_way()),
    })
}

/// `value`, of type `from`, held as a value of `to`, as [`restatement`] plans it.
///
/// Where the plan is to leave it, this is no operation. A primitive is carried or read out where it
/// stands. Anything made of other types that has to be rebuilt is rebuilt by the function this
/// object has for the pair, so a site asking for one is a call and nothing more.
///
/// A primitive is read back out only once a test has said the value is that primitive's case: the
/// value is not asked again here.
pub(crate) fn restate(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    value: ir::Value,
    from: &Ty,
    to: &Ty,
) -> Lowered<ir::Value> {
    let plan = restatement(from, to)?;
    carried_out(builder, lowering, module, value, from, to, plan)
}

/// `value` restated by `plan`, which is what [`restatement`] answered for `from` and `to`.
fn carried_out(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    value: ir::Value,
    from: &Ty,
    to: &Ty,
    plan: Restatement,
) -> Lowered<ir::Value> {
    match plan {
        Restatement::Same => Ok(value),
        Restatement::Carry(prim) => carry(
            builder,
            lowering,
            module,
            &Case::Primitive { prim },
            Some(value),
        ),
        Restatement::Uncarry(prim) => {
            let held = builder
                .ins()
                .load(types::I64, TRUSTED, value, CARRIED as i32);
            Ok(out_of_slot(
                builder,
                held,
                machine_type(&Ty::Prim { prim })?,
            ))
        }
        Restatement::Option(_)
        | Restatement::List(_)
        | Restatement::Tuple(_)
        | Restatement::Function { .. } => {
            let call_conv = builder.func.signature.call_conv;
            let id = lowering
                .restaters
                .of(module, Kind::Rebuild, from, to, plan, call_conv)?;
            let restating = module.declare_func_in_func(id, builder.func);
            let called = builder.ins().call(restating, &[value]);
            Ok(builder.inst_results(called)[0])
        }
    }
}

/// Which of the two functions a pair of types can have this is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    /// Takes a value of the narrower type and answers it rebuilt as the wider.
    Rebuild,
    /// The code of a function value wrapping one of the narrower function type so that it can be
    /// called as the wider: a lifted function's signature over what the wider type takes.
    Wrap,
}

/// The function restating each pair of types some site in this object asked for, and the ones
/// whose body is still to be written.
#[derive(Default)]
pub(crate) struct Restaters {
    by_pair: RefCell<HashMap<(Kind, Ty, Ty), FuncId>>,
    owed: RefCell<Vec<Owed>>,
}

impl Restaters {
    /// The function of `kind` for `from` and `to`, given an id now where none was yet and owed a
    /// body until [`Restaters::owed`] hands it out.
    fn of(
        &self,
        module: &mut ObjectModule,
        kind: Kind,
        from: &Ty,
        to: &Ty,
        plan: Restatement,
        call_conv: CallConv,
    ) -> Lowered<FuncId> {
        let key = (kind, from.clone(), to.clone());
        if let Some(id) = self.by_pair.borrow().get(&key) {
            return Ok(*id);
        }
        let signature = match kind {
            Kind::Rebuild => {
                let mut signature = ir::Signature::new(call_conv);
                signature.params.push(AbiParam::new(machine_type(from)?));
                signature.returns.push(AbiParam::new(machine_type(to)?));
                signature
            }
            Kind::Wrap => {
                let Ty::Fn { fn_: to } = to else {
                    unreachable!("only a function is wrapped, and {} is not one", to.spelt())
                };
                lifted_signature(&to.takes, &to.answers, call_conv)?
            }
        };
        let id = accepted(module.declare_anonymous_function(&signature));
        crate::index::unique(&mut *self.by_pair.borrow_mut(), key, id);
        self.owed.borrow_mut().push(Owed {
            id,
            kind,
            from: from.clone(),
            to: to.clone(),
            plan,
            signature,
        });
        Ok(id)
    }

    /// A function whose body is still to be written. Writing one may owe more, so this is asked
    /// until it answers nothing.
    pub(crate) fn owed(&self) -> Option<Owed> {
        self.owed.borrow_mut().pop()
    }
}

/// A restating function declared and not yet written.
pub(crate) struct Owed {
    pub(crate) id: FuncId,
    kind: Kind,
    from: Ty,
    to: Ty,
    plan: Restatement,
    pub(crate) signature: ir::Signature,
}

/// The body of a function [`Restaters::of`] declared.
pub(crate) fn define_restater(
    func: &mut Function,
    shapes: &mut FunctionBuilderContext,
    owed: &Owed,
    frontend: TargetFrontendConfig,
    lowering: &Lowerings,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(func, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    let given = builder.block_params(entry).to_vec();
    let mut restating = Restating {
        builder: &mut builder,
        lowering,
        module,
    };
    match owed.kind {
        Kind::Rebuild => {
            let rebuilt = restating.rebuilt(&owed.from, &owed.to, &owed.plan, given[0])?;
            restating.builder.ins().return_(&[rebuilt]);
        }
        Kind::Wrap => {
            let (Ty::Fn { fn_: from }, Ty::Fn { fn_: to }) = (&owed.from, &owed.to) else {
                unreachable!("only a function is wrapped")
            };
            let Restatement::Function { takes, answers } = &owed.plan else {
                unreachable!("a function is wrapped by the plan made for it")
            };
            restating.wrapping(from, to, takes, answers, &given)?;
        }
    }
    builder.seal_all_blocks();
    builder.finalize(frontend);
    Ok(())
}

/// One restating function's body being written.
struct Restating<'b, 'f, 'l, 'm> {
    builder: &'b mut FunctionBuilder<'f>,
    lowering: &'l Lowerings<'l>,
    module: &'m mut ObjectModule,
}

impl Restating<'_, '_, '_, '_> {
    /// `value` restated by `plan` where it stands inside what is being rebuilt.
    fn part(
        &mut self,
        from: &Ty,
        to: &Ty,
        plan: &Restatement,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        carried_out(
            self.builder,
            self.lowering,
            self.module,
            value,
            from,
            to,
            plan.clone(),
        )
    }

    /// What a slot of `value` at `offset` holds, as a value of `ty`.
    fn read(&mut self, value: ir::Value, offset: i64, ty: &Ty) -> Lowered<ir::Value> {
        let held = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, value, offset as i32);
        Ok(out_of_slot(self.builder, held, machine_type(ty)?))
    }

    fn write(&mut self, into: ir::Value, offset: i64, value: ir::Value) {
        let held = into_slot(self.builder, value);
        self.builder.ins().store(TRUSTED, held, into, offset as i32);
    }

    /// `value`, of `from`, made again as a value of `to`.
    fn rebuilt(
        &mut self,
        from: &Ty,
        to: &Ty,
        plan: &Restatement,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        match (plan, from, to) {
            (Restatement::Option(held), Ty::Option { option: from }, Ty::Option { option: to }) => {
                self.optional(from, to, held, value)
            }
            (Restatement::List(element), Ty::List { list: from }, Ty::List { list: to }) => {
                self.list(from, to, element, value)
            }
            (Restatement::Tuple(members), Ty::Tuple { tuple: from }, Ty::Tuple { tuple: to }) => {
                let rebuilt =
                    self.lowering
                        .room(self.builder, self.module, room_for_members(members.len()));
                for (at, plan) in members.iter().enumerate() {
                    let member = self.read(value, member_at(at), &from[at])?;
                    let member = self.part(&from[at], &to[at], plan, member)?;
                    self.write(rebuilt, member_at(at), member);
                }
                Ok(rebuilt)
            }
            (Restatement::Function { .. }, Ty::Fn { .. }, Ty::Fn { .. }) => {
                let call_conv = self.builder.func.signature.call_conv;
                let code = self.lowering.restaters.of(
                    self.module,
                    Kind::Wrap,
                    from,
                    to,
                    plan.clone(),
                    call_conv,
                )?;
                let wrapper = self
                    .lowering
                    .room(self.builder, self.module, room_for_closure(1));
                let code = self.module.declare_func_in_func(code, self.builder.func);
                let code = self.builder.ins().func_addr(POINTER, code);
                self.builder
                    .ins()
                    .store(TRUSTED, code, wrapper, CLOSURE_CODE as i32);
                self.write(wrapper, capture_at(0), value);
                Ok(wrapper)
            }
            _ => unreachable!(
                "{} standing as {} is rebuilt only by the plan made for the two",
                from.spelt(),
                to.spelt()
            ),
        }
    }

    /// Nothing where it holds nothing, and otherwise what it holds restated, held anew.
    fn optional(
        &mut self,
        from: &Ty,
        to: &Ty,
        plan: &Restatement,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        let absent = self.builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING);
        let present = self.builder.create_block();
        let done = self.builder.create_block();
        self.builder.append_block_param(done, POINTER);
        self.builder
            .ins()
            .brif(absent, done, &[value.into()], present, &[]);

        self.builder.switch_to_block(present);
        let held = self.read(value, HELD, from)?;
        let held = self.part(from, to, plan, held)?;
        let holding = self
            .lowering
            .room(self.builder, self.module, room_for_held());
        self.write(holding, HELD, held);
        self.builder.ins().jump(done, &[holding.into()]);

        self.builder.switch_to_block(done);
        Ok(self.builder.block_params(done)[0])
    }

    /// A list of as many elements, each the element at its index restated.
    fn list(
        &mut self,
        from: &Ty,
        to: &Ty,
        plan: &Restatement,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        let length = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, value, LIST_LENGTH as i32);
        let along = self.builder.ins().imul_imm_s(length, SLOT);
        let bytes = self.builder.ins().iadd_imm_s(along, room_for_list(0));
        let rebuilt = self.lowering.room_of(self.builder, self.module, bytes);
        self.builder
            .ins()
            .store(TRUSTED, length, rebuilt, LIST_LENGTH as i32);

        let head = self.builder.create_block();
        self.builder.append_block_param(head, types::I64);
        let step = self.builder.create_block();
        let walked = self.builder.create_block();
        let start = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().jump(head, &[start.into()]);

        self.builder.switch_to_block(head);
        let index = self.builder.block_params(head)[0];
        let inside = self
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, index, length);
        self.builder.ins().brif(inside, step, &[], walked, &[]);

        self.builder.switch_to_block(step);
        let along = self.builder.ins().imul_imm_s(index, SLOT);
        let one_at = self.builder.ins().iadd(value, along);
        let other_at = self.builder.ins().iadd(rebuilt, along);
        let element = self.read(one_at, LIST_ELEMENTS, from)?;
        let element = self.part(from, to, plan, element)?;
        self.write(other_at, LIST_ELEMENTS, element);
        let next = self.builder.ins().iadd_imm_s(index, 1);
        self.builder.ins().jump(head, &[next.into()]);

        self.builder.switch_to_block(walked);
        Ok(rebuilt)
    }

    /// The code of a function value that holds one of `from` and is called as one of `to`: what it
    /// is handed restated to what the function it holds takes, that function called, and what it
    /// answers restated to what `to` answers. A status other than an answer is handed on as it
    /// came, and nothing is written through the room for the answer.
    fn wrapping(
        &mut self,
        from: &FnSignature,
        to: &FnSignature,
        takes: &[Restatement],
        answers: &Restatement,
        given: &[ir::Value],
    ) -> Lowered<()> {
        let wrapper = given[0];
        let out = given[given.len() - 1];
        let handed = &given[1..given.len() - 1];

        let held = self
            .builder
            .ins()
            .load(POINTER, TRUSTED, wrapper, capture_at(0) as i32);
        let mut arguments = Vec::with_capacity(handed.len() + 2);
        arguments.push(held);
        for (at, plan) in takes.iter().enumerate() {
            arguments.push(self.part(&to.takes[at], &from.takes[at], plan, handed[at])?);
        }
        let answered_into = out_slot(self.builder);
        arguments.push(answered_into);

        let call_conv = self.builder.func.signature.call_conv;
        let signature = lifted_signature(&from.takes, &from.answers, call_conv)?;
        let signature = self.builder.import_signature(signature);
        let code = self
            .builder
            .ins()
            .load(POINTER, TRUSTED, held, CLOSURE_CODE as i32);
        let called = self
            .builder
            .ins()
            .call_indirect(signature, code, &arguments);
        let status = self.builder.inst_results(called)[0];

        let answered = self.builder.create_block();
        let not = self.builder.create_block();
        let is_answered = self
            .builder
            .ins()
            .icmp_imm_s(IntCC::Equal, status, i64::from(ANSWERED));
        self.builder
            .ins()
            .brif(is_answered, answered, &[], not, &[]);

        self.builder.switch_to_block(not);
        self.builder.ins().return_(&[status]);

        self.builder.switch_to_block(answered);
        let answer =
            self.builder
                .ins()
                .load(machine_type(&from.answers)?, TRUSTED, answered_into, 0);
        let answer = self.part(&from.answers, &to.answers, answers, answer)?;
        self.builder.ins().store(TRUSTED, answer, out, 0);
        let ok = self.builder.ins().iconst(types::I32, i64::from(ANSWERED));
        self.builder.ins().return_(&[ok]);
        Ok(())
    }
}
