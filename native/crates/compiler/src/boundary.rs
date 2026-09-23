//! What an answer is written as where it leaves the object: generated code that walks a value and
//! builds the runtime's external form out of it, from what the checker settled about every
//! position the value stands at.
//!
//! Nothing here decides a representation. Which scalar a field is, whether an absent one is left
//! out or written `null`, whether a set of alternatives travels as a bare name or a discriminated
//! object, and both keys of the second, all arrive on the transport; a declaration's arm says
//! whether a case's own form takes the tag or is wrapped beside it. What is this backend's own is
//! only where a value is in memory — which slot a field is in, how an absent one is held, which
//! token says what a value is — and reading that is the same reading the rest of the lowering
//! does. The walk is compiled per declaration, so nothing about a declaration is carried to run
//! time for the runtime to interpret.
//!
//! An encoder calls the encoder of what a field holds, so writing a value takes a native frame for
//! every level a declared value is nested, and how deep a value it can write is bounded by the
//! stack. Nothing bounds how deep a value it is handed is: one may have been built by another
//! object, or by a behavior the host supplies. The runtime's own walk over the tree, and its drop,
//! take no frame per level.

use super::{
    Declared, Literals, NO_ARM, POINTER, TRUSTED, call_reached, machine_type, not_lowered,
    out_of_slot, text_in_the_object,
};
use crate::transport::{
    AlternativesForm, BoundaryOutput, Case, CodecShape, Declaration, Field, LeafScalar, Prim, Ty,
};
use anyhow::{Result, bail};
use cranelift::codegen::Context;
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{
    self, AbiParam, Function, InstBuilder, TrapCode, UserFuncName, types,
};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift::module::{FuncId, Linkage, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    ANSWERED, EXTERNAL_BOOL, EXTERNAL_INT, EXTERNAL_JSON, EXTERNAL_NULL, EXTERNAL_OBJECT,
    EXTERNAL_PUT, EXTERNAL_STRING, HELD, NOTHING, WHICH, field_at,
};
use std::collections::BTreeMap;

/// An entry a host reaches for its answer as the language writes it: what it runs, what that
/// takes, and what the checker settled the answer leaves as.
pub(crate) struct Boundary<'a> {
    pub symbol: String,
    pub runs: FuncId,
    pub takes: Vec<Ty>,
    pub output: &'a BoundaryOutput,
}

/// Where generated code is emitted from, the same handful of things every definition is.
pub(crate) struct Emitting<'a> {
    pub module: &'a mut ObjectModule,
    pub context: &'a mut Context,
    pub shapes: &'a mut FunctionBuilderContext,
    pub frontend: TargetFrontendConfig,
    pub call_conv: CallConv,
    pub declared: &'a Declared<'a>,
    pub literals: &'a Literals,
}

/// Defines every entry in `boundaries`, and an encoder for each declaration one of them reaches.
pub(crate) fn define(emitting: Emitting, boundaries: &[Boundary]) -> Result<()> {
    if boundaries.is_empty() {
        return Ok(());
    }
    let Emitting {
        module,
        context,
        shapes,
        frontend,
        call_conv,
        declared,
        literals,
    } = emitting;
    let externals = Externals::declare(module, call_conv)?;
    let mut encoders = Encoders::new(call_conv);

    for boundary in boundaries {
        let mut signature = ir::Signature::new(call_conv);
        for taken in &boundary.takes {
            signature.params.push(AbiParam::new(machine_type(taken)?));
        }
        signature.params.push(AbiParam::new(POINTER));
        signature.returns.push(AbiParam::new(types::I32));
        let id = module.declare_function(&boundary.symbol, Linkage::Export, &signature)?;

        context.clear();
        context.func = Function::with_name_signature(UserFuncName::default(), signature);
        let mut builder = FunctionBuilder::new(&mut context.func, shapes);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let given = builder.block_params(entry).to_vec();
        let (arguments, out) = given.split_at(boundary.takes.len());

        // A status that is not `ANSWERED` goes back as it came, and nothing is written.
        let abort = builder.create_block();
        builder.append_block_param(abort, types::I32);
        let answers = machine_type(&boundary.output.ty())?;
        let answer = call_reached(
            &mut builder,
            module,
            abort,
            boundary.runs,
            answers,
            arguments,
        )?;

        let mut writing = Writing {
            builder: &mut builder,
            module,
            declared,
            literals,
            externals: &externals,
            encoders: &mut encoders,
        };
        let form = writing.output(boundary.output, answer)?;
        let json = writing.call(externals.json, &[form]);
        builder.ins().store(TRUSTED, json, out[0], 0);
        let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
        builder.ins().return_(&[ok]);

        builder.switch_to_block(abort);
        let status = builder.block_params(abort)[0];
        builder.ins().return_(&[status]);
        builder.seal_all_blocks();
        builder.finalize(frontend);
        module.define_function(id, context)?;
    }

    while let Some(key) = encoders.pending.pop() {
        let id = encoders.ids[&key];
        context.clear();
        context.func = Function::with_name_signature(UserFuncName::default(), encoders.signature());
        let mut builder = FunctionBuilder::new(&mut context.func, shapes);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let value = builder.block_params(entry)[0];

        let mut writing = Writing {
            builder: &mut builder,
            module,
            declared,
            literals,
            externals: &externals,
            encoders: &mut encoders,
        };
        let form = writing.declaration(&key, value)?;
        builder.ins().return_(&[form]);
        builder.seal_all_blocks();
        builder.finalize(frontend);
        module.define_function(id, context)?;
    }
    Ok(())
}

/// The runtime's constructors for the external form, and its writer. Their ownership is stated
/// beside the symbols in `souther-native-abi`: every constructor hands over a form, `put` takes
/// the item it is given, and `json` takes the root.
struct Externals {
    null: FuncId,
    truth: FuncId,
    int: FuncId,
    string: FuncId,
    object: FuncId,
    put: FuncId,
    json: FuncId,
}

impl Externals {
    fn declare(module: &mut ObjectModule, call_conv: CallConv) -> Result<Self> {
        let mut import = |name: &str, params: &[types::Type], returns: bool| -> Result<FuncId> {
            let mut signature = ir::Signature::new(call_conv);
            for &param in params {
                signature.params.push(AbiParam::new(param));
            }
            if returns {
                signature.returns.push(AbiParam::new(POINTER));
            }
            Ok(module.declare_function(name, Linkage::Import, &signature)?)
        };
        Ok(Externals {
            null: import(EXTERNAL_NULL, &[], true)?,
            truth: import(EXTERNAL_BOOL, &[types::I8], true)?,
            int: import(EXTERNAL_INT, &[types::I64], true)?,
            string: import(EXTERNAL_STRING, &[POINTER], true)?,
            object: import(EXTERNAL_OBJECT, &[], true)?,
            put: import(EXTERNAL_PUT, &[POINTER, POINTER, POINTER], false)?,
            json: import(EXTERNAL_JSON, &[POINTER], true)?,
        })
    }
}

/// One encoder per declaration an entry reaches, each a function of this object's own: a value of
/// the declaration in, the form it is written as out.
struct Encoders {
    call_conv: CallConv,
    ids: BTreeMap<String, FuncId>,
    pending: Vec<String>,
}

impl Encoders {
    fn new(call_conv: CallConv) -> Self {
        Encoders {
            call_conv,
            ids: BTreeMap::new(),
            pending: Vec::new(),
        }
    }

    fn signature(&self) -> ir::Signature {
        let mut signature = ir::Signature::new(self.call_conv);
        signature.params.push(AbiParam::new(POINTER));
        signature.returns.push(AbiParam::new(POINTER));
        signature
    }

    /// The encoder for `declared`, declared the first time it is asked for and defined once every
    /// entry has been.
    fn of(&mut self, module: &mut ObjectModule, declared: &str) -> Result<FuncId> {
        if let Some(&id) = self.ids.get(declared) {
            return Ok(id);
        }
        let id = module.declare_function(
            &format!("$encode${declared}"),
            Linkage::Local,
            &self.signature(),
        )?;
        self.ids.insert(declared.to_string(), id);
        self.pending.push(declared.to_string());
        Ok(id)
    }
}

/// The function being emitted, and what it reaches while it builds a form.
struct Writing<'w, 'f> {
    builder: &'w mut FunctionBuilder<'f>,
    module: &'w mut ObjectModule,
    declared: &'w Declared<'w>,
    literals: &'w Literals,
    externals: &'w Externals,
    encoders: &'w mut Encoders,
}

impl Writing<'_, '_> {
    fn call(&mut self, reached: FuncId, arguments: &[ir::Value]) -> ir::Value {
        let reaching = self.module.declare_func_in_func(reached, self.builder.func);
        let called = self.builder.ins().call(reaching, arguments);
        self.builder.inst_results(called)[0]
    }

    fn call_for_effect(&mut self, reached: FuncId, arguments: &[ir::Value]) {
        let reaching = self.module.declare_func_in_func(reached, self.builder.func);
        self.builder.ins().call(reaching, arguments);
    }

    /// A string written into the object, a literal of the runtime's own layout: a key, or a
    /// case's name.
    fn literal(&mut self, text: &str) -> Result<ir::Value> {
        text_in_the_object(self.builder, self.module, self.literals, text)
    }

    fn object(&mut self) -> ir::Value {
        self.call(self.externals.object, &[])
    }

    fn put(&mut self, object: ir::Value, key: &str, item: ir::Value) -> Result<()> {
        let key = self.literal(key)?;
        self.call_for_effect(self.externals.put, &[object, key, item]);
        Ok(())
    }

    fn name(&mut self, name: &str) -> Result<ir::Value> {
        let spelt = self.literal(name)?;
        Ok(self.call(self.externals.string, &[spelt]))
    }

    /// What an answer leaves as.
    fn output(&mut self, output: &BoundaryOutput, answer: ir::Value) -> Result<ir::Value> {
        match output {
            BoundaryOutput::Scalar { scalar } => self.scalar(*scalar, answer),
            BoundaryOutput::Nominal { declared } => self.named(declared, answer),
            BoundaryOutput::Cases { cases, form, .. } => self.alternatives(cases, form, answer),
            BoundaryOutput::ListOf { .. }
            | BoundaryOutput::SetOf { .. }
            | BoundaryOutput::MapOf { .. } => Err(not_lowered(format!(
                "an answer written as {}",
                output.ty().spelt()
            ))),
        }
    }

    fn scalar(&mut self, scalar: LeafScalar, value: ir::Value) -> Result<ir::Value> {
        match scalar.prim() {
            Prim::Int => Ok(self.call(self.externals.int, &[value])),
            Prim::Bool => Ok(self.call(self.externals.truth, &[value])),
            Prim::String => Ok(self.call(self.externals.string, &[value])),
            other => Err(not_lowered(format!(
                "a {} written at a boundary",
                other.spelt()
            ))),
        }
    }

    fn named(&mut self, declared: &str, value: ir::Value) -> Result<ir::Value> {
        let encoder = self.encoders.of(self.module, declared)?;
        Ok(self.call(encoder, &[value]))
    }

    /// A value standing where it has no key of its own: an absent one is written `null`.
    fn value(&mut self, shape: &CodecShape, value: ir::Value) -> Result<ir::Value> {
        match shape {
            CodecShape::Scalar { scalar } => self.scalar(*scalar, value),
            CodecShape::Named { declared } => self.named(declared, value),
            CodecShape::OptionOf { present } => {
                let absent = self.builder.create_block();
                let held = self.builder.create_block();
                let written = self.builder.create_block();
                self.builder.append_block_param(written, POINTER);
                let nothing = self.builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING);
                self.builder.ins().brif(nothing, absent, &[], held, &[]);

                self.builder.switch_to_block(absent);
                let null = self.call(self.externals.null, &[]);
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
    ) -> Result<()> {
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
    fn held(&mut self, present: &CodecShape, holding: ir::Value) -> Result<ir::Value> {
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
    fn fields_into(&mut self, object: ir::Value, fields: &[Field], value: ir::Value) -> Result<()> {
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
    fn declaration(&mut self, key: &str, value: ir::Value) -> Result<ir::Value> {
        match self.declared.shape(key)? {
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
    ) -> Result<ir::Value> {
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
            let shape = self.declared.shape(key)?;
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
    /// object that carries its tag, rather than the tag put into whatever the case's encoder
    /// handed back, so what a `put` is given is never a form this function has to take on trust.
    fn case(
        &mut self,
        key: &str,
        shape: &Declaration,
        form: &AlternativesForm,
        value: ir::Value,
    ) -> Result<ir::Value> {
        match (form, shape) {
            (_, Declaration::Sum { .. }) => bail!(
                "{key} stands as a case and is a sum, where the checker answers the cases a sum \
                 descends to"
            ),
            (AlternativesForm::Enumeration, Declaration::Unit { .. }) => self.name(shape.name()),
            // Refused when the document was read (`Declared::of`), so reaching here is this
            // compiler's own mistake and not the document's.
            (
                AlternativesForm::Enumeration,
                Declaration::Product { .. } | Declaration::Newtype { .. },
            ) => bail!("{key} carries fields and was admitted as a case of an enumeration"),
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
                let inner = self.named(key, value)?;
                self.put(object, contents, inner)?;
                Ok(object)
            }
        }
    }
}
