//! Which of two values of one type comes first, which is what `<`, `<=`, `>` and `>=` ask.
//!
//! The language orders a number, text and an enumeration, and a newtype over one of those, which is
//! ordered as what it wraps (ADR-0047). So a newtype is opened first, all the way down, and what is
//! left is one of the three.
//!
//! Which order two values are placed on is the checker's answer (`Core.OrderingBasis`), carried on
//! the comparison and on the sort: a primitive, or the enumeration that places a case. A case is
//! not ordered on its own account, since one unit may be a case of two enumerations that place it
//! differently, and so the basis is read here and never looked for.
//!
//! An enumeration is ordered by where each case stands in its declaration (ADR-0069): the leaves a
//! sum walks into, each at the place it was first reached. The token a value carries says which
//! case it is and nothing about where that case stands, so it is looked up among the leaves and
//! never compared as an address. Where two tokens were put is the linker's to decide.

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, types};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::Module;
use cranelift::object::ObjectModule;

use crate::transport::{Case, Op, Prim, Ty};
use crate::{Lowered, Lowerings, Tagged, as_a_whole_number, not_lowered, opened, token_of};
use souther_native_abi::{DECIMAL_COMPARE, RATIONAL_COMPARE};

/// Two values to be ordered: the type they are held as, and the type whose order places them, as
/// the checker settled it (`Core.OrderingBasis`). The basis is none only where the checker placed
/// nothing, which it does only where there is no value to place.
#[derive(Clone, Copy)]
pub(crate) struct Placing<'t> {
    pub(crate) ty: &'t Ty,
    pub(crate) basis: Option<&'t Ty>,
}

/// Whether `a` and `b`, two values `placing` says how to order, stand as `op` asks: a truth, as `<`
/// answers one.
pub(crate) fn ordered(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    op: Op,
    placing: Placing,
    a: ir::Value,
    b: ir::Value,
) -> Lowered<ir::Value> {
    let Placing { ty, basis } = placing;
    let Some(basis) = basis else {
        return Err(unordered(op, ty));
    };
    let (opened_ty, a) = opened(builder, lowering.declared, ty, a)?;
    let (_, b) = opened(builder, lowering.declared, ty, b)?;
    let condition = as_a_whole_number(op);
    let ty = basis;
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
            // Two truths are equal or they are not, and nothing orders them: an order over
            // them is one the checker never writes.
            Prim::Bool => Err(unordered(op, ty)),
            // By amount, whatever the scales, which the runtime compares.
            Prim::Decimal => {
                let compared =
                    crate::runtime_call(builder, lowering, module, DECIMAL_COMPARE, &[a, b]);
                Ok(builder.ins().icmp_imm_s(condition, compared, 0))
            }
            // Chronological, which the runtime compares: to the second for the three a clock reads,
            // and to the nanosecond for an `Instant`.
            Prim::Date | Prim::Time | Prim::DateTime | Prim::Instant => {
                let compared = crate::runtime_call(
                    builder,
                    lowering,
                    module,
                    crate::temporal_compare(*prim),
                    &[a, b],
                );
                Ok(builder.ins().icmp_imm_s(condition, compared, 0))
            }
            // By exact value, which the runtime compares.
            Prim::Rational => {
                let compared =
                    crate::runtime_call(builder, lowering, module, RATIONAL_COMPARE, &[a, b]);
                Ok(builder.ins().icmp_imm_s(condition, compared, 0))
            }
        },
        // A value of an enumeration, of one of its cases, or of a union of them, placed among
        // the enumeration's leaves (ADR-0069). `Coherent` held the basis to be an enumeration that
        // lists every leaf of what the values are (`Declared::orders`).
        Ty::Ref {
            named: Case::Primitive { .. } | Case::Language { .. },
        } => crate::named_as_a_type(ty),
        Ty::Ref {
            named: named @ Case::Declared { .. },
        } => {
            let leaves = lowering
                .declared
                .leaves_of(std::slice::from_ref(named))
                .expect("`Coherent` held every case named to be one a declaration crossed for");
            let one = place(
                builder,
                lowering,
                module,
                &leaves,
                Tagged::of(a, &opened_ty),
            )?;
            let other = place(
                builder,
                lowering,
                module,
                &leaves,
                Tagged::of(b, &opened_ty),
            )?;
            Ok(builder.ins().icmp(condition, one, other))
        }
        Ty::Union { .. } => Err(unordered(op, ty)),
        Ty::Option { .. }
        | Ty::Tuple { .. }
        | Ty::List { .. }
        | Ty::Set { .. }
        | Ty::Map { .. }
        | Ty::Fn { .. }
        | Ty::Nothing { .. }
        | Ty::Never { .. } => Err(unordered(op, ty)),
        Ty::Var { var } => Err(crate::open_type(*var)),
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
