//! Lowers what crossed to Cranelift IR, and lets Cranelift write the object.
//!
//! The object is for the machine this runs on. Choosing a target for another machine is a question
//! about linkers and a runtime built for it, and answering that before the code generation works
//! would be answering the easier question first.

pub mod transport;

use anyhow::{Result, anyhow, bail};
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
/// Only `Int` has one. Every other primitive is a value with a representation to design — how it is
/// held, who owns it, what frees it — and none of that is decided by giving it a width here.
fn machine_type(ty: Ty) -> Result<types::Type> {
    match ty {
        Ty::Int => Ok(types::I64),
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

    let mut bindings = Vec::with_capacity(behavior.parameters.len());
    for (at, taken) in behavior.takes.iter().enumerate() {
        let variable = builder.declare_var(machine_type(*taken)?);
        let given = builder.block_params(entry)[at];
        builder.def_var(variable, given);
        bindings.push(variable);
    }

    let answer = lower(&mut builder, &bindings, &behavior.body)?;
    builder.ins().return_(&[answer]);
    builder.finalize(frontend);
    Ok(())
}

fn lower(builder: &mut FunctionBuilder, bindings: &[Variable], node: &Node) -> Result<ir::Value> {
    Ok(match node {
        Node::Int { value, ty } => builder.ins().iconst(machine_type(*ty)?, *value),
        Node::Read { binding, .. } => {
            let variable = bindings
                .get(*binding)
                .ok_or_else(|| anyhow!("a read of binding {binding}, which nothing bound"))?;
            builder.use_var(*variable)
        }
        Node::Binary {
            op, left, right, ..
        } => {
            let a = lower(builder, bindings, left)?;
            let b = lower(builder, bindings, right)?;
            binary(builder, *op, a, b)?
        }
    })
}

/// Every operator the language has is named, and the ones with no lowering say so.
///
/// Named one by one rather than caught by an arm that stands for the rest: an operator added to the
/// language is then something this has to answer for, which is the only way the list stays the
/// language's rather than this file's memory of it.
fn binary(
    builder: &mut FunctionBuilder,
    op: Op,
    a: ir::Value,
    b: ir::Value,
) -> Result<ir::Value> {
    match op {
        Op::Add => {
            let sum = builder.ins().iadd(a, b);
            trap_on_overflow(builder, a, b, sum);
            Ok(sum)
        }
        Op::Eq
        | Op::Ne
        | Op::Lt
        | Op::Le
        | Op::Gt
        | Op::Ge
        | Op::And
        | Op::Or
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Concat => Err(not_lowered(format!("the operator {}", op.spelt()))),
    }
}

/// A sum that left the range an `Int` holds ends the computation.
///
/// Both operands agreeing in sign and the sum disagreeing with them is what overflowing is, and
/// nothing else is: written as the sign bit of `(a ^ sum) & (b ^ sum)`, which is set exactly then.
fn trap_on_overflow(builder: &mut FunctionBuilder, a: ir::Value, b: ir::Value, sum: ir::Value) {
    let left = builder.ins().bxor(a, sum);
    let right = builder.ins().bxor(b, sum);
    let both = builder.ins().band(left, right);
    let past = builder.ins().ushr_imm_u(both, 63);
    builder
        .ins()
        .trapnz(past, TrapCode::user(OVERFLOWED).expect("a trap code of its own"));
}
