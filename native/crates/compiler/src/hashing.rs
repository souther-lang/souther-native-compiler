//! What a value hashes to, and the two functions of a type's that the runtime calls where it keeps
//! a `Set` or a `Map`.
//!
//! The runtime keeps a set as a trie over hashes and knows no type (`souther_native_abi::SET_EMPTY`),
//! so what an element hashes to and what it is equal to are handed to it by the site that asks, as
//! the addresses of two functions of this object's, one pair per type: a hasher, taking a value as
//! it stands in a slot, and an equality, taking two. Neither is kept in the set. An element stands
//! as a wider type than it was put in as, and a site asking of a `Set<S>` hands `S`'s functions over
//! whatever the set was built as.
//!
//! What makes that sound is how a hash is composed, which `souther_native_abi::HASHING` states and
//! [`hash`] and [`define`] write: a value hashes as the case it is, whatever sum it is read as, so
//! the hash an element was put in under is the hash of it under every type it can stand as without
//! being rebuilt. Where standing wider rebuilds a value (a primitive carried as a case of a union),
//! the set is rebuilt with it (`restating`), and every element is hashed again under the wider
//! type.
//!
//! Each function is given an id the first time a site asks for it and written once every body is,
//! as a comparator is (`equality`): a type is reached from inside its own hash as often as not.

use std::cell::RefCell;
use std::collections::HashMap;

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, AbiParam, Function, InstBuilder, types};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    CARRIED, DATE_HASH, DATETIME_HASH, DECIMAL_HASH, HASH_COMBINE, HASH_PRESENT, HASH_START, HELD,
    INSTANT_HASH, LIST_ELEMENTS, LIST_LENGTH, MAP_HASH, NOTHING, RATIONAL_HASH, SET_HASH, SLOT,
    STRING_HASH, TIME_HASH, field_at, member_at,
};

use crate::transport::{Case, Declaration, Prim, Ty};
use crate::{
    Lowered, Lowerings, POINTER, TRUSTED, Tagged, accepted, into_slot, machine_type, not_lowered,
    out_of_slot, runtime_call, token_of,
};

/// Which of a type's two functions one is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Kind {
    /// A value as it stands in a slot, and its hash.
    Hasher,
    /// Two values as they stand in slots, and whether they are equal: nought or one.
    Equality,
}

/// The hasher and the equality of each type a set or a map in this object is kept over, and the
/// ones whose body is still to be written.
#[derive(Default)]
pub(crate) struct ValueOps {
    by_type: RefCell<HashMap<(Kind, Ty), FuncId>>,
    owed: RefCell<Vec<Owed>>,
}

/// A function declared and not yet written.
pub(crate) struct Owed {
    pub(crate) id: FuncId,
    pub(crate) kind: Kind,
    pub(crate) ty: Ty,
    pub(crate) signature: ir::Signature,
}

impl ValueOps {
    /// The function of `kind` for `ty`, given an id now where none was yet and owed a body until
    /// [`ValueOps::owed`] hands it out.
    ///
    /// Declared whatever `ty` is: a set of a type no value of which is made still hands the
    /// runtime a hasher, which nothing calls.
    pub(crate) fn of(
        &self,
        module: &mut ObjectModule,
        kind: Kind,
        ty: &Ty,
        call_conv: CallConv,
    ) -> FuncId {
        let key = (kind, ty.clone());
        if let Some(id) = self.by_type.borrow().get(&key) {
            return *id;
        }
        let mut signature = ir::Signature::new(call_conv);
        signature.params.push(AbiParam::new(types::I64));
        match kind {
            Kind::Hasher => signature.returns.push(AbiParam::new(types::I64)),
            Kind::Equality => {
                signature.params.push(AbiParam::new(types::I64));
                signature.returns.push(AbiParam::new(types::I8));
            }
        }
        let id = accepted(module.declare_anonymous_function(&signature));
        crate::index::unique(&mut *self.by_type.borrow_mut(), key, id);
        self.owed.borrow_mut().push(Owed {
            id,
            kind,
            ty: ty.clone(),
            signature,
        });
        id
    }

    /// The address of the function of `kind` for `ty`, which is what the runtime is handed.
    pub(crate) fn address(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        kind: Kind,
        ty: &Ty,
    ) -> ir::Value {
        let call_conv = builder.func.signature.call_conv;
        let id = self.of(module, kind, ty, call_conv);
        let reached = module.declare_func_in_func(id, builder.func);
        builder.ins().func_addr(POINTER, reached)
    }

    /// The hasher and then the equality of `ty`, as the runtime takes them.
    pub(crate) fn both(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        ty: &Ty,
    ) -> [ir::Value; 2] {
        [
            self.address(builder, module, Kind::Hasher, ty),
            self.address(builder, module, Kind::Equality, ty),
        ]
    }

    /// A function whose body is still to be written. Writing one may owe more, so this is asked
    /// until it answers nothing.
    pub(crate) fn owed(&self) -> Option<Owed> {
        self.owed.borrow_mut().pop()
    }
}

/// `word` composed into `hash`, in order.
fn combine(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    hash: ir::Value,
    word: ir::Value,
) -> ir::Value {
    let word = into_slot(builder, word);
    runtime_call(builder, lowering, module, HASH_COMBINE, &[hash, word])
}

/// The hash of `value`, a value of `ty` as the machine holds it, composed as
/// `souther_native_abi::HASHING` says.
pub(crate) fn hash(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    ty: &Ty,
    value: ir::Value,
) -> Lowered<ir::Value> {
    let start = |builder: &mut FunctionBuilder| builder.ins().iconst(types::I64, HASH_START);
    match ty {
        // Every primitive is named, for the reason `machine_type` names them.
        Ty::Prim { prim } => {
            let through = match prim {
                Prim::Int | Prim::Bool => {
                    let start = start(builder);
                    return Ok(combine(builder, lowering, module, start, value));
                }
                Prim::String => STRING_HASH,
                Prim::Decimal => DECIMAL_HASH,
                Prim::Rational => RATIONAL_HASH,
                Prim::Date => DATE_HASH,
                Prim::Time => TIME_HASH,
                Prim::DateTime => DATETIME_HASH,
                Prim::Instant => INSTANT_HASH,
            };
            Ok(runtime_call(builder, lowering, module, through, &[value]))
        }
        // The checker gives a function no equality, so nothing asks what one hashes to.
        Ty::Fn { .. } => Err(not_lowered(format!("the hash of {}", ty.spelt()))),
        Ty::Var { var } => Err(crate::open_type(*var)),
        Ty::Ref {
            named: Case::Primitive { .. },
        } => crate::named_as_a_type(ty),
        // No value of it is made, so nothing asks what one hashes to; a hasher of it is still
        // handed to the runtime (`define`). What holds only it is not answered here: an empty set
        // of it stands as an empty set of anything without being rebuilt, and keeps the hash it
        // was built with, so it hashes as every empty set does, and a list and an optional as
        // every empty list and absent optional do.
        Ty::Nothing { .. } | Ty::Never { .. } => Ok(start(builder)),
        Ty::Set { .. } => Ok(runtime_call(builder, lowering, module, SET_HASH, &[value])),
        Ty::Map { map } => {
            let values = lowering
                .value_ops
                .address(builder, module, Kind::Hasher, &map.value);
            Ok(runtime_call(
                builder,
                lowering,
                module,
                MAP_HASH,
                &[value, values],
            ))
        }
        Ty::Ref {
            named: Case::Declared { .. } | Case::Language { .. },
        }
        | Ty::Union { .. }
        | Ty::Option { .. }
        | Ty::Tuple { .. }
        | Ty::List { .. } => {
            let call_conv = builder.func.signature.call_conv;
            let id = lowering.value_ops.of(module, Kind::Hasher, ty, call_conv);
            let hashing = module.declare_func_in_func(id, builder.func);
            let slot = into_slot(builder, value);
            let hashed = builder.ins().call(hashing, &[slot]);
            Ok(builder.inst_results(hashed)[0])
        }
    }
}

/// The body of a function [`ValueOps::of`] declared.
pub(crate) fn define(
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
    let ty = &owed.ty;
    match owed.kind {
        // Never called where no value of the type is made, and written all the same, since the
        // runtime is handed its address.
        Kind::Hasher if ty.has_no_value() => {
            let start = builder.ins().iconst(types::I64, HASH_START);
            builder.ins().return_(&[start]);
        }
        Kind::Hasher => {
            let value = out_of_slot(&mut builder, given[0], machine_type(ty)?);
            let hashed = Hashing {
                builder: &mut builder,
                lowering,
                module,
            }
            .body(ty, value)?;
            builder.ins().return_(&[hashed]);
        }
        Kind::Equality if ty.has_no_value() => {
            let same = builder.ins().iconst(types::I8, 1);
            builder.ins().return_(&[same]);
        }
        Kind::Equality => {
            let held = machine_type(ty)?;
            let a = out_of_slot(&mut builder, given[0], held);
            let b = out_of_slot(&mut builder, given[1], held);
            let same = crate::equality::equal(&mut builder, lowering, module, ty, a, b)?;
            builder.ins().return_(&[same]);
        }
    }
    builder.seal_all_blocks();
    builder.finalize(frontend);
    Ok(())
}

/// One hasher's body being written.
struct Hashing<'b, 'f, 'l, 'm> {
    builder: &'b mut FunctionBuilder<'f>,
    lowering: &'l Lowerings<'l>,
    module: &'m mut ObjectModule,
}

impl Hashing<'_, '_, '_, '_> {
    fn body(&mut self, ty: &Ty, value: ir::Value) -> Lowered<ir::Value> {
        match ty {
            Ty::Ref {
                named: Case::Declared { declared },
            } => match self.lowering.declared.laid(declared) {
                Declaration::Product { fields, .. } => {
                    let types: Vec<Ty> = fields.iter().map(|it| it.codec.ty()).collect();
                    let tagged = self.tagged(&Case::Declared {
                        declared: declared.clone(),
                    })?;
                    self.slots(tagged, &types, field_at, value)
                }
                Declaration::Newtype { field, .. } => {
                    let tagged = self.tagged(&Case::Declared {
                        declared: declared.clone(),
                    })?;
                    self.slots(tagged, &[field.codec.ty()], field_at, value)
                }
                Declaration::Unit { .. } => self.tagged(&Case::Declared {
                    declared: declared.clone(),
                }),
                Declaration::Sum { cases, .. } => self.cases(ty, cases, value),
            },
            Ty::Ref {
                named: named @ Case::Language { .. },
            } => self.tagged(named),
            Ty::Union { union } => self.cases(ty, union, value),
            Ty::Option { option } => self.optional(option, value),
            Ty::Tuple { tuple } => {
                let start = self.builder.ins().iconst(types::I64, HASH_START);
                self.slots(start, tuple, member_at, value)
            }
            Ty::List { list } => self.list(list, value),
            Ty::Prim { .. }
            | Ty::Ref {
                named: Case::Primitive { .. },
            }
            | Ty::Fn { .. }
            | Ty::Set { .. }
            | Ty::Map { .. }
            | Ty::Var { .. }
            | Ty::Nothing { .. }
            | Ty::Never { .. } => hash(self.builder, self.lowering, self.module, ty, value),
        }
    }

    /// Where a value carrying `case`'s token starts from: the token combined into the start.
    fn tagged(&mut self, case: &Case) -> Lowered<ir::Value> {
        let token = token_of(self.builder, self.lowering.declared, self.module, case)?;
        let start = self.builder.ins().iconst(types::I64, HASH_START);
        Ok(combine(
            self.builder,
            self.lowering,
            self.module,
            start,
            token,
        ))
    }

    /// `from` with each slot's hash combined in, in order.
    fn slots(
        &mut self,
        from: ir::Value,
        types: &[Ty],
        at: fn(usize) -> i64,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        let mut hashed = from;
        for (position, ty) in types.iter().enumerate() {
            let held = self
                .builder
                .ins()
                .load(types::I64, TRUSTED, value, at(position) as i32);
            let held = out_of_slot(self.builder, held, machine_type(ty)?);
            let one = hash(self.builder, self.lowering, self.module, ty, held)?;
            hashed = combine(self.builder, self.lowering, self.module, hashed, one);
        }
        Ok(hashed)
    }

    /// As the case the value is, by the token it carries: a declared case by its own hasher, a
    /// primitive a union carries by its token and then what it carries, and a case the language
    /// gives by its token alone. The last case is the one a value tagged by none of the others is,
    /// as a comparison reads it.
    fn cases(&mut self, ty: &Ty, members: &[Case], value: ir::Value) -> Lowered<ir::Value> {
        let which = Tagged::of(value, ty).which(self.builder);
        let leaves = self
            .lowering
            .declared
            .leaves_of(members)
            .expect("`Coherent` held every case named to be one a declaration crossed for");
        let answered = self.builder.create_block();
        self.builder.append_block_param(answered, types::I64);
        for (at, leaf) in leaves.iter().enumerate() {
            if at + 1 < leaves.len() {
                let tag = token_of(self.builder, self.lowering.declared, self.module, leaf)?;
                let is_it = self.builder.ins().icmp(IntCC::Equal, which, tag);
                let taken = self.builder.create_block();
                let next = self.builder.create_block();
                self.builder.ins().brif(is_it, taken, &[], next, &[]);
                self.builder.switch_to_block(taken);
                let hashed = self.as_the_case(leaf, value)?;
                self.builder.ins().jump(answered, &[hashed.into()]);
                self.builder.switch_to_block(next);
            } else {
                let hashed = self.as_the_case(leaf, value)?;
                self.builder.ins().jump(answered, &[hashed.into()]);
            }
        }
        self.builder.switch_to_block(answered);
        Ok(self.builder.block_params(answered)[0])
    }

    /// The hash of `value`, known to be the case `leaf`, as that case.
    fn as_the_case(&mut self, leaf: &Case, value: ir::Value) -> Lowered<ir::Value> {
        match leaf {
            Case::Declared { declared } => hash(
                self.builder,
                self.lowering,
                self.module,
                &Ty::declared(declared.clone()),
                value,
            ),
            Case::Primitive { prim } => {
                let tagged = self.tagged(leaf)?;
                let as_it = Ty::Prim { prim: *prim };
                self.slots(tagged, &[as_it], |_| CARRIED, value)
            }
            Case::Language { .. } => self.tagged(leaf),
        }
    }

    /// The start where it is absent, and what it holds combined with the mark of one present where
    /// it is not.
    fn optional(&mut self, held: &Ty, value: ir::Value) -> Lowered<ir::Value> {
        // Never present, so what it holds is never read.
        if held.has_no_value() {
            return Ok(self.builder.ins().iconst(types::I64, HASH_START));
        }
        let absent = self.builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING);
        let there = self.builder.create_block();
        let answered = self.builder.create_block();
        self.builder.append_block_param(answered, types::I64);
        let start = self.builder.ins().iconst(types::I64, HASH_START);
        self.builder
            .ins()
            .brif(absent, answered, &[start.into()], there, &[]);

        self.builder.switch_to_block(there);
        let present = self.builder.ins().iconst(types::I64, HASH_PRESENT);
        let hashed = self.slots(present, std::slice::from_ref(held), |_| HELD, value)?;
        self.builder.ins().jump(answered, &[hashed.into()]);

        self.builder.switch_to_block(answered);
        Ok(self.builder.block_params(answered)[0])
    }

    /// The length, and then each element in order.
    fn list(&mut self, element: &Ty, value: ir::Value) -> Lowered<ir::Value> {
        let length = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, value, LIST_LENGTH as i32);
        let start = self.builder.ins().iconst(types::I64, HASH_START);
        let from = combine(self.builder, self.lowering, self.module, start, length);
        // Empty, so no element is read.
        if element.has_no_value() {
            return Ok(from);
        }

        let head = self.builder.create_block();
        self.builder.append_block_param(head, types::I64);
        self.builder.append_block_param(head, types::I64);
        let step = self.builder.create_block();
        let walked = self.builder.create_block();
        self.builder.append_block_param(walked, types::I64);
        let first = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().jump(head, &[first.into(), from.into()]);

        self.builder.switch_to_block(head);
        let index = self.builder.block_params(head)[0];
        let hashed = self.builder.block_params(head)[1];
        let inside = self
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, index, length);
        self.builder
            .ins()
            .brif(inside, step, &[], walked, &[hashed.into()]);

        self.builder.switch_to_block(step);
        let along = self.builder.ins().imul_imm_s(index, SLOT);
        let at = self.builder.ins().iadd(value, along);
        let held = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, at, LIST_ELEMENTS as i32);
        let held = out_of_slot(self.builder, held, machine_type(element)?);
        let one = hash(self.builder, self.lowering, self.module, element, held)?;
        let hashed = combine(self.builder, self.lowering, self.module, hashed, one);
        let next = self.builder.ins().iadd_imm_s(index, 1);
        self.builder.ins().jump(head, &[next.into(), hashed.into()]);

        self.builder.switch_to_block(walked);
        Ok(self.builder.block_params(walked)[0])
    }
}
