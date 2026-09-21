//! Lowers what crossed to Cranelift IR, and lets Cranelift write the object.
//!
//! The object is for the machine this runs on. Choosing a target for another machine is a question
//! about linkers and a runtime built for it, and answering that before the code generation works
//! would be answering the easier question first.

pub mod transport;

use anyhow::{Result, anyhow, bail};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{AbiParam, Function, InstBuilder, TrapCode, UserFuncName, types};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::codegen::settings::{self, Configurable};
use cranelift::codegen::{Context, ir};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift::module::{Linkage, Module, default_libcall_names};
use cranelift::object::{ObjectBuilder, ObjectModule};
use souther_native_abi::behavior_symbol;
use std::fmt;
use transport::{Behavior, Node, Op, Program, TRANSPORT_VERSION, Ty};

/// An `Int` overflowing is an abort and not an answer, so the arithmetic traps rather than
/// wrapping. Which abort it was is not said here: nothing yet carries a reason out of a native run,
/// and a number invented at this end would be a second answer to a question Souther has not
/// answered once.
const OVERFLOWED: u8 = 1;

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
    // Nothing links these into a position-independent object yet, and the default on some hosts is
    // to assume one. Said here rather than left to the host so that what is emitted is the same
    // wherever it is built.
    flags.set("is_pic", "false")?;
    let isa = cranelift::native::builder()
        .map_err(|it| anyhow!("no code generator for this host: {it}"))?
        .finish(settings::Flags::new(flags))?;

    let builder = ObjectBuilder::new(isa, "souther", default_libcall_names())?;
    let mut module = ObjectModule::new(builder);
    let mut context = Context::new();
    let mut shapes = FunctionBuilderContext::new();
    let frontend = module.isa().frontend_config();

    for written in &program.modules {
        for behavior in &written.behaviors {
            let symbol = behavior_symbol(&written.name, &behavior.name);
            let signature = signature_of(behavior, module.isa().default_call_conv())?;
            let id = module.declare_function(&symbol, Linkage::Export, &signature)?;

            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            define(&mut context.func, &mut shapes, behavior, frontend)?;
            module.define_function(id, &mut context)?;
        }
    }

    Ok(module.finish().emit()?)
}

fn signature_of(behavior: &Behavior, call_conv: CallConv) -> Result<ir::Signature> {
    let mut signature = ir::Signature::new(call_conv);
    for taken in &behavior.takes {
        signature.params.push(AbiParam::new(machine_type(*taken)?));
    }
    signature
        .returns
        .push(AbiParam::new(machine_type(behavior.answers)?));
    Ok(signature)
}

/// What a value of this type is on the machine.
///
/// `Int` and `Bool` have one. Every other primitive is a value with a representation to design —
/// how it is held, who owns it, what frees it — and none of that is decided by giving it a width
/// here.
fn machine_type(ty: Ty) -> Result<types::Type> {
    match ty {
        Ty::Int => Ok(types::I64),
        Ty::Bool => Ok(types::I8),
        other => Err(not_lowered(format!("a value of type {}", other.spelt()))),
    }
}

fn define(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    behavior: &Behavior,
    frontend: TargetFrontendConfig,
) -> Result<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut bindings = Bindings::default();
    for (at, taken) in behavior.takes.iter().enumerate() {
        let variable = builder.declare_var(machine_type(*taken)?);
        let given = builder.block_params(entry)[at];
        builder.def_var(variable, given);
        bindings.at(at, variable);
    }

    let answer = lower(&mut builder, &mut bindings, &behavior.body)?;
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

fn lower(builder: &mut FunctionBuilder, bindings: &mut Bindings, node: &Node) -> Result<ir::Value> {
    Ok(match node {
        Node::Int { value, ty } => builder.ins().iconst(machine_type(*ty)?, *value),
        Node::Read { binding, .. } => {
            let variable = bindings.of(*binding)?;
            builder.use_var(variable)
        }
        Node::Neg { operand, .. } => {
            let held = lower(builder, bindings, operand)?;
            let nought = builder.ins().iconst(types::I64, 0);
            difference(builder, nought, held)
        }
        Node::Let {
            binding,
            value,
            body,
            ..
        } => {
            let held = lower(builder, bindings, value)?;
            let variable = builder.declare_var(machine_type(value.ty())?);
            builder.def_var(variable, held);
            bindings.at(*binding, variable);
            lower(builder, bindings, body)?
        }
        Node::Bool { value, ty } => builder.ins().iconst(machine_type(*ty)?, i64::from(*value)),
        Node::Binary {
            op, left, right, ..
        } => binary(builder, bindings, *op, left, right)?,
        Node::If {
            cond,
            then,
            els,
            ty,
        } => {
            let asked = lower(builder, bindings, cond)?;
            fork(builder, asked, machine_type(*ty)?, |builder, taken| {
                lower(builder, bindings, if taken { then } else { els })
            })?
        }
    })
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
            let asked = lower(builder, bindings, left)?;
            let answers = machine_type(Ty::Bool)?;
            fork(builder, asked, answers, |builder, taken| {
                if taken == settles_it {
                    Ok(builder.ins().iconst(answers, i64::from(settles_it)))
                } else {
                    lower(builder, bindings, right)
                }
            })
        }
        _ => {
            let a = lower(builder, bindings, left)?;
            let b = lower(builder, bindings, right)?;
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
