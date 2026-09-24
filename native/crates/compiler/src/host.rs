//! What a host reaches a value of a model's own type through — building one, reading its fields or
//! which case it is, and reading it from and writing it to the language's external form — each a
//! function the object defines, so that nothing about where a value keeps what it holds leaves the
//! object.
//!
//! A host holds a value as an address it never looks behind, and hands it back to these and to the
//! behaviors. The layout is this backend's to change, and a host that read an offset would be a
//! third party to that and have to change with it; the same reason a host makes a string through
//! the runtime rather than laying one out.
//!
//! A host's value is not a second representation. Each of these runs on the one the generated code
//! keeps: a constructor here converts what it was handed and calls the declaration's own
//! constructor, which runs the clauses, and a reader reads the slot the lowering writes. What is
//! decided here is only how a value of each type is handed across ([`Host`]), and that is decided
//! apart from whether another object built by this compiler reads it the same way: the two
//! questions agree on most types today and are not one question — an optional is already where
//! they part.
//!
//! What is reached is what the declaring module publishes, and only in the object of the build
//! that declared it. A type a module keeps is reached by nothing here, whatever sum it is a case
//! of: what another party may do with a type is the module's answer about its surface, and working
//! out a second one from which sums reach it would be this side deciding visibility.

use super::{
    Declared, Emitting, Lowered, NO_ARM, POINTER, Runs, TRUSTED, accepted, into_slot, out_of_slot,
    out_slot,
};
use crate::codec::write::Writing;
use crate::codec::{Codecs, Runtime};
use crate::transport::{Case, Declaration, DeclaredBy, Prim, Program, Ty};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, AbiParam, InstBuilder, TrapCode, types};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{FuncId, Linkage, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    ANSWERED, HELD, NOTHING, WHICH, field_at, host_case_symbol, host_constructor_symbol,
    host_decode_symbol, host_encode_symbol, host_field_symbol, room_for_held,
};

/// How a value of a type is handed to a host and taken from one.
///
/// A whole value in one machine word, or a presence beside one, and nothing else yet: a host is
/// handed what it can hold without asking where anything is kept. A type this does not answer for
/// is not handed across, and what is refused for it is only the operation that would hand it: a
/// constructor needs every field handed over, a reader only its own field, so a field with no way
/// across keeps its type from being built by a host and keeps none of its siblings from being read.
#[derive(Clone, Copy)]
pub(crate) enum Host {
    /// The value itself: a number, a truth, or the address of text or of a value of a declared
    /// type, which a host holds and never reads behind.
    Whole(types::Type),
    /// An optional: whether there is a value, as a byte that is nought or one, and the value where
    /// there is. Never the address the generated code holds one at, and never that address being
    /// nothing — a host told absence by a null would be told how this backend keeps an optional.
    Present(types::Type),
}

impl Host {
    /// How a value of `ty` crosses to a host, where it does.
    pub(crate) fn of(ty: &Ty) -> Option<Host> {
        match ty {
            Ty::Option { option } => whole(option).map(Host::Present),
            _ => whole(ty).map(Host::Whole),
        }
    }

    /// What a host hands over for a value of this.
    fn given(self) -> Vec<types::Type> {
        match self {
            Host::Whole(ty) => vec![ty],
            Host::Present(ty) => vec![types::I8, ty],
        }
    }
}

/// The one word a value of `ty` is handed to a host in, where it is one.
///
/// Every primitive named, for the reason `machine_type` names them: one added to the language has
/// to be answered for here, not admitted by an arm standing for the rest. Not `machine_type` itself,
/// which answers how the generated code holds a value, and this answers how a host is handed one.
fn whole(ty: &Ty) -> Option<types::Type> {
    match ty {
        Ty::Prim { prim } => match prim {
            Prim::Int => Some(types::I64),
            Prim::Bool => Some(types::I8),
            // Made and read through the runtime's own functions, which is where a host already
            // makes one to hand a behavior.
            Prim::String => Some(POINTER),
            Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => None,
        },
        Ty::Declared { .. } => Some(POINTER),
        // What holds a union holds one of its members, each of which says which it is — where
        // every member is a declared type. A host asks which through the sum's own reader.
        Ty::Union { union } => union
            .iter()
            .all(|case| matches!(case, Case::Declared { .. }))
            .then_some(POINTER),
        // An optional inside an optional would need a presence for each, and nothing asks for one.
        Ty::Option { .. } => None,
        // No layout yet, and when there is one a host reaches it through operations of its own.
        Ty::List { .. } | Ty::Set { .. } | Ty::Map { .. } => None,
        // What a tuple or a function value holds is a contract between this compiler's own
        // functions, the way `means_the_same_elsewhere` says of a function value, and nothing
        // offers it to a host.
        Ty::Tuple { .. } | Ty::Fn { .. } => None,
    }
}

/// What one of these is emitted by, handed the function's parameters.
type Body<'b> =
    dyn FnMut(&mut FunctionBuilder, &mut ObjectModule, &[ir::Value]) -> Lowered<()> + 'b;

/// Defines what a host reaches every type a module of this build declares and publishes through.
pub(crate) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    program: &Program,
    runs: &Runs,
) -> Lowered<()> {
    let call_conv = emitting.call_conv;
    let declared = emitting.declared;
    let allocate = emitting.allocate;
    for declaration in &program.declarations {
        let key = declaration.key();
        if declaration.by() != DeclaredBy::AModule || !runs.publishes(&key) {
            continue;
        }
        let emit = |emitting: &mut Emitting,
                    symbol: String,
                    signature: ir::Signature,
                    body: &mut Body|
         -> Lowered<()> {
            let id = accepted(emitting.module.declare_function(
                &symbol,
                Linkage::Export,
                &signature,
            ));
            emitting.function(id, signature, body)
        };
        if runs.carries(&key) {
            let mut signature = ir::Signature::new(call_conv);
            signature.params.push(AbiParam::new(POINTER));
            signature.params.push(AbiParam::new(types::I64));
            signature.params.push(AbiParam::new(POINTER));
            signature.returns.push(AbiParam::new(types::I32));
            emit(
                emitting,
                host_decode_symbol(declaration.module(), declaration.name()),
                signature,
                &mut |builder, module, given| {
                    decode(builder, module, codecs, declared, &key, given);
                    Ok(())
                },
            )?;
            let mut signature = ir::Signature::new(call_conv);
            signature.params.push(AbiParam::new(POINTER));
            signature.returns.push(AbiParam::new(POINTER));
            let literals = emitting.literals;
            emit(
                emitting,
                host_encode_symbol(declaration.module(), declaration.name()),
                signature,
                &mut |builder, module, given| {
                    let json = {
                        let mut writing = Writing {
                            builder: &mut *builder,
                            module,
                            declared,
                            literals,
                            codecs: &mut *codecs,
                        };
                        let form = writing.named(&key, given[0]);
                        writing.call(Runtime::ExternalJson, &[form])
                    };
                    builder.ins().return_(&[json]);
                    Ok(())
                },
            )?;
        }
        if let Declaration::Sum { cases, .. } = declaration {
            if cases
                .iter()
                .all(|case| matches!(case, Case::Declared { .. }))
            {
                let mut signature = ir::Signature::new(call_conv);
                signature.params.push(AbiParam::new(POINTER));
                signature.returns.push(AbiParam::new(types::I32));
                emit(
                    emitting,
                    host_case_symbol(declaration.module(), declaration.name()),
                    signature,
                    &mut |builder, module, given| {
                        which_case(builder, module, declared, cases, given)
                    },
                )?;
            }
            continue;
        }
        let fields = declaration.fields();
        let handed: Option<Vec<Host>> = fields.iter().map(|it| Host::of(&it.codec.ty())).collect();
        if let Some(handed) = handed {
            let constructor = emitting.constructors.of(&key)?;
            let mut signature = ir::Signature::new(call_conv);
            for host in &handed {
                for ty in host.given() {
                    signature.params.push(AbiParam::new(ty));
                }
            }
            signature.params.push(AbiParam::new(POINTER));
            signature.returns.push(AbiParam::new(types::I32));
            emit(
                emitting,
                host_constructor_symbol(declaration.module(), declaration.name()),
                signature,
                &mut |builder, module, given| {
                    build(builder, module, allocate, constructor, &handed, given);
                    Ok(())
                },
            )?;
        }
        for (at, field) in fields.iter().enumerate() {
            let Some(host) = Host::of(&field.codec.ty()) else {
                continue;
            };
            let mut signature = ir::Signature::new(call_conv);
            signature.params.push(AbiParam::new(POINTER));
            match host {
                Host::Whole(ty) => signature.returns.push(AbiParam::new(ty)),
                Host::Present(_) => {
                    signature.params.push(AbiParam::new(POINTER));
                    signature.returns.push(AbiParam::new(types::I8));
                }
            }
            emit(
                emitting,
                host_field_symbol(declaration.module(), declaration.name(), &field.name),
                signature,
                &mut |builder, _, given| {
                    read(builder, at, host, given);
                    Ok(())
                },
            )?;
        }
    }
    Ok(())
}

/// A host's decoder: the bytes read as a document, the document read as a value of `key` by the
/// type's reader, and the reading handed to the host, which asks it what it came to.
///
/// Where a clause the reading ran ended without a value, the reading is dropped and the host is
/// answered that status, as it would be by the type's constructor: the document is not what went
/// wrong, and a reading would say nothing true about it.
fn decode(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    codecs: &mut Codecs,
    declared: &Declared,
    key: &str,
    given: &[ir::Value],
) {
    let [bytes, length, out] = given else {
        unreachable!("a decoder takes bytes, how many, and room for the reading")
    };
    let begin = codecs.runtime(module, Runtime::DecodeBegin);
    let root = codecs.runtime(module, Runtime::DecodeRoot);
    let end = codecs.runtime(module, Runtime::DecodeEnd);
    let abandon = codecs.runtime(module, Runtime::DecodeAbandon);
    let reader = codecs.reader(module, declared, key);
    let mut call = |builder: &mut FunctionBuilder, called: FuncId, arguments: &[ir::Value]| {
        let reaching = module.declare_func_in_func(called, builder.func);
        let call = builder.ins().call(reaching, arguments);
        builder.inst_results(call).first().copied()
    };

    let reading = call(builder, begin, &[*bytes, *length]).expect("a reading");
    let document = call(builder, root, &[reading]).expect("a root or none");

    let read = builder.create_block();
    let ended = builder.create_block();
    builder.append_block_param(ended, POINTER);
    let none = builder.ins().iconst(POINTER, NOTHING);
    builder
        .ins()
        .brif(document, read, &[], ended, &[none.into()]);

    builder.switch_to_block(read);
    let room = out_slot(builder);
    let at_the_root = builder.ins().iconst(POINTER, 0);
    let status = call(builder, reader, &[document, at_the_root, reading, room]).expect("a status");
    let answered = builder.create_block();
    let abandoned = builder.create_block();
    let is_answered = builder
        .ins()
        .icmp_imm_s(IntCC::Equal, status, i64::from(ANSWERED));
    builder
        .ins()
        .brif(is_answered, answered, &[], abandoned, &[]);

    builder.switch_to_block(abandoned);
    call(builder, abandon, &[reading]);
    builder.ins().return_(&[status]);

    builder.switch_to_block(answered);
    let value = builder.ins().load(POINTER, TRUSTED, room, 0);
    builder.ins().jump(ended, &[value.into()]);

    builder.switch_to_block(ended);
    let value = builder.block_params(ended)[0];
    call(builder, end, &[reading, value]);
    builder.ins().store(TRUSTED, reading, *out, 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);
}

/// A host's constructor: what it was handed, turned into what the declaration's own constructor
/// takes, and that constructor called with the host's own room for the answer.
///
/// The status is the constructor's, and so is whether anything is written through the room: it
/// writes the value there only once every clause has held, and a host that was answered
/// `InvariantNotHeld` was handed nothing.
fn build(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    allocate: FuncId,
    constructor: FuncId,
    handed: &[Host],
    params: &[ir::Value],
) {
    let mut given = params.iter().copied();
    let mut fields = Vec::with_capacity(handed.len());
    for host in handed {
        let field = match host {
            Host::Whole(_) => given
                .next()
                .expect("a parameter for every field handed over"),
            Host::Present(_) => {
                let present = given.next().expect("a presence for every optional");
                let value = given.next().expect("a value beside every presence");
                held(builder, module, allocate, present, value)
            }
        };
        fields.push(field);
    }
    let out = given.next().expect("room for the answer after the fields");
    fields.push(out);
    let reaching = module.declare_func_in_func(constructor, builder.func);
    let called = builder.ins().call(reaching, &fields);
    let status = builder.inst_results(called)[0];
    builder.ins().return_(&[status]);
}

/// An optional as the generated code holds one, from a presence and a value: room holding the
/// value where the presence is not nought, and nothing where it is.
fn held(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    allocate: FuncId,
    present: ir::Value,
    value: ir::Value,
) -> ir::Value {
    let there = builder.create_block();
    let absent = builder.create_block();
    let joined = builder.create_block();
    builder.append_block_param(joined, POINTER);
    builder.ins().brif(present, there, &[], absent, &[]);

    builder.switch_to_block(there);
    let taking = module.declare_func_in_func(allocate, builder.func);
    let size = builder.ins().iconst(types::I64, room_for_held());
    let taken = builder.ins().call(taking, &[size]);
    let holding = builder.inst_results(taken)[0];
    let slot = into_slot(builder, value);
    builder.ins().store(TRUSTED, slot, holding, HELD as i32);
    builder.ins().jump(joined, &[holding.into()]);

    builder.switch_to_block(absent);
    let nothing = builder.ins().iconst(POINTER, NOTHING);
    builder.ins().jump(joined, &[nothing.into()]);

    builder.switch_to_block(joined);
    builder.block_params(joined)[0]
}

/// A host's reader of the field at `at`: the value, or, for an optional, whether there is one,
/// with the value written through the host's room only where there is.
fn read(builder: &mut FunctionBuilder, at: usize, host: Host, given: &[ir::Value]) {
    let owner = given[0];
    let slot = builder
        .ins()
        .load(types::I64, TRUSTED, owner, field_at(at) as i32);
    match host {
        Host::Whole(ty) => {
            let value = out_of_slot(builder, slot, ty);
            builder.ins().return_(&[value]);
        }
        Host::Present(ty) => {
            let room = given[1];
            let there = builder.create_block();
            let absent = builder.create_block();
            let present = builder.ins().icmp_imm_s(IntCC::NotEqual, slot, NOTHING);
            builder.ins().brif(present, there, &[], absent, &[]);

            builder.switch_to_block(there);
            let held = builder.ins().load(types::I64, TRUSTED, slot, HELD as i32);
            let value = out_of_slot(builder, held, ty);
            builder.ins().store(TRUSTED, value, room, 0);
            let yes = builder.ins().iconst(types::I8, 1);
            builder.ins().return_(&[yes]);

            builder.switch_to_block(absent);
            let no = builder.ins().iconst(types::I8, 0);
            builder.ins().return_(&[no]);
        }
    }
}

/// A sum's case reader: which of the cases the checker settled for the sum the value is, as its
/// place among them.
///
/// The cases a sum descends to, in the checker's order, which is what the document carries: a
/// case that is a sum again is answered as the case of it the value is, so a host is told the
/// concrete case the value is. Whether a host can go on to read that case is a separate question,
/// which the case's own publication answers: a case the module keeps has no readers here. A value
/// that is none of them is not a value of the sum, which is a host having handed over something
/// else or this compiler having built it wrongly, and traps the way a fork that runs out of arms
/// does rather than answering a number that means nothing.
///
/// Defined only for a sum whose every case is a declared type, since only a value of one of those
/// says which it is.
fn which_case(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    declared: &Declared,
    cases: &[Case],
    given: &[ir::Value],
) -> Lowered<()> {
    let value = given[0];
    let which = builder.ins().load(POINTER, TRUSTED, value, WHICH as i32);
    for (place, case) in cases.iter().enumerate() {
        let Case::Declared { declared: key } = case else {
            unreachable!("a case reader is defined only for a sum whose every case is declared");
        };
        let token = declared.tag(module, key)?;
        let token = module.declare_data_in_func(token, builder.func);
        let expected = builder.ins().symbol_value(POINTER, token);
        let same = builder.ins().icmp(IntCC::Equal, which, expected);
        let this = builder.create_block();
        let next = builder.create_block();
        builder.ins().brif(same, this, &[], next, &[]);

        builder.switch_to_block(this);
        let place = builder.ins().iconst(
            types::I32,
            i64::try_from(place).expect("a sum has fewer cases than a status counts"),
        );
        builder.ins().return_(&[place]);

        builder.switch_to_block(next);
    }
    builder
        .ins()
        .trap(TrapCode::user(NO_ARM).expect("a trap code of its own"));
    Ok(())
}
