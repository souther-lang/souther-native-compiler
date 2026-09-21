//! Lowers what crossed to Cranelift IR, and lets Cranelift write the object.
//!
//! The object is for the machine this runs on. Choosing a target for another machine is a question
//! about linkers and a runtime built for it, and answering that before the code generation works
//! would be answering the easier question first.

pub mod transport;

use anyhow::{Result, anyhow, bail};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{
    AbiParam, Function, InstBuilder, MemFlagsData, TrapCode, UserFuncName, types,
};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::codegen::settings::{self, Configurable};
use cranelift::codegen::{Context, ir};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift::module::{Linkage, Module, default_libcall_names};
use cranelift::object::{ObjectBuilder, ObjectModule};
use souther_native_abi::{
    ALLOCATE, HELD, NOTHING, SLOT, WHICH, behavior_symbol, field_at, member_at,
};
use std::collections::HashMap;
use std::fmt;
use transport::{Arm, Behavior, Declaration, Node, Op, Prim, Program, Selects, TRANSPORT_VERSION, Ty};

/// An `Int` overflowing is an abort and not an answer, so the arithmetic traps rather than
/// wrapping. Which abort it was is not said here: nothing yet carries a reason out of a native run,
/// and a number invented at this end would be a second answer to a question Souther has not
/// answered once.
const OVERFLOWED: u8 = 1;

/// A fork that ran out of arms, which is this compiler having emitted the wrong test rather than
/// anything a program can be written as: the checker settles that a fork always answers.
const NO_ARM: u8 = 2;

/// Something the language admits and this driver does not lower yet.
///
/// Its own type because the half that started this has to tell it from a document it could not
/// read: one says the program is ahead of this backend, the other that the two halves disagree
/// about what they are saying to each other, and a caller that heard one word for both would
/// report a compiler's own fault as the author's.
#[derive(Debug)]
pub struct NotLowered(pub String);

impl fmt::Display for NotLowered {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

impl std::error::Error for NotLowered {}

/// How the driver answers, for the half that started it.
///
/// A process says one number and the reader of it is in another language, so this is a contract
/// with a party the compiler cannot check. What holds the two together is a test that runs the
/// whole way through rather than each side's reading of this comment.
pub mod ended {
    /// The object is on stdout.
    pub const WITH_AN_OBJECT: u8 = 0;

    /// Something went wrong here: a document this driver could not read, or a machine it could not
    /// write for. Not the program's author's to fix.
    pub const BADLY: u8 = 1;

    /// The program is one the language admits and this backend does not write yet. What it was is
    /// on stderr.
    pub const NOT_LOWERED: u8 = 2;
}

fn not_lowered(what: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(NotLowered(what.into()))
}

/// The object holding every behavior the document carries.
pub fn object_for(document: &str) -> Result<Vec<u8>> {
    let program: Program = serde_json::from_str(document)?;
    if program.transport != TRANSPORT_VERSION {
        bail!(
            "this driver reads transport {TRANSPORT_VERSION} and was handed {}",
            program.transport
        );
    }

    let mut flags = settings::builder();
    // A call out of this object reaches its callee the way the platform's linker expects, which on
    // both of the hosts this runs on means position-independent. Said here rather than left to the
    // host, so that what is emitted is the same wherever it is built.
    flags.set("is_pic", "true")?;
    let isa = cranelift::native::builder()
        .map_err(|it| anyhow!("no code generator for this host: {it}"))?
        .finish(settings::Flags::new(flags))?;

    let builder = ObjectBuilder::new(isa, "souther", default_libcall_names())?;
    let mut module = ObjectModule::new(builder);
    let mut context = Context::new();
    let mut shapes = FunctionBuilderContext::new();
    let frontend = module.isa().frontend_config();
    let call_conv = module.isa().default_call_conv();

    let mut taking_room = ir::Signature::new(call_conv);
    taking_room.params.push(AbiParam::new(types::I64));
    taking_room.returns.push(AbiParam::new(POINTER));
    let allocate = module.declare_function(ALLOCATE, Linkage::Import, &taking_room)?;

    let declared = Declared::of(&program.declarations)?;

    for written in &program.modules {
        for behavior in &written.behaviors {
            let symbol = behavior_symbol(&written.name, &behavior.name);
            let signature = signature_of(behavior, call_conv)?;
            let id = module.declare_function(&symbol, Linkage::Export, &signature)?;

            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            let taking = module.declare_func_in_func(allocate, &mut context.func);
            let lowering = Lowering {
                declared: &declared,
                taking,
            };
            define(&mut context.func, &mut shapes, behavior, frontend, &lowering)?;
            module.define_function(id, &mut context)?;
        }
    }

    Ok(module.finish().emit()?)
}

/// What every declared type of the program is, and which number stands for it.
///
/// The number is this lowering's and nothing outside it means anything by one: it is an index into
/// the declarations the document carries, so two builds of one document agree and nothing else has
/// to. A value carries it so that a fork on what a value is can be a comparison rather than a
/// question about where the value came from.
struct Declared<'a> {
    which: HashMap<&'a str, i64>,
    shapes: HashMap<&'a str, &'a Declaration>,
}

impl<'a> Declared<'a> {
    fn of(declarations: &'a [Declaration]) -> Result<Self> {
        let mut which = HashMap::new();
        let mut shapes = HashMap::new();
        for (at, declaration) in declarations.iter().enumerate() {
            let name = declaration.declared();
            if which.insert(name, at as i64).is_some() {
                bail!("two declarations are both written {name}");
            }
            shapes.insert(name, declaration);
        }
        Ok(Declared { which, shapes })
    }

    fn number(&self, declared: &str) -> Result<i64> {
        self.which
            .get(declared)
            .copied()
            .ok_or_else(|| anyhow!("a value of {declared}, which no declaration crossed for"))
    }

    fn shape(&self, declared: &str) -> Result<&'a Declaration> {
        self.shapes
            .get(declared)
            .copied()
            .ok_or_else(|| anyhow!("a value of {declared}, which no declaration crossed for"))
    }
}

/// What the lowering of one function needs besides the function itself.
struct Lowering<'a> {
    declared: &'a Declared<'a>,
    taking: ir::FuncRef,
}

impl Lowering<'_> {
    /// Room for `slots` slots, from the arena the caller brackets.
    fn room(&self, builder: &mut FunctionBuilder, slots: usize) -> ir::Value {
        let size = builder.ins().iconst(types::I64, SLOT * slots as i64);
        let taken = builder.ins().call(self.taking, &[size]);
        builder.inst_results(taken)[0]
    }
}

fn signature_of(behavior: &Behavior, call_conv: CallConv) -> Result<ir::Signature> {
    let mut signature = ir::Signature::new(call_conv);
    for taken in &behavior.takes {
        signature.params.push(AbiParam::new(machine_type(taken)?));
    }
    signature
        .returns
        .push(AbiParam::new(machine_type(&behavior.answers)?));
    Ok(signature)
}

/// What a value of this type is on the machine.
///
/// A number, a truth, or the address of what a value is made of. Every other primitive is a value
/// with a representation to design — how it is held, who owns it, what frees it — and none of that
/// is decided by giving it a width here.
fn machine_type(ty: &Ty) -> Result<types::Type> {
    match ty {
        Ty::Prim { prim: Prim::Int } => Ok(types::I64),
        Ty::Prim { prim: Prim::Bool } => Ok(types::I8),
        Ty::Declared { .. } | Ty::Union { .. } | Ty::Option { .. } | Ty::Tuple { .. } => Ok(POINTER),
        other => Err(not_lowered(format!("a value of type {}", other.spelt()))),
    }
}

/// What holds the address of a value made of fields.
///
/// One width, and the host's. Nothing here is written for a machine this is not running on, and a
/// pointer narrower or wider than a slot would make a field's offset a question about the host
/// rather than a multiplication.
const POINTER: types::Type = types::I64;

/// Everything a value is made of sits in a slot of one width, so what is put in one is widened to
/// it and what comes out is narrowed back.
///
/// A `Bool` is the only thing narrower today. Widening it here rather than laying it out where it
/// fits keeps a field's offset a fact about its position and not about the types before it.
/// How this reads and writes what it has just made room for.
///
/// Aligned and not trapping, which is what the arena answers: room is handed out a slot at a time
/// and a pointer from it is one nothing else is using. Written once here so that every access says
/// the same thing rather than each site deciding what it trusts.
const TRUSTED: MemFlagsData = MemFlagsData::trusted();

fn into_slot(builder: &mut FunctionBuilder, value: ir::Value) -> ir::Value {
    if builder.func.dfg.value_type(value) == types::I64 {
        value
    } else {
        builder.ins().uextend(types::I64, value)
    }
}

fn out_of_slot(builder: &mut FunctionBuilder, held: ir::Value, wanted: types::Type) -> ir::Value {
    if wanted == types::I64 {
        held
    } else {
        builder.ins().ireduce(wanted, held)
    }
}

fn define(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    behavior: &Behavior,
    frontend: TargetFrontendConfig,
    lowering: &Lowering,
) -> Result<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut bindings = Bindings::default();
    for (at, taken) in behavior.takes.iter().enumerate() {
        let variable = builder.declare_var(machine_type(taken)?);
        let given = builder.block_params(entry)[at];
        builder.def_var(variable, given);
        bindings.at(at, variable);
    }

    let answer = lower(&mut builder, lowering, &mut bindings, &behavior.body)?;
    builder.ins().return_(&[answer]);
    builder.finalize(frontend);
    Ok(())
}

/// What the document's numbers for a behavior's bindings stand for here.
///
/// Held by the number rather than pushed in the order they are met: the writer numbers a binder
/// where it writes it and this lowers a binding's value before the binder exists, so an order
/// either side happened to have would only agree until a binding's value held a binding of its own.
#[derive(Default)]
struct Bindings {
    held: Vec<Option<Variable>>,
}

impl Bindings {
    fn at(&mut self, number: usize, variable: Variable) {
        if self.held.len() <= number {
            self.held.resize(number + 1, None);
        }
        self.held[number] = Some(variable);
    }

    fn of(&self, number: usize) -> Result<Variable> {
        self.held
            .get(number)
            .copied()
            .flatten()
            .ok_or_else(|| anyhow!("a read of binding {number}, which nothing bound"))
    }
}

fn lower(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    bindings: &mut Bindings,
    node: &Node,
) -> Result<ir::Value> {
    Ok(match node {
        Node::Int { value, ty } => builder.ins().iconst(machine_type(ty)?, *value),
        Node::Read { binding, .. } => {
            let variable = bindings.of(*binding)?;
            builder.use_var(variable)
        }
        Node::Neg { operand, .. } => {
            let held = lower(builder, lowering, bindings, operand)?;
            let nought = builder.ins().iconst(machine_type(operand.ty())?, 0);
            difference(builder, nought, held)
        }
        Node::Let {
            binding,
            value,
            body,
            ..
        } => {
            let held = lower(builder, lowering, bindings, value)?;
            let variable = builder.declare_var(machine_type(value.ty())?);
            builder.def_var(variable, held);
            bindings.at(*binding, variable);
            lower(builder, lowering, bindings, body)?
        }
        Node::Bool { value, ty } => builder.ins().iconst(machine_type(ty)?, i64::from(*value)),
        Node::Binary {
            op, left, right, ..
        } => binary(builder, lowering, bindings, *op, left, right)?,
        Node::If {
            cond,
            then,
            els,
            ty,
        } => {
            let asked = lower(builder, lowering, bindings, cond)?;
            let answers = machine_type(ty)?;
            fork(builder, asked, answers, |builder, taken| {
                lower(builder, lowering, bindings, if taken { then } else { els })
            })?
        }
        Node::Unit { declared, .. } => {
            let flags = TRUSTED;
            let value = lowering.room(builder, 1);
            let which = builder
                .ins()
                .iconst(types::I64, lowering.declared.number(declared)?);
            builder.ins().store(flags, which, value, WHICH as i32);
            value
        }
        Node::Construct {
            declared, values, ..
        } => {
            let shape = lowering.declared.shape(declared)?;
            // A construction runs the type's clauses and stops at the first that does not hold,
            // which is an abort and not a value. Nothing here runs one, and building the value
            // without running them would make a type's invariant true of what this emits by
            // omission.
            if shape.invariants() > 0 {
                return Err(not_lowered(format!(
                    "a construction of {declared}, which states what every one of its values owes"
                )));
            }
            if shape.field_count() != values.len() {
                bail!(
                    "{declared} is declared with {} fields and is built here from {}",
                    shape.field_count(),
                    values.len()
                );
            }
            // The fields are worked out before any room is taken, because working one out can
            // take room of its own and what is half-written is not a value.
            let mut held = Vec::with_capacity(values.len());
            for value in values {
                let answered = lower(builder, lowering, bindings, value)?;
                held.push(into_slot(builder, answered));
            }
            let flags = TRUSTED;
            let value = lowering.room(builder, 1 + values.len());
            let which = builder
                .ins()
                .iconst(types::I64, lowering.declared.number(declared)?);
            builder.ins().store(flags, which, value, WHICH as i32);
            for (at, field) in held.into_iter().enumerate() {
                builder
                    .ins()
                    .store(flags, field, value, field_at(at) as i32);
            }
            value
        }
        Node::Field { target, field, ty } => {
            let of = target.ty();
            let Ty::Declared { declared } = of else {
                bail!("a field of {}, which holds no fields", of.spelt());
            };
            let shape = lowering.declared.shape(declared)?;
            let at = shape
                .position_of(field)
                .ok_or_else(|| anyhow!("{declared} declares no field {field}"))?;
            let value = lower(builder, lowering, bindings, target)?;
            let flags = TRUSTED;
            let held = builder
                .ins()
                .load(types::I64, flags, value, field_at(at) as i32);
            out_of_slot(builder, held, machine_type(ty)?)
        }
        Node::Match { subject, arms, ty } => {
            let value = lower(builder, lowering, bindings, subject)?;
            fork_on_what_it_is(builder, lowering, bindings, value, arms, machine_type(ty)?)?
        }
        Node::Some { value, .. } => {
            let held = lower(builder, lowering, bindings, value)?;
            let held = into_slot(builder, held);
            let flags = TRUSTED;
            let holding = lowering.room(builder, 1);
            builder.ins().store(flags, held, holding, HELD as i32);
            holding
        }
        Node::None { .. } => builder.ins().iconst(POINTER, NOTHING),
        Node::Tuple { members, .. } => {
            let mut held = Vec::with_capacity(members.len());
            for member in members {
                let answered = lower(builder, lowering, bindings, member)?;
                held.push(into_slot(builder, answered));
            }
            let flags = TRUSTED;
            let value = lowering.room(builder, members.len().max(1));
            for (at, member) in held.into_iter().enumerate() {
                builder.ins().store(flags, member, value, member_at(at) as i32);
            }
            value
        }
        Node::Member { tuple, at, ty } => {
            let value = lower(builder, lowering, bindings, tuple)?;
            let flags = TRUSTED;
            let held = builder
                .ins()
                .load(types::I64, flags, value, member_at(*at) as i32);
            out_of_slot(builder, held, machine_type(ty)?)
        }
    })
}

/// A fork on what a value is, arm by arm.
///
/// The arms are tried in the order they are written, because that is the order the language reads
/// them in. What an arm tests is what the checker resolved it to and not the name it was written
/// under, so a case that is itself a sum arrives here as the several types it stands for.
///
/// Running out of arms is this compiler having emitted the wrong test: the checker settles that a
/// fork always answers, so nothing a program can be written as reaches the end of this.
fn fork_on_what_it_is(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    bindings: &mut Bindings,
    value: ir::Value,
    arms: &[Arm],
    answers: types::Type,
) -> Result<ir::Value> {
    let after = builder.create_block();
    builder.append_block_param(after, answers);

    for arm in arms {
        let taken = builder.create_block();
        let next = builder.create_block();
        let asked = tests(builder, lowering, value, &arm.selects)?;
        builder.ins().brif(asked, taken, &[], next, &[]);
        builder.seal_block(taken);
        builder.seal_block(next);

        builder.switch_to_block(taken);
        if let Some(number) = arm.binding {
            let read_as = arm
                .binds
                .as_ref()
                .ok_or_else(|| anyhow!("an arm binds a value and does not say what it reads it as"))?;
            let held = binds(builder, value, &arm.selects, machine_type(read_as)?);
            let variable = builder.declare_var(machine_type(read_as)?);
            builder.def_var(variable, held);
            bindings.at(number, variable);
        }
        let answered = lower(builder, lowering, bindings, &arm.body)?;
        builder.ins().jump(after, &[answered.into()]);

        builder.switch_to_block(next);
    }

    builder
        .ins()
        .trap(TrapCode::user(NO_ARM).expect("a trap code of its own"));

    builder.seal_block(after);
    builder.switch_to_block(after);
    Ok(builder.block_params(after)[0])
}

/// Whether the value is one of the cases this arm answers for.
fn tests(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    value: ir::Value,
    selects: &[Selects],
) -> Result<ir::Value> {
    let mut asked: Option<ir::Value> = None;
    for one in selects {
        let this = match one {
            Selects::Which { atoms } => {
                let flags = TRUSTED;
                let which = builder.ins().load(types::I64, flags, value, WHICH as i32);
                let mut any: Option<ir::Value> = None;
                for atom in atoms {
                    let number = lowering.declared.number(atom)?;
                    let same = builder.ins().icmp_imm_s(IntCC::Equal, which, number);
                    any = Some(match any {
                        None => same,
                        Some(before) => builder.ins().bor(before, same),
                    });
                }
                any.ok_or_else(|| anyhow!("an arm testing what a value is names no case"))?
            }
            Selects::Held => builder.ins().icmp_imm_s(IntCC::NotEqual, value, NOTHING),
            Selects::Nothing => builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING),
        };
        asked = Some(match asked {
            None => this,
            Some(before) => builder.ins().bor(before, this),
        });
    }
    asked.ok_or_else(|| anyhow!("an arm answers for at least one case"))
}

/// What the arm reads the value as, once it is known to be one of its cases.
///
/// An arm over an optional's present carrier reads what it holds; every other arm reads the value
/// itself, which is already the case it selected. What comes out of the slot is narrowed to what
/// the arm says it reads the value as — which the arm carries, because the test it was selected by
/// does not say it.
fn binds(
    builder: &mut FunctionBuilder,
    value: ir::Value,
    selects: &[Selects],
    read_as: types::Type,
) -> ir::Value {
    if selects.iter().any(|it| matches!(it, Selects::Held)) {
        let held = builder.ins().load(types::I64, TRUSTED, value, HELD as i32);
        out_of_slot(builder, held, read_as)
    } else {
        value
    }
}

/// A value that is one of two, worked out on the side the condition took.
///
/// Both sides are written and only one runs, which is what makes this a fork rather than a choice
/// between two values: what the arm not taken would have computed is not computed, and for an arm
/// that aborts that is the difference between a run that answers and one that does not.
fn fork<A>(
    builder: &mut FunctionBuilder,
    asked: ir::Value,
    answers: types::Type,
    mut arm: A,
) -> Result<ir::Value>
where
    A: FnMut(&mut FunctionBuilder, bool) -> Result<ir::Value>,
{
    let when_taken = builder.create_block();
    let otherwise = builder.create_block();
    let after = builder.create_block();
    builder.append_block_param(after, answers);

    builder.ins().brif(asked, when_taken, &[], otherwise, &[]);
    builder.seal_block(when_taken);
    builder.seal_block(otherwise);

    for (block, taken) in [(when_taken, true), (otherwise, false)] {
        builder.switch_to_block(block);
        let answered = arm(builder, taken)?;
        builder.ins().jump(after, &[answered.into()]);
    }

    builder.seal_block(after);
    builder.switch_to_block(after);
    Ok(builder.block_params(after)[0])
}

/// Every operator the language has is named, and the ones with no lowering say so.
///
/// Named one by one rather than caught by an arm that stands for the rest: an operator added to the
/// language is then something this has to answer for, which is the only way the list stays the
/// language's rather than this file's memory of it.
///
/// The operands are lowered here and not before, because two of these decide whether the right one
/// runs at all.
fn binary(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    bindings: &mut Bindings,
    op: Op,
    left: &Node,
    right: &Node,
) -> Result<ir::Value> {
    match op {
        // `&&` and `||` stop as soon as the answer is settled, and which operands run is part of
        // what they mean rather than something a backend decides: a condition narrows what its
        // right side may compute, and run eagerly the right side would abort at the value the
        // condition exists to exclude.
        Op::And | Op::Or => {
            let settles_it = matches!(op, Op::Or);
            let asked = lower(builder, lowering, bindings, left)?;
            // What the left one is, which is what the whole of it is: a condition answers what its
            // operands answer, and reading that off the operand rather than knowing it here keeps
            // the width a fact that crossed.
            let answers = machine_type(left.ty())?;
            fork(builder, asked, answers, |builder, taken| {
                if taken == settles_it {
                    Ok(builder.ins().iconst(answers, i64::from(settles_it)))
                } else {
                    lower(builder, lowering, bindings, right)
                }
            })
        }
        _ => {
            let a = lower(builder, lowering, bindings, left)?;
            let b = lower(builder, lowering, bindings, right)?;
            match op {
                Op::Add => {
                    let sum = builder.ins().iadd(a, b);
                    let past = builder.ins().bxor(a, sum);
                    let also = builder.ins().bxor(b, sum);
                    trap_where_the_sign_bit_is_set(builder, past, also);
                    Ok(sum)
                }
                Op::Sub => Ok(difference(builder, a, b)),
                Op::Mul => Ok(product(builder, a, b)),
                Op::Eq => Ok(builder.ins().icmp(IntCC::Equal, a, b)),
                Op::Ne => Ok(builder.ins().icmp(IntCC::NotEqual, a, b)),
                Op::Lt => Ok(builder.ins().icmp(IntCC::SignedLessThan, a, b)),
                Op::Le => Ok(builder.ins().icmp(IntCC::SignedLessThanOrEqual, a, b)),
                Op::Gt => Ok(builder.ins().icmp(IntCC::SignedGreaterThan, a, b)),
                Op::Ge => Ok(builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, a, b)),
                // `/` answers the exact quotient, which is not a whole number and has no
                // representation here yet.
                Op::Div => Err(not_lowered(format!("the operator {}", op.spelt()))),
                Op::Concat => Err(not_lowered(format!("the operator {}", op.spelt()))),
                Op::And | Op::Or => unreachable!("answered above, where the right side may not run"),
            }
        }
    }
}

/// A subtraction that left the range an `Int` holds ends the computation.
///
/// The operands disagreeing in sign and the answer disagreeing with the left one is what that is.
/// A negation is this against nought, which is why the two are one function: negating the smallest
/// `Int` there is leaves the range exactly as any other subtraction does.
fn difference(builder: &mut FunctionBuilder, a: ir::Value, b: ir::Value) -> ir::Value {
    let difference = builder.ins().isub(a, b);
    let apart = builder.ins().bxor(a, b);
    let moved = builder.ins().bxor(a, difference);
    trap_where_the_sign_bit_is_set(builder, apart, moved);
    difference
}

/// A product that left the range an `Int` holds ends the computation.
///
/// Worked out at twice the width and held to what comes back when it is narrowed: the two agree
/// exactly when the product is one an `Int` holds. Said this way rather than as a division, which
/// has an operand pair of its own that no `Int` answers for.
fn product(builder: &mut FunctionBuilder, a: ir::Value, b: ir::Value) -> ir::Value {
    let a_wide = builder.ins().sextend(types::I128, a);
    let b_wide = builder.ins().sextend(types::I128, b);
    let wide = builder.ins().imul(a_wide, b_wide);
    let held = builder.ins().ireduce(types::I64, wide);
    let back = builder.ins().sextend(types::I128, held);
    let past = builder.ins().icmp(IntCC::NotEqual, wide, back);
    builder
        .ins()
        .trapnz(past, TrapCode::user(OVERFLOWED).expect("a trap code of its own"));
    held
}

/// Ends the computation where both of these have their sign bit set.
///
/// What leaving the range looks like is two facts about signs holding at once, and which two
/// depends on the operation. Each caller works out its own pair and this is what they end on.
fn trap_where_the_sign_bit_is_set(builder: &mut FunctionBuilder, one: ir::Value, other: ir::Value) {
    let both = builder.ins().band(one, other);
    let past = builder.ins().ushr_imm_u(both, 63);
    builder
        .ins()
        .trapnz(past, TrapCode::user(OVERFLOWED).expect("a trap code of its own"));
}
