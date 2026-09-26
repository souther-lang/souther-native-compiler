//! Which of two values of one type comes first, which is what `<`, `<=`, `>` and `>=` ask.
//!
//! The language orders a number, text and an enumeration, and a newtype over one of those, which is
//! ordered as what it wraps (ADR-0047). So a newtype is opened first, all the way down, and what is
//! left is one of the three.
//!
//! An enumeration is ordered by where each case stands in its declaration (ADR-0069): the leaves a
//! sum walks into, each at the place it was first reached. So is a value of one of its cases, and
//! of a union of them, where one enumeration and no other places them. The token a value carries says which
//! case it is and nothing about where that case stands, so it is looked up among the leaves and
//! never compared as an address. Where two tokens were put is the linker's to decide.

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, types};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::Module;
use cranelift::object::ObjectModule;

use crate::transport::{Case, Op, Prim, Ty};
use crate::{Lowered, Lowerings, Tagged, as_a_whole_number, not_lowered, opened, token_of};
use souther_native_abi::DECIMAL_COMPARE;

/// Whether `a` and `b`, two values of `ty`, stand as `op` asks: a truth, as `<` answers one.
pub(crate) fn ordered(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    op: Op,
    ty: &Ty,
    a: ir::Value,
    b: ir::Value,
) -> Lowered<ir::Value> {
    let (opened_ty, a) = opened(builder, lowering.declared, ty, a)?;
    let (_, b) = opened(builder, lowering.declared, ty, b)?;
    let ty = &opened_ty;
    let condition = as_a_whole_number(op);
    match ty {
        // Every primitive is named, for the reason `machine_type` names them.
        Ty::Prim { prim } => match prim {
            Prim::Int => Ok(builder.ins().icmp(condition, a, b)),
            // What text is ordered by is the runtime's to say, and it is a walk over what the two
            // hold, not a comparison of the two addresses.
            Prim::String => {
                let comparing = module.declare_func_in_func(lowering.compare_text, builder.func);
                let compared = builder.ins().call(comparing, &[a, b]);
                let answered = builder.inst_results(compared)[0];
                Ok(builder.ins().icmp_imm_s(condition, answered, 0))
            }
            // Two truths are equal or they are not, and nothing orders them, nor raw bytes: an
            // order over either is one the checker never writes.
            Prim::Bool | Prim::Raw => Err(unordered(op, ty)),
            // By amount, whatever the scales, which the runtime compares.
            Prim::Decimal => {
                let compared =
                    crate::runtime_call(builder, lowering, module, DECIMAL_COMPARE, &[a, b]);
                Ok(builder.ins().icmp_imm_s(condition, compared, 0))
            }
            Prim::Rational | Prim::Date | Prim::Time | Prim::DateTime | Prim::Instant => {
                Err(not_lowered(format!(
                    "a comparison of two values of type {}",
                    prim.spelt()
                )))
            }
        },
        // A value of an enumeration, of one of its cases, or of a union of them, ordered by the
        // one enumeration that places them (ADR-0069). That is the type itself where it is one, and
        // the sum listing it where it is a case: a case is not ordered on its own account, since
        // one unit may be a case of two sums that place it differently.
        Ty::Ref {
            named: Case::Primitive { .. } | Case::Language { .. },
        } => crate::named_as_a_type(ty),
        Ty::Ref {
            named: Case::Declared { .. },
        }
        | Ty::Union { .. } => {
            let Some(enumeration) = lowering
                .declared
                .enumeration_of(ty)
                .expect("`Coherent` held every case named to be one a declaration crossed for")
            else {
                return Err(not_lowered(format!(
                    "{} over two values of {}, which no one enumeration this document carries \
                     places",
                    op.spelt(),
                    ty.spelt()
                )));
            };
            let leaves = lowering
                .declared
                .leaves_of(&[Case::Declared {
                    declared: enumeration,
                }])
                .expect("`Coherent` held every case named to be one a declaration crossed for");
            let one = place(builder, lowering, module, &leaves, Tagged::of(a, ty))?;
            let other = place(builder, lowering, module, &leaves, Tagged::of(b, ty))?;
            Ok(builder.ins().icmp(condition, one, other))
        }
        Ty::Option { .. }
        | Ty::Tuple { .. }
        | Ty::List { .. }
        | Ty::Set { .. }
        | Ty::Map { .. }
        | Ty::Fn { .. }
        | Ty::Nothing { .. }
        | Ty::Never { .. } => Err(unordered(op, ty)),
        Ty::Var { var } => crate::laid_out_nowhere(*var),
    }
}

/// Where the case `value` is stands among `leaves`, counted from nought.
///
/// The last is the one a value tagged by none of the others is, which the checker settles every
/// value of the enumeration to be one of.
fn place(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    leaves: &[Case],
    value: Tagged,
) -> Lowered<ir::Value> {
    let which = value.which(builder);
    let last = leaves.len() - 1;
    let mut at = builder.ins().iconst(types::I64, last as i64);
    for (position, leaf) in leaves[..last].iter().enumerate().rev() {
        let tag = token_of(builder, lowering.declared, module, leaf)?;
        let is_it = builder.ins().icmp(IntCC::Equal, which, tag);
        let here = builder.ins().iconst(types::I64, position as i64);
        at = builder.ins().select(is_it, here, at);
    }
    Ok(at)
}

/// An order over a type the language gives none, which the checker never writes.
fn unordered(op: Op, ty: &Ty) -> crate::NotLowered {
    not_lowered(format!(
        "{} over two values of {}, which the language does not order",
        op.spelt(),
        ty.spelt()
    ))
}
