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

use super::{Codecs, Runtime};
use crate::literals::Literals;
use crate::transport::{AlternativesForm, Case, CodecShape, Declaration, Field, Prim};
use crate::{
    Construction, Constructors, Declared, Emitting, Lowered, POINTER, TRUSTED, construction,
    decide, into_slot, lay_out, machine_type, not_lowered, out_slot,
};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, types};
use cranelift::frontend::{FunctionBuilder, Variable};
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    ANSWERED, HELD, LIST_ELEMENTS, LIST_LENGTH, NOTHING, SLOT, room_for_held, room_for_list,
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

    fn literal(&mut self, text: &str) -> Lowered<ir::Value> {
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
        let key = self.literal(&field.name)?;
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
            CodecShape::Named { declared } => {
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
            CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => Err(not_lowered(format!(
                "{} read at a boundary",
                shape.ty().spelt()
            ))),
        }
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
        self.broken(declaration, clause, path)?;

        self.builder.seal_block(held);
        self.builder.switch_to_block(held);
        Ok(self.builder.block_params(held)[0])
    }

    /// Records that the clause at `clause` among `declaration`'s did not hold of the value at
    /// `path`, naming the clause where its author did, and answers nothing.
    fn broken(
        &mut self,
        declaration: &Declaration,
        clause: ir::Value,
        path: ir::Value,
    ) -> Lowered<()> {
        let module = self.literal(declaration.module())?;
        let name = self.literal(declaration.name())?;
        let clauses = declaration
            .clauses()
            .expect("a value is built by a call here only of a type whose clauses this build runs");
        for (at, stated) in clauses.iter().enumerate() {
            let Some(called) = &stated.name else {
                continue;
            };
            let this = self.builder.create_block();
            let next = self.builder.create_block();
            let is = self.builder.ins().icmp_imm_s(
                IntCC::Equal,
                clause,
                i64::try_from(at).expect("fewer clauses than an Int counts"),
            );
            self.builder.ins().brif(is, this, &[], next, &[]);
            self.builder.switch_to_block(this);
            let called = self.literal(called)?;
            self.call(
                Runtime::ReadInvariant,
                &[path, self.decoding, module, name, called],
            );
            self.builder.ins().jump(self.nothing, &[]);
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

    /// One of a set of alternatives, told apart the way the set's form says it is written.
    fn alternatives(
        &mut self,
        cases: &[Case],
        form: &AlternativesForm,
        node: ir::Value,
        path: ir::Value,
    ) -> Lowered<()> {
        let mut declared = Vec::with_capacity(cases.len());
        for case in cases {
            let Case::Declared { declared: key } = case else {
                return Err(not_lowered(format!(
                    "the case {}, which this backend does not read as one of a set of \
                     alternatives yet",
                    case.spelt()
                )));
            };
            declared.push((key.as_str(), self.declared.laid(key)));
        }
        match form {
            // The value is the case's name, and the case is a unit.
            AlternativesForm::Enumeration => {
                let named = self.asked(Runtime::ReadCase, &[node, path, self.decoding]);
                self.or_nothing(named);
                for (key, case) in declared {
                    self.when_named(node, case.name(), |reading| {
                        let value = reading.construct(key, &[], path)?;
                        reading.answer(value);
                        Ok(())
                    })?;
                }
                self.call(Runtime::ReadNotACase, &[node, path, self.decoding]);
                self.builder.ins().jump(self.nothing, &[]);
            }
            AlternativesForm::Discriminated { tag, contents } => {
                let object = self.asked(Runtime::ReadObject, &[node, path, self.decoding]);
                self.or_nothing(object);
                let tag = self.literal(tag)?;
                let at_tag = self.below(path, tag);
                let named = self.asked(Runtime::ReadTag, &[node, tag, at_tag, self.decoding]);
                let there = self.builder.ins().icmp_imm_s(IntCC::NotEqual, named, 0);
                self.or_nothing(there);
                for (key, case) in declared {
                    self.when_named(named, case.name(), |reading| {
                        reading.case(key, case, contents, node, path)
                    })?;
                }
                self.call(Runtime::ReadNotACase, &[named, at_tag, self.decoding]);
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
        let spelt = self.literal(name)?;
        let is = self.asked(Runtime::ReadIs, &[named, spelt]);
        let this = self.builder.create_block();
        let next = self.builder.create_block();
        self.builder.ins().brif(is, this, &[], next, &[]);
        self.builder.switch_to_block(this);
        then(self)?;
        self.builder.switch_to_block(next);
        Ok(())
    }

    /// A case of a discriminated set, read where the writer put it: its fields in the object that
    /// carries the tag, or wrapped under `contents` beside it.
    fn case(
        &mut self,
        key: &str,
        case: &Declaration,
        contents: &str,
        node: ir::Value,
        path: ir::Value,
    ) -> Lowered<()> {
        match case {
            Declaration::Sum { .. } => {
                unreachable!("`Declared::settled` refused a sum standing as the case {key}")
            }
            Declaration::Product { .. } | Declaration::Unit { .. } => {
                self.read_as(key, node, path);
            }
            Declaration::Newtype { .. } => {
                let contents = self.literal(contents)?;
                let at = self.below(path, contents);
                let member = self.asked(Runtime::ReadMember, &[node, contents]);
                let there = self.builder.create_block();
                let missing = self.builder.create_block();
                self.builder.ins().brif(member, there, &[], missing, &[]);
                self.builder.switch_to_block(missing);
                self.call(Runtime::ReadMissing, &[at, self.decoding]);
                self.builder.ins().jump(self.nothing, &[]);
                self.builder.switch_to_block(there);
                self.read_as(key, member, at);
            }
        }
        Ok(())
    }
}
