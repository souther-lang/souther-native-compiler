//! What a set or a map is to a host: a list of its members, and a list of pairs of a key and its
//! value.
//!
//! A host reaches a list through functions for the shape its element crosses in, and those know
//! no type: the functions for a list of one declared type are the ones for a list of any other
//! (`host`). A set cannot be built that way. Which members of a list are one member is the
//! element's equality, and what it hashes to is its type's, which the shape does not say. So a set
//! crosses as the list of its members, in the shape a list does, and is made a set here, where the
//! type is known: at the place a value of the model is handed to a host or taken from one, the
//! whole of it is turned into its host's form or out of it, down through what holds it, before
//! anything shape-driven reads it. A map is the list of its entries, each the pair of its key and
//! its value.
//!
//! The order the list is in is the trie's, which is no order the language says anything of, as
//! `Set.toList`'s is not (spec §stdlib-set). A list a host hands over in which two members are one,
//! or two keys, is taken as `Set.fromList` and `Map.fromList` take it: one member of each, and the
//! later pair's value.
//!
//! Only what the model holds a set in on its own is turned: an optional, a tuple, a list, and a
//! set or a map of those. A value of a declared type crosses as a value, and a set in one of its
//! fields crosses where that field is read or built, which is typed too. A function value crosses
//! through functions for its shape, where nothing is known of a type to turn a set by, so a set
//! in what a function value takes or answers is refused where the function would cross (`host`).

use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, types};
use cranelift::codegen::isa::CallConv;
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    HELD, LIST_ELEMENTS, LIST_LENGTH, MAP_FROM_LIST, MAP_TO_LIST, NOTHING, SET_FROM_LIST,
    SET_TO_LIST, SLOT, member_at, room_for_held, room_for_list, room_for_members,
};

use crate::hashing::ValueOps;
use crate::transport::Ty;
use crate::{Lowered, POINTER, TRUSTED, import_runtime, into_slot, machine_type, out_of_slot};

/// Whether a value of `ty` holds a set or a map where it is turned: in itself, or in what an
/// optional, a tuple, a list, a set or a map of it holds.
pub(crate) fn holds_a_collection(ty: &Ty) -> bool {
    match ty {
        Ty::Set { .. } | Ty::Map { .. } => true,
        Ty::Option { option: held } | Ty::List { list: held } => holds_a_collection(held),
        Ty::Tuple { tuple } => tuple.iter().any(holds_a_collection),
        Ty::Prim { .. }
        | Ty::Ref { .. }
        | Ty::Union { .. }
        | Ty::Fn { .. }
        | Ty::Var { .. }
        | Ty::Nothing { .. }
        | Ty::Never { .. } => false,
    }
}

/// What a value of `ty` is to a host: every set in it a list of its members, and every map a list
/// of pairs.
pub(crate) fn form_of(ty: &Ty) -> Ty {
    match ty {
        Ty::Set { set } => Ty::List {
            list: Box::new(form_of(set)),
        },
        Ty::Map { map } => Ty::List {
            list: Box::new(Ty::Tuple {
                tuple: vec![form_of(&map.key), form_of(&map.value)],
            }),
        },
        Ty::Option { option } => Ty::Option {
            option: Box::new(form_of(option)),
        },
        Ty::List { list } => Ty::List {
            list: Box::new(form_of(list)),
        },
        Ty::Tuple { tuple } => Ty::Tuple {
            tuple: tuple.iter().map(form_of).collect(),
        },
        other => other.clone(),
    }
}

/// What turning a value into its host's form and out of it is written with.
pub(crate) struct Turning<'t> {
    pub module: &'t mut ObjectModule,
    pub allocate: FuncId,
    pub value_ops: &'t ValueOps,
    pub call_conv: CallConv,
}

impl Turning<'_> {
    fn runtime(
        &mut self,
        builder: &mut FunctionBuilder,
        name: &str,
        handed: &[ir::Value],
    ) -> ir::Value {
        let id = import_runtime(self.module, name, self.call_conv);
        let reaching = self.module.declare_func_in_func(id, builder.func);
        let called = builder.ins().call(reaching, handed);
        builder.inst_results(called)[0]
    }

    fn room(&mut self, builder: &mut FunctionBuilder, size: ir::Value) -> ir::Value {
        let taking = self
            .module
            .declare_func_in_func(self.allocate, builder.func);
        let taken = builder.ins().call(taking, &[size]);
        builder.inst_results(taken)[0]
    }

    /// `value`, of `ty`, as a host is handed it ([`form_of`]).
    pub(crate) fn handed(
        &mut self,
        builder: &mut FunctionBuilder,
        ty: &Ty,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        self.turned(builder, ty, value, Way::ToHost)
    }

    /// `value`, of `ty`'s host form, as the model holds a value of `ty`.
    pub(crate) fn taken(
        &mut self,
        builder: &mut FunctionBuilder,
        ty: &Ty,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        self.turned(builder, ty, value, Way::FromHost)
    }

    fn turned(
        &mut self,
        builder: &mut FunctionBuilder,
        ty: &Ty,
        value: ir::Value,
        way: Way,
    ) -> Lowered<ir::Value> {
        if !holds_a_collection(ty) {
            return Ok(value);
        }
        match ty {
            Ty::Set { set: element } => match way {
                Way::ToHost => {
                    let listed = self.runtime(builder, SET_TO_LIST, &[value]);
                    self.list(builder, element, listed, way)
                }
                Way::FromHost => {
                    let listed = self.list(builder, element, value, way)?;
                    let [hasher, equality] = self.value_ops.both(builder, self.module, element);
                    Ok(self.runtime(builder, SET_FROM_LIST, &[listed, hasher, equality]))
                }
            },
            Ty::Map { map } => {
                let pair = Ty::Tuple {
                    tuple: vec![(*map.key).clone(), (*map.value).clone()],
                };
                match way {
                    Way::ToHost => {
                        let listed = self.runtime(builder, MAP_TO_LIST, &[value]);
                        self.list(builder, &pair, listed, way)
                    }
                    Way::FromHost => {
                        let listed = self.list(builder, &pair, value, way)?;
                        let [hasher, equality] =
                            self.value_ops.both(builder, self.module, &map.key);
                        Ok(self.runtime(builder, MAP_FROM_LIST, &[listed, hasher, equality]))
                    }
                }
            }
            Ty::Option { option: held } => {
                let absent = builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING);
                let there = builder.create_block();
                let done = builder.create_block();
                builder.append_block_param(done, POINTER);
                builder
                    .ins()
                    .brif(absent, done, &[value.into()], there, &[]);

                builder.switch_to_block(there);
                let slot = builder.ins().load(types::I64, TRUSTED, value, HELD as i32);
                let inner = out_of_slot(builder, slot, machine_type(held)?);
                let inner = self.turned(builder, held, inner, way)?;
                let size = builder.ins().iconst(types::I64, room_for_held());
                let holding = self.room(builder, size);
                let slot = into_slot(builder, inner);
                builder.ins().store(TRUSTED, slot, holding, HELD as i32);
                builder.ins().jump(done, &[holding.into()]);

                builder.switch_to_block(done);
                Ok(builder.block_params(done)[0])
            }
            Ty::Tuple { tuple } => {
                let size = builder
                    .ins()
                    .iconst(types::I64, room_for_members(tuple.len()));
                let turned = self.room(builder, size);
                for (at, member) in tuple.iter().enumerate() {
                    let slot = builder
                        .ins()
                        .load(types::I64, TRUSTED, value, member_at(at) as i32);
                    let held = out_of_slot(builder, slot, machine_type(member)?);
                    let held = self.turned(builder, member, held, way)?;
                    let slot = into_slot(builder, held);
                    builder
                        .ins()
                        .store(TRUSTED, slot, turned, member_at(at) as i32);
                }
                Ok(turned)
            }
            Ty::List { list: element } => self.list(builder, element, value, way),
            _ => unreachable!(
                "{} holds no set, which `holds_a_collection` said",
                ty.spelt()
            ),
        }
    }

    /// A list of `element`s, each element turned the way `way` says, as a list of its own.
    fn list(
        &mut self,
        builder: &mut FunctionBuilder,
        element: &Ty,
        list: ir::Value,
        way: Way,
    ) -> Lowered<ir::Value> {
        if !holds_a_collection(element) {
            return Ok(list);
        }
        let length = builder
            .ins()
            .load(types::I64, TRUSTED, list, LIST_LENGTH as i32);
        let along = builder.ins().imul_imm_s(length, SLOT);
        let size = builder.ins().iadd_imm_s(along, room_for_list(0));
        let turned = self.room(builder, size);
        builder
            .ins()
            .store(TRUSTED, length, turned, LIST_LENGTH as i32);

        let head = builder.create_block();
        builder.append_block_param(head, types::I64);
        let step = builder.create_block();
        let done = builder.create_block();
        let start = builder.ins().iconst(types::I64, 0);
        builder.ins().jump(head, &[start.into()]);

        builder.switch_to_block(head);
        let index = builder.block_params(head)[0];
        let inside = builder.ins().icmp(IntCC::SignedLessThan, index, length);
        builder.ins().brif(inside, step, &[], done, &[]);

        builder.switch_to_block(step);
        let along = builder.ins().imul_imm_s(index, SLOT);
        let from = builder.ins().iadd(list, along);
        let slot = builder
            .ins()
            .load(types::I64, TRUSTED, from, LIST_ELEMENTS as i32);
        let held = out_of_slot(builder, slot, machine_type(element)?);
        let held = self.turned(builder, element, held, way)?;
        let slot = into_slot(builder, held);
        let into = builder.ins().iadd(turned, along);
        builder
            .ins()
            .store(TRUSTED, slot, into, LIST_ELEMENTS as i32);
        let next = builder.ins().iadd_imm_s(index, 1);
        builder.ins().jump(head, &[next.into()]);

        builder.switch_to_block(done);
        Ok(turned)
    }
}

/// Which way a value is turned.
#[derive(Clone, Copy)]
enum Way {
    ToHost,
    FromHost,
}
