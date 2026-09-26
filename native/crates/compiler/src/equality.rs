//! Whether two values of one type are equal, which is what they are made of compared and not where
//! they are kept.
//!
//! A number and a truth are compared where they stand, and text by the runtime's walk. Everything
//! else is held as an address, and two of those built apart are equal where what they hold is:
//! a declared value by its fields, a newtype by what it wraps, a sum by which case it is and then
//! as that case, an optional by whether it holds a value and then by the value, a tuple member by
//! member, and a list by its length and then element by element.
//!
//! Each of those is a function of this object's, one per type, and not code written at every site
//! that says `==`. A type is reached from inside its own comparison as often as not: an element of
//! a list, a field of a declared type, and a declared type holding a list of itself. So a type's
//! function is given an id the first time any comparison asks for it, and its body is written
//! later, once every body of the object is ([`Comparators::owed`]). A comparison reaching a type
//! whose function is still to be written calls it by that id all the same, which is what keeps a
//! type that holds itself from being unfolded without end. Nothing here guards against a value
//! that holds itself: a value is built from values that already exist, so none does.
//!
//! Two values of the same type only. A value compared with a value of another type (a newtype with
//! a bare literal, a sum with one of its cases) is a reading the checker states on the operator,
//! and is not answered here.

use std::cell::RefCell;
use std::collections::HashMap;

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, AbiParam, Function, InstBuilder, types};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    CARRIED, DECIMAL_COMPARE, HELD, LIST_ELEMENTS, LIST_LENGTH, NOTHING, SLOT, field_at, member_at,
};

use crate::transport::{Case, Declaration, Prim, Ty};
use crate::{
    Lowered, Lowerings, TRUSTED, Tagged, accepted, machine_type, not_lowered, out_of_slot, token_of,
};

/// The function comparing two values of each type some comparison in this object asked about,
/// and the ones whose body is still to be written.
#[derive(Default)]
pub(crate) struct Comparators {
    /// By the type itself, which is what a comparator is one of per object.
    by_type: RefCell<HashMap<Ty, FuncId>>,
    owed: RefCell<Vec<Owed>>,
}

impl Comparators {
    /// The function comparing two values of `ty`, given an id now where none was yet and owed a
    /// body until [`Comparators::owed`] hands it out.
    fn of(&self, module: &mut ObjectModule, ty: &Ty, call_conv: CallConv) -> Lowered<FuncId> {
        if let Some(id) = self.by_type.borrow().get(ty) {
            return Ok(*id);
        }
        let held = machine_type(ty)?;
        let mut signature = ir::Signature::new(call_conv);
        signature.params.push(AbiParam::new(held));
        signature.params.push(AbiParam::new(held));
        signature.returns.push(AbiParam::new(types::I8));
        let id = accepted(module.declare_anonymous_function(&signature));
        crate::index::unique(&mut *self.by_type.borrow_mut(), ty.clone(), id);
        self.owed.borrow_mut().push(Owed {
            id,
            ty: ty.clone(),
            signature,
        });
        Ok(id)
    }

    /// A function whose body is still to be written, and the type it compares. Writing one may owe
    /// more, so this is asked until it answers nothing.
    pub(crate) fn owed(&self) -> Option<Owed> {
        self.owed.borrow_mut().pop()
    }
}

/// A comparator declared and not yet written.
pub(crate) struct Owed {
    pub(crate) id: FuncId,
    pub(crate) ty: Ty,
    pub(crate) signature: ir::Signature,
}

/// Whether `a` and `b`, two values of `ty`, are equal: a truth, as `==` answers one.
pub(crate) fn equal(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    ty: &Ty,
    a: ir::Value,
    b: ir::Value,
) -> Lowered<ir::Value> {
    match ty {
        // Every primitive is named, for the reason `machine_type` names them.
        Ty::Prim { prim } => match prim {
            Prim::Int | Prim::Bool => Ok(builder.ins().icmp(IntCC::Equal, a, b)),
            Prim::String => {
                let comparing = module.declare_func_in_func(lowering.compare_text, builder.func);
                let compared = builder.ins().call(comparing, &[a, b]);
                let answered = builder.inst_results(compared)[0];
                Ok(builder.ins().icmp_imm_s(IntCC::Equal, answered, 0))
            }
            // By amount and not by scale, which the runtime compares: `1.0` and `1.00` are equal.
            Prim::Decimal => {
                let compared =
                    crate::runtime_call(builder, lowering, module, DECIMAL_COMPARE, &[a, b]);
                Ok(builder.ins().icmp_imm_s(IntCC::Equal, compared, 0))
            }
            // By the day, the time of day or the moment they name, which the runtime compares: two
            // made apart are equal where they name one, and their addresses say nothing of it.
            Prim::Date | Prim::Time | Prim::DateTime | Prim::Instant => {
                let compared = crate::runtime_call(
                    builder,
                    lowering,
                    module,
                    crate::temporal_compare(*prim),
                    &[a, b],
                );
                Ok(builder.ins().icmp_imm_s(IntCC::Equal, compared, 0))
            }
            Prim::Rational | Prim::Raw => Err(not_lowered(format!(
                "a comparison of two values of type {}",
                prim.spelt()
            ))),
        },
        // What a function does is not something two of them can be asked to agree on, and the
        // checker gives a function no equality to ask with.
        Ty::Fn { .. } => Err(not_lowered(format!(
            "a comparison of two values of {}",
            ty.spelt()
        ))),
        Ty::Var { var } => crate::laid_out_nowhere(*var),
        // No value of it is ever made, so there are never two to compare; one asked for is refused
        // the way a value of it is (`machine_type`), and not answered with a truth nothing earned.
        Ty::Nothing { .. } => Err(not_lowered(format!(
            "a comparison of two values of {}",
            ty.spelt()
        ))),
        Ty::Ref {
            named: Case::Primitive { .. },
        } => crate::named_as_a_type(ty),
        // A case the language gives holds nothing, so two values of one are the one value.
        Ty::Ref {
            named: Case::Language { .. },
        } => Ok(builder.ins().iconst(types::I8, 1)),
        Ty::Never { .. } => Err(not_lowered(format!(
            "a comparison of two values of {}",
            ty.spelt()
        ))),
        Ty::Ref {
            named: Case::Declared { .. },
        }
        | Ty::Union { .. }
        | Ty::Option { .. }
        | Ty::Tuple { .. }
        | Ty::List { .. }
        | Ty::Set { .. }
        | Ty::Map { .. } => {
            let call_conv = builder.func.signature.call_conv;
            let id = lowering.comparators.of(module, ty, call_conv)?;
            let comparing = module.declare_func_in_func(id, builder.func);
            let compared = builder.ins().call(comparing, &[a, b]);
            Ok(builder.inst_results(compared)[0])
        }
    }
}

/// The body of the function comparing two values of `ty`, which [`Comparators::of`] declared.
pub(crate) fn define_comparator(
    func: &mut Function,
    shapes: &mut FunctionBuilderContext,
    ty: &Ty,
    frontend: TargetFrontendConfig,
    lowering: &Lowerings,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(func, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    let (a, b) = {
        let given = builder.block_params(entry);
        (given[0], given[1])
    };
    let unequal = builder.create_block();
    let mut comparing = Comparing {
        builder: &mut builder,
        lowering,
        module,
        unequal,
    };
    comparing.body(ty, a, b)?;
    builder.switch_to_block(unequal);
    let no = builder.ins().iconst(types::I8, 0);
    builder.ins().return_(&[no]);
    builder.seal_all_blocks();
    builder.finalize(frontend);
    Ok(())
}

/// One comparator's body being written: every way it finds the two values unequal leaves for the
/// one block that answers so.
struct Comparing<'b, 'f, 'l, 'm> {
    builder: &'b mut FunctionBuilder<'f>,
    lowering: &'l Lowerings<'l>,
    module: &'m mut ObjectModule,
    unequal: ir::Block,
}

impl Comparing<'_, '_, '_, '_> {
    fn body(&mut self, ty: &Ty, a: ir::Value, b: ir::Value) -> Lowered<()> {
        match ty {
            Ty::Ref {
                named: Case::Declared { declared },
            } => match self.lowering.declared.laid(declared) {
                // A value of a type built from fields is tagged with that type and no other, so
                // two of one type differ in nothing but their fields.
                Declaration::Product { fields, .. } => {
                    let types: Vec<Ty> = fields.iter().map(|it| it.codec.ty()).collect();
                    self.slots(&types, field_at, a, b)?;
                }
                Declaration::Newtype { field, .. } => {
                    self.slots(&[field.codec.ty()], field_at, a, b)?;
                }
                // The one value there is.
                Declaration::Unit { .. } => {}
                Declaration::Sum { cases, .. } => self.cases(ty, cases, a, b)?,
            },
            Ty::Union { union } => self.cases(ty, union, a, b)?,
            Ty::Option { option } => self.optional(option, a, b)?,
            Ty::Tuple { tuple } => self.slots(tuple, member_at, a, b)?,
            Ty::List { list } => self.list(list, a, b)?,
            Ty::Prim { .. }
            | Ty::Ref {
                named: Case::Primitive { .. } | Case::Language { .. },
            }
            | Ty::Fn { .. }
            | Ty::Set { .. }
            | Ty::Map { .. }
            | Ty::Nothing { .. }
            | Ty::Never { .. } => {
                let same = equal(self.builder, self.lowering, self.module, ty, a, b)?;
                self.unless(same);
            }
            Ty::Var { var } => crate::laid_out_nowhere(*var),
        }
        let yes = self.builder.ins().iconst(types::I8, 1);
        self.builder.ins().return_(&[yes]);
        Ok(())
    }

    /// Leaves for the unequal block where `same` is false, and carries on where it is true.
    fn unless(&mut self, same: ir::Value) {
        let next = self.builder.create_block();
        self.builder.ins().brif(same, next, &[], self.unequal, &[]);
        self.builder.switch_to_block(next);
    }

    /// The two values' slots compared one after another, each at the type it holds.
    fn slots(
        &mut self,
        types: &[Ty],
        at: fn(usize) -> i64,
        a: ir::Value,
        b: ir::Value,
    ) -> Lowered<()> {
        for (position, ty) in types.iter().enumerate() {
            let one = self.read(a, at(position) as i32, ty)?;
            let other = self.read(b, at(position) as i32, ty)?;
            let same = equal(self.builder, self.lowering, self.module, ty, one, other)?;
            self.unless(same);
        }
        Ok(())
    }

    fn read(&mut self, value: ir::Value, offset: i32, ty: &Ty) -> Lowered<ir::Value> {
        let held = self.builder.ins().load(types::I64, TRUSTED, value, offset);
        Ok(out_of_slot(self.builder, held, machine_type(ty)?))
    }

    /// Which case each is, by the token it carries, and then the two as that case: a declared case
    /// by its own comparator, reached by the id, a primitive a union carried by what it carries,
    /// and a case the language gives, which holds nothing, as equal.
    fn cases(&mut self, ty: &Ty, members: &[Case], a: ir::Value, b: ir::Value) -> Lowered<()> {
        let one = Tagged::of(a, ty).which(self.builder);
        let other = Tagged::of(b, ty).which(self.builder);
        let same = self.builder.ins().icmp(IntCC::Equal, one, other);
        self.unless(same);
        let leaves = self
            .lowering
            .declared
            .leaves_of(members)
            .expect("`Coherent` held every case named to be one a declaration crossed for");
        let answered = self.builder.create_block();
        self.builder.append_block_param(answered, types::I8);
        for (at, leaf) in leaves.iter().enumerate() {
            // The last case is the one a value tagged by none of the others is, which the checker
            // settles every value of the sum to be one of.
            if at + 1 < leaves.len() {
                let tag = token_of(self.builder, self.lowering.declared, self.module, leaf)?;
                let is_it = self.builder.ins().icmp(IntCC::Equal, one, tag);
                let taken = self.builder.create_block();
                let next = self.builder.create_block();
                self.builder.ins().brif(is_it, taken, &[], next, &[]);
                self.builder.switch_to_block(taken);
                let same = self.as_the_case(leaf, a, b)?;
                self.builder.ins().jump(answered, &[same.into()]);
                self.builder.switch_to_block(next);
            } else {
                let same = self.as_the_case(leaf, a, b)?;
                self.builder.ins().jump(answered, &[same.into()]);
            }
        }
        self.builder.switch_to_block(answered);
        let same = self.builder.block_params(answered)[0];
        self.unless(same);
        Ok(())
    }

    /// Whether `a` and `b`, both known to be the case `leaf`, are equal as it.
    fn as_the_case(&mut self, leaf: &Case, a: ir::Value, b: ir::Value) -> Lowered<ir::Value> {
        match leaf {
            Case::Declared { declared } => {
                let as_it = Ty::declared(declared.clone());
                equal(self.builder, self.lowering, self.module, &as_it, a, b)
            }
            Case::Primitive { prim } => {
                let as_it = Ty::Prim { prim: *prim };
                let one = self.read(a, CARRIED as i32, &as_it)?;
                let other = self.read(b, CARRIED as i32, &as_it)?;
                equal(self.builder, self.lowering, self.module, &as_it, one, other)
            }
            Case::Language { .. } => Ok(self.builder.ins().iconst(types::I8, 1)),
        }
    }

    /// Both absent, or both present and what they hold equal.
    fn optional(&mut self, held: &Ty, a: ir::Value, b: ir::Value) -> Lowered<()> {
        let one_absent = self.builder.ins().icmp_imm_s(IntCC::Equal, a, NOTHING);
        let other_absent = self.builder.ins().icmp_imm_s(IntCC::Equal, b, NOTHING);
        let either = self.builder.ins().bor(one_absent, other_absent);
        let absent = self.builder.create_block();
        let present = self.builder.create_block();
        self.builder.ins().brif(either, absent, &[], present, &[]);

        self.builder.switch_to_block(absent);
        let both = self.builder.ins().band(one_absent, other_absent);
        self.builder.ins().return_(&[both]);

        self.builder.switch_to_block(present);
        self.slots(std::slice::from_ref(held), |_| HELD, a, b)
    }

    /// The same length, and then each element equal to the one at its index.
    fn list(&mut self, element: &Ty, a: ir::Value, b: ir::Value) -> Lowered<()> {
        let length = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, a, LIST_LENGTH as i32);
        let also = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, b, LIST_LENGTH as i32);
        let same = self.builder.ins().icmp(IntCC::Equal, length, also);
        self.unless(same);

        let head = self.builder.create_block();
        self.builder.append_block_param(head, types::I64);
        let walked = self.builder.create_block();
        let step = self.builder.create_block();
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
        let one_at = self.builder.ins().iadd(a, along);
        let other_at = self.builder.ins().iadd(b, along);
        let one = self.read(one_at, LIST_ELEMENTS as i32, element)?;
        let other = self.read(other_at, LIST_ELEMENTS as i32, element)?;
        let same = equal(
            self.builder,
            self.lowering,
            self.module,
            element,
            one,
            other,
        )?;
        self.unless(same);
        let next = self.builder.ins().iadd_imm_s(index, 1);
        self.builder.ins().jump(head, &[next.into()]);

        self.builder.switch_to_block(walked);
        Ok(())
    }
}
