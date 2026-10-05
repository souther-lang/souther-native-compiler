//! A value out of a document: generated code that walks a declaration's external form over the
//! places of a document and builds a value of it, or says what was wrong.
//!
//! Each arm is the writer's ([`write`](super::write)) walked backwards. An object is read member by
//! member where the writer put members; a field the writer leaves out when it is absent is read as
//! absent when it is not there, and `null` is absence where the writer writes `null`; a case is told
//! apart by the key and the name the writer puts it under.
//!
//! What is wrong is recorded where it is found and reading goes on, so every field of a value is
//! read whatever the one before it was, and a document with several mistakes answers all of them.
//! A place that is not the shape it was declared as — not an object where an object goes, not text
//! where a case's name goes — is one issue there, and nothing below it is read: there is nothing
//! below it to read the declaration's shape into. A value whose every field was read is handed to
//! its type's construction, and one that is not has no value to hand it, so a clause runs only over
//! what was written whole.
//!
//! A reader answers the value, or nothing ([`NOTHING`]) where there is none, which is never a value
//! of a declared type: those are all somewhere. It answers a status as well, because a clause is
//! Souther and may end without a value, dividing by nought or leaving an `Int`'s range. That is not
//! the document's mistake and is not recorded as one: it goes back as it came, the way it does from
//! any other call.

use super::write::Writing;
use super::{Codecs, Runtime};
use crate::literals::Literals;
use crate::patterns::Patterns;
use crate::transport::{
    AlternativesForm, BoundaryConstraint, Case, CodecShape, Declaration, Field, LeafScalar, Prim,
};
use crate::{
    CaseBody, Construction, Constructors, Declared, Emitting, Lowered, POINTER, TRUSTED,
    carry_into, construction, decide, into_slot, lay_out, machine_type, not_lowered, out_slot,
    room_to_carry,
};
use cranelift::codegen::ir::TrapCode;
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, types};
use cranelift::frontend::{FunctionBuilder, Variable};
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    ANSWERED, HELD, LIST_ELEMENTS, LIST_LENGTH, NOTHING, SLOT, case_names_written, room_for_held,
    room_for_list,
};

/// Defines the reader of `key`.
pub(super) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    id: FuncId,
    signature: ir::Signature,
    key: &str,
) -> Lowered<()> {
    let declared = emitting.declared;
    let literals = emitting.literals;
    let constructors = emitting.constructors;
    let allocate = emitting.allocate;
    let value_ops = emitting.value_ops;
    let patterns = emitting.patterns;
    emitting.function(id, signature, |builder, module, given| {
        let [node, path, decoding, out] = given else {
            unreachable!("a reader takes a node, a path, a reading and room for the value")
        };
        let refused = builder.declare_var(types::I8);
        let nought = builder.ins().iconst(types::I8, 0);
        builder.def_var(refused, nought);
        let nothing = builder.create_block();
        let abort = builder.create_block();
        builder.append_block_param(abort, types::I32);
        let mut reading = Reading {
            builder,
            module,
            declared,
            literals,
            constructors,
            allocate,
            value_ops,
            patterns,
            codecs,
            decoding: *decoding,
            out: *out,
            refused,
            nothing,
            abort,
        };
        reading.declaration(key, *node, *path)?;

        builder.switch_to_block(nothing);
        let none = builder.ins().iconst(POINTER, NOTHING);
        builder.ins().store(TRUSTED, none, *out, 0);
        let answered = builder.ins().iconst(types::I32, i64::from(ANSWERED));
        builder.ins().return_(&[answered]);

        builder.switch_to_block(abort);
        let status = builder.block_params(abort)[0];
        builder.ins().return_(&[status]);
        Ok(())
    })
}

/// The reader being emitted, and what it reaches.
struct Reading<'w, 'f> {
    builder: &'w mut FunctionBuilder<'f>,
    module: &'w mut ObjectModule,
    declared: &'w Declared<'w>,
    literals: &'w Literals,
    constructors: &'w Constructors,
    allocate: FuncId,
    /// The hasher and the equality of what a set or a map read here is kept over.
    value_ops: &'w crate::hashing::ValueOps,
    /// The patterns the object holds, for a value held to a pattern.
    patterns: &'w Patterns,
    codecs: &'w mut Codecs,
    /// The reading every issue is recorded in.
    decoding: ir::Value,
    /// Where the value goes.
    out: ir::Value,
    /// Whether anything below the value being read was not what it was declared as, which is
    /// whether there is a value to build.
    refused: Variable,
    /// Where a reader goes to answer that there is no value.
    nothing: ir::Block,
    /// Where a status that is not `ANSWERED` goes back from.
    abort: ir::Block,
}

impl Reading<'_, '_> {
    fn call(&mut self, called: Runtime, arguments: &[ir::Value]) -> Option<ir::Value> {
        let reached = self.codecs.runtime(self.module, called);
        let reaching = self.module.declare_func_in_func(reached, self.builder.func);
        let call = self.builder.ins().call(reaching, arguments);
        self.builder.inst_results(call).first().copied()
    }

    fn asked(&mut self, called: Runtime, arguments: &[ir::Value]) -> ir::Value {
        self.call(called, arguments)
            .expect("a question of the runtime answers")
    }

    fn literal(&mut self, text: &str) -> ir::Value {
        self.literals.address(self.builder, self.module, text)
    }

    /// The place `step` below `path`.
    fn below(&mut self, path: ir::Value, step: ir::Value) -> ir::Value {
        self.asked(Runtime::PathBelow, &[path, step])
    }

    /// The value from here on is refused where `failed` is not nought.
    fn refuse_where(&mut self, failed: ir::Value) {
        let before = self.builder.use_var(self.refused);
        let after = self.builder.ins().bor(before, failed);
        self.builder.def_var(self.refused, after);
    }

    fn refuse(&mut self) {
        let one = self.builder.ins().iconst(types::I8, 1);
        self.builder.def_var(self.refused, one);
    }

    /// Goes on in a block of its own where `answered` is not nought, and to [`Self::nothing`] where
    /// it is.
    fn or_nothing(&mut self, answered: ir::Value) {
        let go_on = self.builder.create_block();
        self.builder
            .ins()
            .brif(answered, go_on, &[], self.nothing, &[]);
        self.builder.switch_to_block(go_on);
    }

    /// Goes on where everything read so far was what it was declared as.
    fn whole_or_nothing(&mut self) {
        let refused = self.builder.use_var(self.refused);
        let go_on = self.builder.create_block();
        self.builder
            .ins()
            .brif(refused, self.nothing, &[], go_on, &[]);
        self.builder.switch_to_block(go_on);
    }

    /// Forwards a status that is not `ANSWERED` exactly as it came.
    fn forward(&mut self, status: ir::Value) {
        let go_on = self.builder.create_block();
        let answered = self
            .builder
            .ins()
            .icmp_imm_s(IntCC::Equal, status, i64::from(ANSWERED));
        self.builder
            .ins()
            .brif(answered, go_on, &[], self.abort, &[status.into()]);
        self.builder.switch_to_block(go_on);
    }

    fn answer(&mut self, value: ir::Value) {
        self.builder.ins().store(TRUSTED, value, self.out, 0);
        let answered = self.builder.ins().iconst(types::I32, i64::from(ANSWERED));
        self.builder.ins().return_(&[answered]);
    }

    /// A reader called with this function's own room for the value and its status answered as
    /// this function's: the value at `node` is read wholly by that reader.
    fn read_as(&mut self, key: &str, node: ir::Value, path: ir::Value) {
        let reader = self.codecs.reader(self.module, self.declared, key);
        let reaching = self.module.declare_func_in_func(reader, self.builder.func);
        let call = self
            .builder
            .ins()
            .call(reaching, &[node, path, self.decoding, self.out]);
        let status = self.builder.inst_results(call)[0];
        self.builder.ins().return_(&[status]);
    }

    /// What a value of a declaration is read as on its own, wherever it stands.
    fn declaration(&mut self, key: &str, node: ir::Value, path: ir::Value) -> Lowered<()> {
        match self.declared.laid(key) {
            Declaration::Product { fields, .. } => {
                let object = self.asked(Runtime::ReadObject, &[node, path, self.decoding]);
                self.or_nothing(object);
                let mut values = Vec::with_capacity(fields.len());
                for field in fields {
                    values.push(self.field(node, path, field)?);
                }
                self.whole_or_nothing();
                let value = self.construct(key, &values, path)?;
                self.answer(value);
            }
            // Written as what it holds, at the place it stands, so what it holds is read there and
            // a clause it breaks is reported there too.
            Declaration::Newtype { field, .. } => {
                let held = self.value(node, path, &field.codec)?;
                self.whole_or_nothing();
                let value = self.construct(key, &[held], path)?;
                self.answer(value);
            }
            Declaration::Unit { .. } => {
                let object = self.asked(Runtime::ReadObject, &[node, path, self.decoding]);
                self.or_nothing(object);
                let value = self.construct(key, &[], path)?;
                self.answer(value);
            }
            Declaration::Sum { cases, form, .. } => self.alternatives(cases, form, node, path)?,
        }
        Ok(())
    }

    /// A field of an object, read from its member, or absent where the member is not there and the
    /// field may be.
    fn field(&mut self, node: ir::Value, path: ir::Value, field: &Field) -> Lowered<ir::Value> {
        let key = self.literal(&field.name);
        let at = self.below(path, key);
        let member = self.asked(Runtime::ReadMember, &[node, key]);
        let ty = machine_type(&field.codec.ty())?;

        let absent = self.builder.create_block();
        let present = self.builder.create_block();
        let read = self.builder.create_block();
        self.builder.append_block_param(read, ty);
        let missing = self.builder.ins().icmp_imm_s(IntCC::Equal, member, 0);
        self.builder.ins().brif(missing, absent, &[], present, &[]);

        self.builder.switch_to_block(absent);
        // What absence is written as where there is a key to leave out.
        if !matches!(field.codec, CodecShape::OptionOf { .. }) {
            self.call(Runtime::ReadMissing, &[at, self.decoding]);
            self.refuse();
        }
        let none = self.builder.ins().iconst(ty, NOTHING);
        self.builder.ins().jump(read, &[none.into()]);

        self.builder.switch_to_block(present);
        let value = self.value(member, at, &field.codec)?;
        self.builder.ins().jump(read, &[value.into()]);

        self.builder.switch_to_block(read);
        Ok(self.builder.block_params(read)[0])
    }

    /// A value standing at `node`, as the generated code holds one of `shape`: where there is no
    /// key, absence is `null`.
    fn value(
        &mut self,
        node: ir::Value,
        path: ir::Value,
        shape: &CodecShape,
    ) -> Lowered<ir::Value> {
        match shape {
            CodecShape::Scalar { scalar } => {
                let (reads, ty) = match scalar.prim() {
                    Prim::Int => (Runtime::ReadInt, types::I64),
                    Prim::Bool => (Runtime::ReadBool, types::I8),
                    Prim::String => (Runtime::ReadString, POINTER),
                    // At the scale the number was spelt at, which the runtime reads off it.
                    Prim::Decimal => (Runtime::ReadDecimal, POINTER),
                    // Text that names one, by the grammar of the type: what a boundary writes
                    // and, for an `Instant`, an offset spelling of the same moment.
                    Prim::Date => (Runtime::ReadDate, POINTER),
                    Prim::Time => (Runtime::ReadTime, POINTER),
                    Prim::DateTime => (Runtime::ReadDateTime, POINTER),
                    Prim::Instant => (Runtime::ReadInstant, POINTER),
                    other => {
                        return Err(not_lowered(format!(
                            "a {} read at a boundary",
                            other.spelt()
                        )));
                    }
                };
                let room = out_slot(self.builder);
                let read = self.asked(reads, &[node, path, self.decoding, room]);
                let failed = self.builder.ins().icmp_imm_s(IntCC::Equal, read, 0);
                self.refuse_where(failed);
                Ok(self.builder.ins().load(ty, TRUSTED, room, 0))
            }
            CodecShape::Named { named } => {
                let Some(declared) = named.declared() else {
                    return crate::named_as_a_type(&shape.ty());
                };
                let reader = self.codecs.reader(self.module, self.declared, declared);
                let reaching = self.module.declare_func_in_func(reader, self.builder.func);
                let room = out_slot(self.builder);
                let call = self
                    .builder
                    .ins()
                    .call(reaching, &[node, path, self.decoding, room]);
                let status = self.builder.inst_results(call)[0];
                self.forward(status);
                let value = self.builder.ins().load(POINTER, TRUSTED, room, 0);
                let none = self.builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING);
                self.refuse_where(none);
                Ok(value)
            }
            CodecShape::OptionOf { present } => {
                let absent = self.builder.create_block();
                let there = self.builder.create_block();
                let read = self.builder.create_block();
                self.builder.append_block_param(read, POINTER);
                let null = self.asked(Runtime::ReadNull, &[node]);
                self.builder.ins().brif(null, absent, &[], there, &[]);

                self.builder.switch_to_block(absent);
                let none = self.builder.ins().iconst(POINTER, NOTHING);
                self.builder.ins().jump(read, &[none.into()]);

                self.builder.switch_to_block(there);
                let inner = self.value(node, path, present.shape())?;
                let holding = self.held(inner);
                self.builder.ins().jump(read, &[holding.into()]);

                self.builder.switch_to_block(read);
                Ok(self.builder.block_params(read)[0])
            }
            CodecShape::ListOf { element } => self.list(node, path, element),
            CodecShape::SetOf { element } => self.set(node, path, element),
            CodecShape::MapOf { key, value } => self.map(node, path, &key.shape(), value),
        }
    }

    /// What `read` reads, and whether anything below it was not what it was declared as, apart
    /// from what was refused before it: a collection is built only of elements that were each read
    /// whole, whatever else of the value was not.
    fn own(
        &mut self,
        read: impl FnOnce(&mut Self) -> Lowered<ir::Value>,
    ) -> Lowered<(ir::Value, ir::Value)> {
        let before = self.builder.use_var(self.refused);
        let nought = self.builder.ins().iconst(types::I8, 0);
        self.builder.def_var(self.refused, nought);
        let value = read(self)?;
        let own = self.builder.use_var(self.refused);
        let after = self.builder.ins().bor(before, own);
        self.builder.def_var(self.refused, after);
        Ok((value, own))
    }

    /// The runtime's collection `build` makes of what was read, where every part of it was read
    /// whole, and nothing where one was not.
    fn built_where(
        &mut self,
        own: ir::Value,
        build: impl FnOnce(&mut Self) -> Lowered<ir::Value>,
    ) -> Lowered<ir::Value> {
        let whole = self.builder.create_block();
        let read = self.builder.create_block();
        self.builder.append_block_param(read, POINTER);
        let none = self.builder.ins().iconst(POINTER, NOTHING);
        self.builder
            .ins()
            .brif(own, read, &[none.into()], whole, &[]);

        self.builder.switch_to_block(whole);
        let built = build(self)?;
        self.builder.ins().jump(read, &[built.into()]);

        self.builder.switch_to_block(read);
        Ok(self.builder.block_params(read)[0])
    }

    /// A set, from an array read as a list of its elements: the array's order says nothing and two
    /// equal elements are one member (spec §collections), as `Set.fromList` makes of a list.
    fn set(
        &mut self,
        node: ir::Value,
        path: ir::Value,
        element: &CodecShape,
    ) -> Lowered<ir::Value> {
        let (list, own) = self.own(|reading| reading.list(node, path, element))?;
        let element = element.ty();
        self.built_where(own, |reading| {
            let [hasher, equality] =
                reading
                    .value_ops
                    .both(reading.builder, reading.module, &element);
            Ok(reading.asked(Runtime::SetFromList, &[list, hasher, equality]))
        })
    }

    /// A map, from an object, as the JVM's reader reads one: every member's value first, each at
    /// its key's place, and then, where every value was read whole, every key as the key's own type
    /// is read anywhere else, its invariant and all. A key that is one the map already holds once
    /// both are read is two spellings of one key, and is recorded where it stands
    /// (`duplicate_key`) rather than have one value lost to the other with nothing said. A place
    /// that is not an object is one issue there and no map.
    fn map(
        &mut self,
        node: ir::Value,
        path: ir::Value,
        key: &CodecShape,
        value: &CodecShape,
    ) -> Lowered<ir::Value> {
        let is_object = self.asked(Runtime::ReadObject, &[node, path, self.decoding]);
        let object = self.builder.create_block();
        let not_object = self.builder.create_block();
        let read = self.builder.create_block();
        self.builder.append_block_param(read, POINTER);
        self.builder
            .ins()
            .brif(is_object, object, &[], not_object, &[]);

        self.builder.switch_to_block(not_object);
        self.refuse();
        let none = self.builder.ins().iconst(POINTER, NOTHING);
        self.builder.ins().jump(read, &[none.into()]);

        self.builder.switch_to_block(object);
        let count = self.asked(Runtime::ReadMembers, &[node]);
        // The values, kept in a list in the order their members were written.
        let (values, values_own) = self.own(|reading| {
            let values = reading.room_for_list(count);
            reading.each_member(count, |reading, index| {
                let member = reading.asked(Runtime::ReadMemberValue, &[node, index]);
                let at = reading.asked(Runtime::PathBelowMember, &[path, node, index]);
                let held = reading.value(member, at, value)?;
                reading.put_element(values, index, held);
                Ok(())
            })?;
            Ok(values)
        })?;
        let key_ty = key.ty();
        let map = self.built_where(values_own, |reading| {
            let (map, keys_own) = reading.own(|reading| {
                let empty = reading.asked(Runtime::MapEmpty, &[]);
                let map = reading.builder.declare_var(POINTER);
                reading.builder.def_var(map, empty);
                let [hasher, equality] =
                    reading
                        .value_ops
                        .both(reading.builder, reading.module, &key_ty);
                reading.each_member(count, |reading, index| {
                    let written = reading.asked(Runtime::ReadMemberKey, &[node, index]);
                    let at = reading.asked(Runtime::PathBelowMember, &[path, node, index]);
                    let (read_key, key_own) =
                        reading.own(|reading| reading.value(written, at, key))?;
                    let whole = reading.builder.create_block();
                    let next = reading.builder.create_block();
                    reading.builder.ins().brif(key_own, next, &[], whole, &[]);

                    reading.builder.switch_to_block(whole);
                    let slot = into_slot(reading.builder, read_key);
                    let so_far = reading.builder.use_var(map);
                    let there =
                        reading.asked(Runtime::MapContainsKey, &[so_far, slot, hasher, equality]);
                    let fresh = reading.builder.create_block();
                    let twice = reading.builder.create_block();
                    reading.builder.ins().brif(there, twice, &[], fresh, &[]);

                    reading.builder.switch_to_block(twice);
                    reading.call(Runtime::ReadDuplicateKey, &[at, reading.decoding]);
                    reading.refuse();
                    reading.builder.ins().jump(next, &[]);

                    // One key more than the object has members, which no map is short of room
                    // for: what the runtime answers about whether it wrote it is not read.
                    reading.builder.switch_to_block(fresh);
                    let along = reading.builder.ins().imul_imm_s(index, SLOT);
                    let into = reading.builder.ins().iadd(values, along);
                    let held =
                        reading
                            .builder
                            .ins()
                            .load(types::I64, TRUSTED, into, LIST_ELEMENTS as i32);
                    let room = out_slot(reading.builder);
                    reading.call(
                        Runtime::MapInsert,
                        &[so_far, slot, held, hasher, equality, room],
                    );
                    let grown = reading.builder.ins().load(POINTER, TRUSTED, room, 0);
                    reading.builder.def_var(map, grown);
                    reading.builder.ins().jump(next, &[]);

                    reading.builder.switch_to_block(next);
                    Ok(())
                })?;
                Ok(reading.builder.use_var(map))
            })?;
            reading.built_where(keys_own, |_| Ok(map))
        })?;
        self.builder.ins().jump(read, &[map.into()]);

        self.builder.switch_to_block(read);
        Ok(self.builder.block_params(read)[0])
    }

    /// Room for a list of `count` elements, its length written and its elements left to the caller.
    fn room_for_list(&mut self, count: ir::Value) -> ir::Value {
        let along = self.builder.ins().imul_imm_s(count, SLOT);
        let size = self.builder.ins().iadd_imm_s(along, room_for_list(0));
        let taking = self
            .module
            .declare_func_in_func(self.allocate, self.builder.func);
        let taken = self.builder.ins().call(taking, &[size]);
        let list = self.builder.inst_results(taken)[0];
        self.builder
            .ins()
            .store(TRUSTED, count, list, LIST_LENGTH as i32);
        list
    }

    /// `value` into the slot of `list` at `index`.
    fn put_element(&mut self, list: ir::Value, index: ir::Value, value: ir::Value) {
        let slot = into_slot(self.builder, value);
        let along = self.builder.ins().imul_imm_s(index, SLOT);
        let into = self.builder.ins().iadd(list, along);
        self.builder
            .ins()
            .store(TRUSTED, slot, into, LIST_ELEMENTS as i32);
    }

    /// `each` for every index below `count`, in order.
    fn each_member(
        &mut self,
        count: ir::Value,
        mut each: impl FnMut(&mut Self, ir::Value) -> Lowered<()>,
    ) -> Lowered<()> {
        let head = self.builder.create_block();
        self.builder.append_block_param(head, types::I64);
        let step = self.builder.create_block();
        let walked = self.builder.create_block();
        let start = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().jump(head, &[start.into()]);

        self.builder.switch_to_block(head);
        let index = self.builder.block_params(head)[0];
        let inside = self.builder.ins().icmp(IntCC::SignedLessThan, index, count);
        self.builder.ins().brif(inside, step, &[], walked, &[]);

        self.builder.switch_to_block(step);
        each(self, index)?;
        let next = self.builder.ins().iadd_imm_s(index, 1);
        self.builder.ins().jump(head, &[next.into()]);

        self.builder.switch_to_block(walked);
        Ok(())
    }

    /// A list, from an array: every element read at its own index below `path`, whatever the one
    /// before it was, so an array with several wrong elements answers each of them. A place that is
    /// not an array is one issue there and no list.
    fn list(
        &mut self,
        node: ir::Value,
        path: ir::Value,
        element: &CodecShape,
    ) -> Lowered<ir::Value> {
        let is_array = self.asked(Runtime::ReadArray, &[node, path, self.decoding]);
        let array = self.builder.create_block();
        let not_array = self.builder.create_block();
        let read = self.builder.create_block();
        self.builder.append_block_param(read, POINTER);
        self.builder
            .ins()
            .brif(is_array, array, &[], not_array, &[]);

        self.builder.switch_to_block(not_array);
        self.refuse();
        let none = self.builder.ins().iconst(POINTER, NOTHING);
        self.builder.ins().jump(read, &[none.into()]);

        self.builder.switch_to_block(array);
        let length = self.asked(Runtime::ReadArrayLength, &[node]);
        let along = self.builder.ins().imul_imm_s(length, SLOT);
        let size = self.builder.ins().iadd_imm_s(along, room_for_list(0));
        let taking = self
            .module
            .declare_func_in_func(self.allocate, self.builder.func);
        let taken = self.builder.ins().call(taking, &[size]);
        let list = self.builder.inst_results(taken)[0];
        self.builder
            .ins()
            .store(TRUSTED, length, list, LIST_LENGTH as i32);

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
        let item = self.asked(Runtime::ReadElement, &[node, index]);
        let at = self.asked(Runtime::PathAt, &[path, index]);
        let value = self.value(item, at, element)?;
        let slot = into_slot(self.builder, value);
        let along = self.builder.ins().imul_imm_s(index, SLOT);
        let into = self.builder.ins().iadd(list, along);
        self.builder
            .ins()
            .store(TRUSTED, slot, into, LIST_ELEMENTS as i32);
        let next = self.builder.ins().iadd_imm_s(index, 1);
        self.builder.ins().jump(head, &[next.into()]);

        self.builder.switch_to_block(walked);
        self.builder.ins().jump(read, &[list.into()]);

        self.builder.switch_to_block(read);
        Ok(self.builder.block_params(read)[0])
    }

    /// An optional holding `value`, as the generated code holds one.
    fn held(&mut self, value: ir::Value) -> ir::Value {
        let taking = self
            .module
            .declare_func_in_func(self.allocate, self.builder.func);
        let size = self.builder.ins().iconst(types::I64, room_for_held());
        let taken = self.builder.ins().call(taking, &[size]);
        let holding = self.builder.inst_results(taken)[0];
        let slot = into_slot(self.builder, value);
        self.builder
            .ins()
            .store(TRUSTED, slot, holding, HELD as i32);
        holding
    }

    /// A value of `key` built from `fields`, which were all read whole, the way any construction of
    /// it is made: laid out where there is no clause to run, and otherwise by the one function that
    /// runs them. A clause that does not hold is recorded at `path` and leaves no value.
    fn construct(
        &mut self,
        key: &str,
        fields: &[ir::Value],
        path: ir::Value,
    ) -> Lowered<ir::Value> {
        let declaration = self.declared.laid(key);
        if construction(declaration) == Construction::Laid {
            return lay_out(
                self.builder,
                self.module,
                self.declared,
                self.allocate,
                declaration,
                fields,
            );
        }
        let checked = self.constructors.checked(key)?;
        let broken = self.builder.create_block();
        self.builder.append_block_param(broken, types::I64);
        let value = decide(
            self.builder,
            self.module,
            checked,
            fields,
            self.abort,
            broken,
        );
        // Where every clause held is left for the rest of the reading, which goes on past what a
        // clause that did not hold records.
        let held = self.builder.create_block();
        self.builder.append_block_param(held, POINTER);
        self.builder.ins().jump(held, &[value.into()]);

        self.builder.seal_block(broken);
        self.builder.switch_to_block(broken);
        let clause = self.builder.block_params(broken)[0];
        self.broken(declaration, clause, fields, path)?;

        self.builder.seal_block(held);
        self.builder.switch_to_block(held);
        Ok(self.builder.block_params(held)[0])
    }

    /// Records that the clause at `clause` among `declaration`'s did not hold of the value built
    /// of `fields` at `path`, and answers nothing.
    ///
    /// A newtype crosses as its field's value, so a clause of one is reported as what the checker
    /// found it to be as standard constraints on that value ([`Invariant::projection`](crate::transport::Invariant::projection)): each of
    /// them asked in the order it is written, the first the value breaks recorded as Raoh's own,
    /// and the clause's own failure only where no constraint is broken and the constraints are not
    /// the whole of it. Which clause did not hold is the construction's answer, which decides the
    /// clauses in the order a decoder chains them, so every clause before it held and so did every
    /// constraint of theirs. A product crosses as an object and its clauses run whole, as the rules
    /// they are, whatever they are as constraints: its failure names its type and the clause.
    fn broken(
        &mut self,
        declaration: &Declaration,
        clause: ir::Value,
        fields: &[ir::Value],
        path: ir::Value,
    ) -> Lowered<()> {
        let module = self.literal(declaration.module());
        let name = self.literal(declaration.name());
        let clauses = declaration
            .clauses()
            .expect("a value is built by a call here only of a type whose clauses this build runs");
        let held = match declaration {
            Declaration::Newtype { field, .. } => Some((&field.codec, fields[0])),
            _ => None,
        };
        for (at, stated) in clauses.iter().enumerate() {
            let constraints = match held {
                Some(_) => stated.projection.constraints(),
                None => &[],
            };
            if stated.name.is_none() && constraints.is_empty() {
                continue;
            }
            let this = self.builder.create_block();
            let next = self.builder.create_block();
            let is = self.builder.ins().icmp_imm_s(
                IntCC::Equal,
                clause,
                i64::try_from(at).expect("fewer clauses than an Int counts"),
            );
            self.builder.ins().brif(is, this, &[], next, &[]);
            self.builder.switch_to_block(this);
            if let Some((shape, value)) = held {
                for constraint in constraints {
                    let met = self.meets(constraint, shape, value, path)?;
                    let go_on = self.builder.create_block();
                    self.builder.ins().brif(met, go_on, &[], self.nothing, &[]);
                    self.builder.switch_to_block(go_on);
                }
            }
            if held.is_some() && stated.projection.complete() {
                // The constraints are the whole of the clause, the checker's claim that a value
                // meeting them meets it (`ConstraintProjection`), so the clause cannot have failed
                // with every one of them met.
                self.builder.ins().trap(
                    TrapCode::user(crate::A_WHOLE_CLAUSE_MET).expect("a trap code of its own"),
                );
            } else {
                let called = match &stated.name {
                    Some(called) => self.literal(called),
                    None => self.builder.ins().iconst(POINTER, 0),
                };
                self.call(
                    Runtime::ReadInvariant,
                    &[path, self.decoding, module, name, called],
                );
                self.builder.ins().jump(self.nothing, &[]);
            }
            self.builder.switch_to_block(next);
        }
        let unnamed = self.builder.ins().iconst(POINTER, 0);
        self.call(
            Runtime::ReadInvariant,
            &[path, self.decoding, module, name, unnamed],
        );
        self.builder.ins().jump(self.nothing, &[]);
        Ok(())
    }

    /// Whether `value`, read as `shape`, meets `constraint`, the runtime having recorded what
    /// Raoh's own constraint reports where it does not. What each is called and what its failure
    /// says is the runtime's; which one a clause is, is the checker's.
    fn meets(
        &mut self,
        constraint: &BoundaryConstraint,
        shape: &CodecShape,
        value: ir::Value,
        path: ir::Value,
    ) -> Lowered<ir::Value> {
        let reading = self.decoding;
        let n = |reading: &mut Self, n: &i64| reading.builder.ins().iconst(types::I64, *n);
        Ok(match constraint {
            BoundaryConstraint::MinLength { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadMinLength, &[path, reading, value, bound])
            }
            BoundaryConstraint::MaxLength { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadMaxLength, &[path, reading, value, bound])
            }
            BoundaryConstraint::FixedLength { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadFixedLength, &[path, reading, value, bound])
            }
            BoundaryConstraint::Pattern { written, image } => {
                let pattern = self.patterns.address(self.builder, self.module, image);
                let written = self.literal(written);
                self.asked(
                    Runtime::ReadPattern,
                    &[path, reading, value, pattern, written],
                )
            }
            BoundaryConstraint::Min { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadIntMin, &[path, reading, value, bound])
            }
            BoundaryConstraint::Max { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadIntMax, &[path, reading, value, bound])
            }
            BoundaryConstraint::Positive => {
                self.asked(Runtime::ReadIntPositive, &[path, reading, value])
            }
            BoundaryConstraint::NonNegative => {
                self.asked(Runtime::ReadIntNonNegative, &[path, reading, value])
            }
            BoundaryConstraint::DecimalMin { n: bound } => {
                let bound = self.decimal(bound);
                self.asked(Runtime::ReadDecimalMin, &[path, reading, value, bound])
            }
            BoundaryConstraint::DecimalMax { n: bound } => {
                let bound = self.decimal(bound);
                self.asked(Runtime::ReadDecimalMax, &[path, reading, value, bound])
            }
            BoundaryConstraint::DecimalPositive => {
                self.asked(Runtime::ReadDecimalPositive, &[path, reading, value])
            }
            BoundaryConstraint::DecimalNonNegative => {
                self.asked(Runtime::ReadDecimalNonNegative, &[path, reading, value])
            }
            BoundaryConstraint::NonEmpty => {
                self.asked(Runtime::ReadListNonEmpty, &[path, reading, value])
            }
            BoundaryConstraint::MinSize { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadListMinSize, &[path, reading, value, bound])
            }
            BoundaryConstraint::MaxSize { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadListMaxSize, &[path, reading, value, bound])
            }
            BoundaryConstraint::FixedSize { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadListFixedSize, &[path, reading, value, bound])
            }
            BoundaryConstraint::Unique => self.unique(shape, value, path)?,
            BoundaryConstraint::MapNonEmpty => {
                self.asked(Runtime::ReadMapNonEmpty, &[path, reading, value])
            }
            BoundaryConstraint::MapMinSize { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadMapMinSize, &[path, reading, value, bound])
            }
            BoundaryConstraint::MapMaxSize { n: bound } => {
                let bound = n(self, bound);
                self.asked(Runtime::ReadMapMaxSize, &[path, reading, value, bound])
            }
        })
    }

    /// A `Decimal` bound, made by the runtime from its integer and its scale as a literal is.
    fn decimal(&mut self, bound: &crate::transport::DecimalBound) -> ir::Value {
        let digits = self.literal(&bound.unscaled);
        let scale = self
            .builder
            .ins()
            .iconst(types::I64, i64::from(bound.scale));
        self.asked(Runtime::DecimalLiteral, &[digits, scale])
    }

    /// Whether the list `value` of `shape` holds no element twice, compared as Souther compares.
    /// Where it holds some, they are written as a boundary writes the list's elements and handed
    /// to the runtime, which records them as Raoh's `duplicates`.
    fn unique(
        &mut self,
        shape: &CodecShape,
        value: ir::Value,
        path: ir::Value,
    ) -> Lowered<ir::Value> {
        let CodecShape::ListOf { element } = shape else {
            unreachable!("`Declared::of` held a list's constraint to be stated of a list")
        };
        let element_type = self.meta_type(element).ok_or_else(|| {
            not_lowered(format!(
                "a list of {} whose elements must be distinct: Raoh's `unique` takes only elements \
                 an issue can write, and this backend writes no other until Souther decides what \
                 such a clause is (souther-lang/souther#2149)",
                element.ty().spelt()
            ))
        })?;
        let [hasher, equality] = self
            .value_ops
            .both(self.builder, self.module, &element.ty());
        let repeated = self.asked(Runtime::ListDuplicates, &[value, hasher, equality]);
        let length = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, repeated, LIST_LENGTH as i32);
        let none = self.builder.ins().icmp_imm_s(IntCC::Equal, length, 0);
        let some = self.builder.create_block();
        let answered = self.builder.create_block();
        self.builder.append_block_param(answered, types::I8);
        let met = self.builder.ins().iconst(types::I8, 1);
        self.builder
            .ins()
            .brif(none, answered, &[met.into()], some, &[]);

        self.builder.switch_to_block(some);
        let written = Writing {
            builder: &mut *self.builder,
            module: &mut *self.module,
            declared: self.declared,
            literals: self.literals,
            codecs: &mut *self.codecs,
        }
        .shaped(shape, repeated)?;
        let element_type = self.literal(&element_type);
        self.call(
            Runtime::ReadDuplicates,
            &[path, self.decoding, written, element_type],
        );
        let broken = self.builder.ins().iconst(types::I8, 0);
        self.builder.ins().jump(answered, &[broken.into()]);

        self.builder.switch_to_block(answered);
        Ok(self.builder.block_params(answered)[0])
    }

    /// The type an element of `shape` is in Raoh's value model, as an issue's metadata names it
    /// (`META_TYPES`), read off the declaration and not off what the element is written as: a
    /// newtype is what it holds, as a boundary writes it, and a list is `list<T>` of its elements'.
    /// None where Raoh gives the element no message form, so no issue can write it: an optional, a
    /// product, a sum, a set or a map.
    fn meta_type(&self, shape: &CodecShape) -> Option<String> {
        match shape {
            CodecShape::Scalar { scalar } => Some(
                match scalar {
                    LeafScalar::Int => "int",
                    LeafScalar::Decimal => "decimal",
                    LeafScalar::String => "string",
                    LeafScalar::Bool => "bool",
                    LeafScalar::Date => "date",
                    LeafScalar::Time => "time",
                    LeafScalar::DateTime => "datetime",
                    LeafScalar::Instant => "instant",
                }
                .to_owned(),
            ),
            CodecShape::ListOf { element } => Some(format!("list<{}>", self.meta_type(element)?)),
            CodecShape::Named { named } => match self.declared.body_of(named) {
                CaseBody::Declared {
                    declaration: Declaration::Newtype { field, .. },
                    ..
                } => self.meta_type(&field.codec),
                _ => None,
            },
            CodecShape::SetOf { .. } | CodecShape::MapOf { .. } | CodecShape::OptionOf { .. } => {
                None
            }
        }
    }

    /// One of a set of alternatives, told apart the way the set's form says it is written.
    fn alternatives(
        &mut self,
        cases: &[Case],
        form: &AlternativesForm,
        node: ir::Value,
        path: ir::Value,
    ) -> Lowered<()> {
        let bodies: Vec<CaseBody> = cases
            .iter()
            .map(|case| self.declared.body_of(case))
            .collect();
        // What a refusal lists as allowed: every case's name, which is what each is read as.
        let names = case_names_written(bodies.iter().map(|body| body.name()));
        match form {
            // The value is the case's name, and the case is a unit.
            AlternativesForm::Enumeration => {
                let named = self.asked(Runtime::ReadCase, &[node, path, self.decoding]);
                self.or_nothing(named);
                for body in &bodies {
                    let CaseBody::Declared {
                        key,
                        declaration: Declaration::Unit { .. },
                    } = body
                    else {
                        unreachable!(
                            "`Declared::settled` refused {}, which is not a unit, in an \
                             enumeration",
                            body.name()
                        )
                    };
                    self.when_named(node, body.name(), |reading| {
                        let value = reading.construct(key, &[], path)?;
                        reading.answer(value);
                        Ok(())
                    })?;
                }
                let names = self.literal(&names);
                self.call(Runtime::ReadNotOneOf, &[node, path, self.decoding, names]);
                self.builder.ins().jump(self.nothing, &[]);
            }
            AlternativesForm::Discriminated { tag, contents } => {
                let object = self.asked(Runtime::ReadObject, &[node, path, self.decoding]);
                self.or_nothing(object);
                let tag = self.literal(tag);
                let at_tag = self.below(path, tag);
                let named = self.asked(Runtime::ReadTag, &[node, tag, at_tag, self.decoding]);
                let there = self.builder.ins().icmp_imm_s(IntCC::NotEqual, named, 0);
                self.or_nothing(there);
                for (case, body) in cases.iter().zip(&bodies) {
                    self.when_named(named, body.name(), |reading| {
                        reading.case(case, body, contents, node, path)
                    })?;
                }
                let names = self.literal(&names);
                self.call(
                    Runtime::ReadNoSuchTag,
                    &[named, at_tag, self.decoding, names],
                );
                self.builder.ins().jump(self.nothing, &[]);
            }
        }
        Ok(())
    }

    /// Emits `then` where the text at `named` is `name`, and goes on where it is not.
    fn when_named(
        &mut self,
        named: ir::Value,
        name: &str,
        then: impl FnOnce(&mut Self) -> Lowered<()>,
    ) -> Lowered<()> {
        let spelt = self.literal(name);
        let is = self.asked(Runtime::ReadIs, &[named, spelt]);
        let this = self.builder.create_block();
        let next = self.builder.create_block();
        self.builder.ins().brif(is, this, &[], next, &[]);
        self.builder.switch_to_block(this);
        then(self)?;
        self.builder.switch_to_block(next);
        Ok(())
    }

    /// A case of a discriminated set, read where the writer put it: a product's fields in the object
    /// that carries the tag, and what a newtype or a primitive holds under `contents` beside it. A
    /// case that holds nothing is the tag alone.
    fn case(
        &mut self,
        case: &Case,
        body: &CaseBody,
        contents: &str,
        node: ir::Value,
        path: ir::Value,
    ) -> Lowered<()> {
        match body {
            CaseBody::Declared {
                key,
                declaration: Declaration::Sum { .. },
            } => {
                unreachable!("`Declared::settled` refused a sum standing as the case {key}")
            }
            CaseBody::Declared {
                key,
                declaration: Declaration::Product { .. } | Declaration::Unit { .. },
            } => {
                self.read_as(key, node, path);
            }
            CaseBody::Declared {
                key,
                declaration: Declaration::Newtype { .. },
            } => {
                let (member, at) = self.contents(contents, node, path);
                self.read_as(key, member, at);
            }
            CaseBody::Primitive(prim) => {
                let (member, at) = self.contents(contents, node, path);
                let shape = CodecShape::Scalar {
                    scalar: LeafScalar::of(*prim).ok_or_else(|| {
                        not_lowered(format!("a {} read at a boundary", prim.spelt()))
                    })?,
                };
                let held = self.value(member, at, &shape)?;
                self.whole_or_nothing();
                let value = self.carried(case, Some(held))?;
                self.answer(value);
            }
            CaseBody::Empty(_) => {
                let value = self.carried(case, None)?;
                self.answer(value);
            }
        }
        Ok(())
    }

    /// What is under `contents` beside a tag, and the place it stands at, going to
    /// [`Self::nothing`] where the member is not there.
    fn contents(
        &mut self,
        contents: &str,
        node: ir::Value,
        path: ir::Value,
    ) -> (ir::Value, ir::Value) {
        let contents = self.literal(contents);
        let at = self.below(path, contents);
        let member = self.asked(Runtime::ReadMember, &[node, contents]);
        let there = self.builder.create_block();
        let missing = self.builder.create_block();
        self.builder.ins().brif(member, there, &[], missing, &[]);
        self.builder.switch_to_block(missing);
        self.call(Runtime::ReadMissing, &[at, self.decoding]);
        self.builder.ins().jump(self.nothing, &[]);
        self.builder.switch_to_block(there);
        (member, at)
    }

    /// A value of a case no declaration names, carried with its token.
    fn carried(&mut self, case: &Case, holds: Option<ir::Value>) -> Lowered<ir::Value> {
        let taking = self
            .module
            .declare_func_in_func(self.allocate, self.builder.func);
        let size = self
            .builder
            .ins()
            .iconst(types::I64, room_to_carry(holds.is_some()));
        let taken = self.builder.ins().call(taking, &[size]);
        let room = self.builder.inst_results(taken)[0];
        carry_into(self.builder, self.declared, self.module, room, case, holds)
    }
}
