//! A value as the runtime's external form: generated code that walks a value and builds the tree
//! the runtime writes out as JSON.
//!
//! A writer calls the writer of what a field holds, so writing a value takes a native frame for
//! every level a declared value is nested, and how deep a value it can write is bounded by the
//! stack. Nothing bounds how deep a value it is handed is: one may have been built by another
//! object, or by a behavior the host supplies. The runtime's own walk over the tree, and its drop,
//! take no frame per level.

use super::{Codecs, Runtime};
use crate::transport::{
    AlternativesForm, BoundaryOutput, Case, CodecShape, Declaration, Field, LeafScalar, Prim,
};
use crate::{
    Declared, Emitting, Literals, Lowered, NO_ARM, POINTER, TRUSTED, machine_type, not_lowered,
    out_of_slot, text_in_the_object,
};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, TrapCode, types};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{FuncId, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{HELD, NOTHING, WHICH, field_at};

/// Defines the writer of `key`.
pub(super) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    id: FuncId,
    signature: ir::Signature,
    key: &str,
) -> Lowered<()> {
    let declared = emitting.declared;
    let literals = emitting.literals;
    emitting.function(id, signature, |builder, module, given| {
        let mut writing = Writing {
            builder,
            module,
            declared,
            literals,
            codecs,
        };
        let form = writing.declaration(key, given[0])?;
        writing.builder.ins().return_(&[form]);
        Ok(())
    })
}

/// The function being emitted, and what it reaches while it builds a form.
pub(crate) struct Writing<'w, 'f> {
    pub builder: &'w mut FunctionBuilder<'f>,
    pub module: &'w mut ObjectModule,
    pub declared: &'w Declared<'w>,
    pub literals: &'w Literals,
    pub codecs: &'w mut Codecs,
}

impl Writing<'_, '_> {
    pub(crate) fn call(&mut self, called: Runtime, arguments: &[ir::Value]) -> ir::Value {
        let reached = self.codecs.runtime(self.module, called);
        self.call_function(reached, arguments)
    }

    fn call_function(&mut self, reached: FuncId, arguments: &[ir::Value]) -> ir::Value {
        let reaching = self.module.declare_func_in_func(reached, self.builder.func);
        let called = self.builder.ins().call(reaching, arguments);
        self.builder.inst_results(called)[0]
    }

    fn call_for_effect(&mut self, called: Runtime, arguments: &[ir::Value]) {
        let reached = self.codecs.runtime(self.module, called);
        let reaching = self.module.declare_func_in_func(reached, self.builder.func);
        self.builder.ins().call(reaching, arguments);
    }

    /// A string written into the object, a literal of the runtime's own layout: a key, or a
    /// case's name.
    fn literal(&mut self, text: &str) -> Lowered<ir::Value> {
        text_in_the_object(self.builder, self.module, self.literals, text)
    }

    fn object(&mut self) -> ir::Value {
        self.call(Runtime::ExternalObject, &[])
    }

    fn put(&mut self, object: ir::Value, key: &str, item: ir::Value) -> Lowered<()> {
        let key = self.literal(key)?;
        self.call_for_effect(Runtime::ExternalPut, &[object, key, item]);
        Ok(())
    }

    fn name(&mut self, name: &str) -> Lowered<ir::Value> {
        let spelt = self.literal(name)?;
        Ok(self.call(Runtime::ExternalString, &[spelt]))
    }

    /// What an answer leaves as.
    pub(crate) fn output(
        &mut self,
        output: &BoundaryOutput,
        answer: ir::Value,
    ) -> Lowered<ir::Value> {
        match output {
            BoundaryOutput::Scalar { scalar } => self.scalar(*scalar, answer),
            BoundaryOutput::Nominal { declared } => Ok(self.named(declared, answer)),
            BoundaryOutput::Cases { cases, form, .. } => self.alternatives(cases, form, answer),
            BoundaryOutput::ListOf { .. }
            | BoundaryOutput::SetOf { .. }
            | BoundaryOutput::MapOf { .. } => Err(not_lowered(format!(
                "an answer written as {}",
                output.ty().spelt()
            ))),
        }
    }

    fn scalar(&mut self, scalar: LeafScalar, value: ir::Value) -> Lowered<ir::Value> {
        match scalar.prim() {
            Prim::Int => Ok(self.call(Runtime::ExternalInt, &[value])),
            Prim::Bool => Ok(self.call(Runtime::ExternalBool, &[value])),
            Prim::String => Ok(self.call(Runtime::ExternalString, &[value])),
            other => Err(not_lowered(format!(
                "a {} written at a boundary",
                other.spelt()
            ))),
        }
    }

    /// A value of a declared type, written by that type's writer.
    pub(crate) fn named(&mut self, declared: &str, value: ir::Value) -> ir::Value {
        let writer = self.codecs.writer(self.module, declared);
        self.call_function(writer, &[value])
    }

    /// A value standing where it has no key of its own: an absent one is written `null`.
    fn value(&mut self, shape: &CodecShape, value: ir::Value) -> Lowered<ir::Value> {
        match shape {
            CodecShape::Scalar { scalar } => self.scalar(*scalar, value),
            CodecShape::Named { declared } => Ok(self.named(declared, value)),
            CodecShape::OptionOf { present } => {
                let absent = self.builder.create_block();
                let held = self.builder.create_block();
                let written = self.builder.create_block();
                self.builder.append_block_param(written, POINTER);
                let nothing = self.builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING);
                self.builder.ins().brif(nothing, absent, &[], held, &[]);

                self.builder.switch_to_block(absent);
                let null = self.call(Runtime::ExternalNull, &[]);
                self.builder.ins().jump(written, &[null.into()]);

                self.builder.switch_to_block(held);
                let inner = self.held(present.shape(), value)?;
                let form = self.value(present.shape(), inner)?;
                self.builder.ins().jump(written, &[form.into()]);

                self.builder.switch_to_block(written);
                Ok(self.builder.block_params(written)[0])
            }
            CodecShape::ListOf { .. } | CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => Err(
                not_lowered(format!("{} written at a boundary", shape.ty().spelt())),
            ),
        }
    }

    /// A field of an object, which has a second way of holding nothing: not being there.
    fn field(
        &mut self,
        object: ir::Value,
        name: &str,
        shape: &CodecShape,
        slot: ir::Value,
    ) -> Lowered<()> {
        let CodecShape::OptionOf { present } = shape else {
            let value = out_of_slot(self.builder, slot, machine_type(&shape.ty())?);
            let form = self.value(shape, value)?;
            return self.put(object, name, form);
        };
        let held = self.builder.create_block();
        let done = self.builder.create_block();
        let nothing = self.builder.ins().icmp_imm_s(IntCC::Equal, slot, NOTHING);
        self.builder.ins().brif(nothing, done, &[], held, &[]);

        self.builder.switch_to_block(held);
        let inner = self.held(present.shape(), slot)?;
        let form = self.value(present.shape(), inner)?;
        self.put(object, name, form)?;
        self.builder.ins().jump(done, &[]);

        self.builder.switch_to_block(done);
        Ok(())
    }

    /// What a present optional holds, read out of its slot the way any value is.
    fn held(&mut self, present: &CodecShape, holding: ir::Value) -> Lowered<ir::Value> {
        let slot = self
            .builder
            .ins()
            .load(types::I64, TRUSTED, holding, HELD as i32);
        Ok(out_of_slot(
            self.builder,
            slot,
            machine_type(&present.ty())?,
        ))
    }

    /// Every field of a value, put into an object this function made, in the order they are laid
    /// out.
    fn fields_into(
        &mut self,
        object: ir::Value,
        fields: &[Field],
        value: ir::Value,
    ) -> Lowered<()> {
        for (at, field) in fields.iter().enumerate() {
            let slot = self
                .builder
                .ins()
                .load(types::I64, TRUSTED, value, field_at(at) as i32);
            self.field(object, &field.name, &field.codec, slot)?;
        }
        Ok(())
    }

    /// What a value of a declaration is written as on its own, wherever it stands.
    fn declaration(&mut self, key: &str, value: ir::Value) -> Lowered<ir::Value> {
        match self.declared.laid(key) {
            Declaration::Product { fields, .. } => {
                let object = self.object();
                self.fields_into(object, fields, value)?;
                Ok(object)
            }
            Declaration::Newtype { field, .. } => {
                let slot = self
                    .builder
                    .ins()
                    .load(types::I64, TRUSTED, value, field_at(0) as i32);
                let inner = out_of_slot(self.builder, slot, machine_type(&field.codec.ty())?);
                self.value(&field.codec, inner)
            }
            Declaration::Unit { .. } => Ok(self.object()),
            Declaration::Sum { cases, form, .. } => self.alternatives(cases, form, value),
        }
    }

    /// One of a set of alternatives, told apart by the token at the front of the value and written
    /// in the form the set travels in.
    fn alternatives(
        &mut self,
        cases: &[Case],
        form: &AlternativesForm,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        let which = self
            .builder
            .ins()
            .load(POINTER, TRUSTED, value, WHICH as i32);
        let written = self.builder.create_block();
        self.builder.append_block_param(written, POINTER);

        for case in cases {
            let Case::Declared { declared: key } = case else {
                return Err(not_lowered(format!(
                    "the case {}, which carries no token to be told apart by",
                    case.spelt()
                )));
            };
            let token = self.declared.tag(self.module, key)?;
            let token = self.module.declare_data_in_func(token, self.builder.func);
            let expected = self.builder.ins().symbol_value(POINTER, token);
            let same = self.builder.ins().icmp(IntCC::Equal, which, expected);
            let this = self.builder.create_block();
            let next = self.builder.create_block();
            self.builder.ins().brif(same, this, &[], next, &[]);

            self.builder.switch_to_block(this);
            let shape = self.declared.laid(key);
            let form = self.case(key, shape, form, value)?;
            self.builder.ins().jump(written, &[form.into()]);
            self.builder.switch_to_block(next);
        }
        self.builder
            .ins()
            .trap(TrapCode::user(NO_ARM).expect("a trap code of its own"));

        self.builder.switch_to_block(written);
        Ok(self.builder.block_params(written)[0])
    }

    /// A case, written with what membership adds. Whether the case takes the tag into its own
    /// object or is wrapped beside it is read off its declaration's arm, never off what its own
    /// form turns out to be: a newtype over a product writes an object and is still wrapped.
    ///
    /// Every object a member is put into is one made here. A case's own fields are laid into the
    /// object that carries its tag, rather than the tag put into whatever the case's writer
    /// handed back, so what a `put` is given is never a form this function has to take on trust.
    ///
    /// [`read`](super::read) reads each arm here back, and the two are held arm for arm.
    fn case(
        &mut self,
        key: &str,
        shape: &Declaration,
        form: &AlternativesForm,
        value: ir::Value,
    ) -> Lowered<ir::Value> {
        match (form, shape) {
            (_, Declaration::Sum { .. }) => {
                unreachable!("`Declared::settled` refused a sum standing as a case of {key}")
            }
            (AlternativesForm::Enumeration, Declaration::Unit { .. }) => self.name(shape.name()),
            (
                AlternativesForm::Enumeration,
                Declaration::Product { .. } | Declaration::Newtype { .. },
            ) => unreachable!(
                "`Declared::settled` refused {key}, which has fields, in an enumeration"
            ),
            (AlternativesForm::Discriminated { tag, .. }, Declaration::Product { fields, .. }) => {
                let object = self.object();
                let name = self.name(shape.name())?;
                self.put(object, tag, name)?;
                self.fields_into(object, fields, value)?;
                Ok(object)
            }
            (AlternativesForm::Discriminated { tag, .. }, Declaration::Unit { .. }) => {
                let object = self.object();
                let name = self.name(shape.name())?;
                self.put(object, tag, name)?;
                Ok(object)
            }
            (AlternativesForm::Discriminated { tag, contents }, Declaration::Newtype { .. }) => {
                let object = self.object();
                let name = self.name(shape.name())?;
                self.put(object, tag, name)?;
                let inner = self.named(key, value);
                self.put(object, contents, inner)?;
                Ok(object)
            }
        }
    }
}
