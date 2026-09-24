//! What a host reaches in the object: a behavior a module publishes, a value it publishes, and a
//! value of a type it publishes — building one, reading its fields or which case it is, and reading
//! it from and writing it to the language's external form. Each is a function the object defines,
//! so that nothing about where a value keeps what it holds, or how one object built by this
//! compiler calls another, leaves the object.
//!
//! A host holds a value as an address it never looks behind, and hands it back to these. The
//! layout is this backend's to change, and a host that read an offset would be a third party to
//! that and have to change with it; the same reason a host makes a string through the runtime
//! rather than laying one out.
//!
//! A host's value is not a second representation. Each of these runs on the one the generated code
//! keeps: a constructor here converts what it was handed and calls the declaration's own
//! constructor, which runs the clauses, a reader reads the slot the lowering writes, and a
//! behavior's entry converts what it was handed and calls the behavior. What is decided here is
//! only how a value of each type is handed across ([`Host`]), and that is decided apart from
//! whether another object built by this compiler reads it the same way: the two questions agree on
//! most types today and are not one question — an optional is already where they part.
//!
//! What is reached is what the declaring module publishes, and only in the object of the build
//! that declared it. A type a module keeps is reached by nothing here, whatever sum it is a case
//! of: what another party may do with a type is the module's answer about its surface, and working
//! out a second one from which sums reach it would be this side deciding visibility. A row's entry
//! and a boundary are not here either: they are how this project's own tests run the object, and
//! a host is told nothing of them.
//!
//! A host is also what answers a behavior with no body that declares nothing to depend on, and the
//! object of the build that declares one is what calls it: under the behavior's own symbol, with
//! what a host registered for it on the calling thread ([`define_injections`]). That crossing is
//! the one above the other way round, and is decided by the same [`Host`].
//!
//! Every function is emitted from the [`HostFunction`] a host is told about, and put on the
//! [`Surface`] where it is emitted, so what the object defines for a host and what the header and
//! the manifest say it defines are one decision.

use super::{
    Declared, Emitting, Lowered, NO_ARM, POINTER, Runs, TRUSTED, accepted, into_slot, machine_type,
    not_lowered, out_of_slot, out_slot,
};
use crate::codec::write::Writing;
use crate::codec::{Codecs, Runtime};
use crate::interface::{DeclarationSurface, HostFunction, Surface, machine};
use crate::transport::{Case, Declaration, DeclaredBy, Prim, Program, Ty};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, TrapCode, types};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{DataDescription, FuncId, Linkage, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    ANSWERED, HELD, HostParameter, HostWord, IMPLEMENTATION_ANSWERS, INJECTION_PROTOCOL_VIOLATION,
    INJECTION_UNBOUND, NOTHING, TOKEN, WHICH, field_at, host_behavior_symbol, host_case_symbol,
    host_constructor_symbol, host_decode_symbol, host_encode_symbol, host_field_symbol,
    host_implementation_type, host_register_symbol, host_value_symbol, room_for_held,
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
    Whole(HostWord),
    /// An optional: whether there is a value, as a byte that is nought or one, and the value where
    /// there is. Never the address the generated code holds one at, and never that address being
    /// nothing — a host told absence by a null would be told how this backend keeps an optional.
    Present(HostWord),
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
    fn given(self) -> Vec<HostParameter> {
        match self {
            Host::Whole(word) => vec![HostParameter::Given(word)],
            Host::Present(word) => vec![
                HostParameter::Given(HostWord::Bool),
                HostParameter::Given(word),
            ],
        }
    }

    /// The room a host hands over for a value of this to be written through.
    fn room(self) -> Vec<HostParameter> {
        match self {
            Host::Whole(word) => vec![HostParameter::Room(word)],
            Host::Present(word) => vec![
                HostParameter::Room(HostWord::Bool),
                HostParameter::Room(word),
            ],
        }
    }
}

/// The one word a value of `ty` is handed to a host in, where it is one.
///
/// Every primitive named, for the reason `machine_type` names them: one added to the language has
/// to be answered for here, not admitted by an arm standing for the rest. Not `machine_type` itself,
/// which answers how the generated code holds a value, and this answers how a host is handed one.
fn whole(ty: &Ty) -> Option<HostWord> {
    match ty {
        Ty::Prim { prim } => match prim {
            Prim::Int => Some(HostWord::Int),
            Prim::Bool => Some(HostWord::Bool),
            // Made and read through the runtime's own functions, which is where a host already
            // makes one to hand a behavior.
            Prim::String => Some(HostWord::String),
            Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => None,
        },
        Ty::Declared { .. } => Some(HostWord::Value),
        // What holds a union holds one of its members, each of which says which it is — where
        // every member is a declared type. A host asks which through the sum's own reader.
        Ty::Union { union } => union
            .iter()
            .all(|case| matches!(case, Case::Declared { .. }))
            .then_some(HostWord::Value),
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

/// Defines `function`, exported under its symbol, as what `body` emits, and answers it back for
/// whoever puts it on the surface: the function a host is told about is the one that was emitted.
fn expose(
    emitting: &mut Emitting,
    function: HostFunction,
    body: &mut Body,
) -> Lowered<HostFunction> {
    let signature = function.signature(emitting.call_conv);
    let id = accepted(emitting.module.declare_function(
        &function.symbol,
        Linkage::Export,
        &signature,
    ));
    emitting.function(id, signature, body)?;
    Ok(function)
}

/// Defines what a host reaches every type a module of this build declares and publishes through,
/// and puts each type and what reaches it on `surface`.
pub(crate) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    surface: &mut Surface,
    program: &Program,
    runs: &Runs,
) -> Lowered<()> {
    let declared = emitting.declared;
    let allocate = emitting.allocate;
    for declaration in &program.declarations {
        let key = declaration.key();
        if declaration.by() != DeclaredBy::AModule || !runs.publishes(&key) {
            continue;
        }
        let module_name = declaration.module();
        let name = declaration.name();
        let mut described = DeclarationSurface::of(declaration, declared);
        if runs.carries(&key) {
            let decoding = HostFunction {
                symbol: host_decode_symbol(module_name, name),
                takes: vec![
                    HostParameter::Given(HostWord::Bytes),
                    HostParameter::Given(HostWord::Count),
                    HostParameter::Room(HostWord::Decoded),
                ],
                answers: Some(HostWord::Status),
            };
            described.decoded_by(&expose(
                emitting,
                decoding,
                &mut |builder, module, given| {
                    decode(builder, module, codecs, declared, &key, given);
                    Ok(())
                },
            )?);
            let encoding = HostFunction {
                symbol: host_encode_symbol(module_name, name),
                takes: vec![HostParameter::Given(HostWord::Value)],
                answers: Some(HostWord::String),
            };
            let literals = emitting.literals;
            described.encoded_by(&expose(
                emitting,
                encoding,
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
            )?);
        }
        if let Declaration::Sum { cases, .. } = declaration {
            if cases
                .iter()
                .all(|case| matches!(case, Case::Declared { .. }))
            {
                let casing = HostFunction {
                    symbol: host_case_symbol(module_name, name),
                    takes: vec![HostParameter::Given(HostWord::Value)],
                    answers: Some(HostWord::Case),
                };
                described.cased_by(&expose(emitting, casing, &mut |builder, module, given| {
                    which_case(builder, module, declared, cases, given)
                })?);
            }
            surface.declaration(module_name, described);
            continue;
        }
        let fields = declaration.fields();
        let handed: Option<Vec<Host>> = fields.iter().map(|it| Host::of(&it.codec.ty())).collect();
        if let Some(handed) = handed {
            let constructor = emitting.constructors.of(&key)?;
            let mut takes: Vec<HostParameter> =
                handed.iter().flat_map(|host| host.given()).collect();
            takes.push(HostParameter::Room(HostWord::Value));
            let constructing = HostFunction {
                symbol: host_constructor_symbol(module_name, name),
                takes,
                answers: Some(HostWord::Status),
            };
            described.constructed_by(&expose(
                emitting,
                constructing,
                &mut |builder, module, given| {
                    build(builder, module, allocate, constructor, &handed, given);
                    Ok(())
                },
            )?);
        }
        for (at, field) in fields.iter().enumerate() {
            let Some(host) = Host::of(&field.codec.ty()) else {
                continue;
            };
            let reading = HostFunction {
                symbol: host_field_symbol(module_name, name, &field.name),
                takes: match host {
                    Host::Whole(_) => vec![HostParameter::Given(HostWord::Value)],
                    Host::Present(word) => vec![
                        HostParameter::Given(HostWord::Value),
                        HostParameter::Room(word),
                    ],
                },
                answers: Some(match host {
                    Host::Whole(word) => word,
                    Host::Present(_) => HostWord::Bool,
                }),
            };
            described.field_read_by(
                at,
                &expose(emitting, reading, &mut |builder, _, given| {
                    read(builder, at, host, given);
                    Ok(())
                })?,
            );
        }
        surface.declaration(module_name, described);
    }
    Ok(())
}

/// A behavior or a published value's entry, as a host would call it: what it runs, what that takes,
/// and what it answers.
pub(crate) struct Entry<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub runs: FuncId,
    pub takes: Vec<Ty>,
    pub answers: Ty,
}

/// Defines what a host calls each published behavior this object defines through, and puts every
/// one of them on `surface`, with the function where a host can hand over what it takes and be
/// handed what it answers.
pub(crate) fn define_behaviors(
    emitting: &mut Emitting,
    surface: &mut Surface,
    behaviors: &[Entry],
) -> Lowered<()> {
    for behavior in behaviors {
        let symbol = host_behavior_symbol(behavior.module, behavior.name);
        let call = forward(emitting, symbol, behavior)?;
        surface.behavior(
            behavior.module,
            behavior.name,
            &behavior.takes,
            &behavior.answers,
            emitting.declared,
            call.as_ref(),
        );
    }
    Ok(())
}

/// Defines what a host reads each value a module of this object publishes through, and puts every
/// one of them on `surface` the same way.
pub(crate) fn define_values(
    emitting: &mut Emitting,
    surface: &mut Surface,
    values: &[Entry],
) -> Lowered<()> {
    for value in values {
        let symbol = host_value_symbol(value.module, value.name);
        let read = forward(emitting, symbol, value)?;
        surface.value(
            value.module,
            value.name,
            &value.answers,
            emitting.declared,
            read.as_ref(),
        );
    }
    Ok(())
}

/// An entry a host calls in place of `entry`: what a host hands over, turned into what the entry
/// takes, the entry called, and what it answered written as a host takes it. The status is the
/// entry's own, and nothing is written through the host's room unless it is `ANSWERED`.
///
/// None where a host cannot hand over something the entry takes or be handed what it answers: the
/// entry is still what another object built by this compiler calls, and a host is told it is there
/// and that it has no way in.
fn forward(
    emitting: &mut Emitting,
    symbol: String,
    entry: &Entry,
) -> Lowered<Option<HostFunction>> {
    let Some(handed) = entry.takes.iter().map(Host::of).collect::<Option<Vec<_>>>() else {
        return Ok(None);
    };
    let Some(answered) = Host::of(&entry.answers) else {
        return Ok(None);
    };
    let mut takes: Vec<HostParameter> = handed.iter().flat_map(|host| host.given()).collect();
    takes.extend(answered.room());
    let function = HostFunction {
        symbol,
        takes,
        answers: Some(HostWord::Status),
    };
    let allocate = emitting.allocate;
    let runs = entry.runs;
    let answers = machine_type(&entry.answers)?;
    let exposed = expose(emitting, function, &mut |builder, module, params| {
        let mut given = params.iter().copied();
        let mut arguments = Vec::with_capacity(handed.len() + 1);
        for (host, taken) in handed.iter().zip(&entry.takes) {
            arguments.push(match host {
                Host::Whole(word) => {
                    // What a host hands over whole is what the entry takes, word for word; an
                    // optional is where the two part, and only there.
                    assert_eq!(machine(*word), machine_type(taken)?);
                    given.next().expect("a parameter for every one handed over")
                }
                Host::Present(_) => {
                    let present = given.next().expect("a presence for every optional");
                    let value = given.next().expect("a value beside every presence");
                    held(builder, module, allocate, present, value)
                }
            });
        }
        let reaching = module.declare_func_in_func(runs, builder.func);
        match answered {
            Host::Whole(word) => {
                assert_eq!(machine(word), answers);
                // The entry writes its answer through the host's own room, and only once it has
                // one, which is what a host is told of the room.
                arguments.push(given.next().expect("room for the answer"));
                let called = builder.ins().call(reaching, &arguments);
                let status = builder.inst_results(called)[0];
                builder.ins().return_(&[status]);
            }
            Host::Present(word) => {
                let present = given.next().expect("room for the presence");
                let room = given.next().expect("room for the value");
                let out = out_slot(builder);
                arguments.push(out);
                let called = builder.ins().call(reaching, &arguments);
                let status = builder.inst_results(called)[0];
                let answered = builder.create_block();
                let ended = builder.create_block();
                let is_answered =
                    builder
                        .ins()
                        .icmp_imm_s(IntCC::Equal, status, i64::from(ANSWERED));
                builder.ins().brif(is_answered, answered, &[], ended, &[]);

                builder.switch_to_block(ended);
                builder.ins().return_(&[status]);

                builder.switch_to_block(answered);
                let holding = builder.ins().load(POINTER, TRUSTED, out, 0);
                let there = builder.create_block();
                let absent = builder.create_block();
                let is_there = builder.ins().icmp_imm_s(IntCC::NotEqual, holding, NOTHING);
                builder.ins().brif(is_there, there, &[], absent, &[]);

                builder.switch_to_block(there);
                let slot = builder
                    .ins()
                    .load(types::I64, TRUSTED, holding, HELD as i32);
                let value = out_of_slot(builder, slot, machine(word));
                builder.ins().store(TRUSTED, value, room, 0);
                let yes = builder.ins().iconst(types::I8, 1);
                builder.ins().store(TRUSTED, yes, present, 0);
                builder.ins().return_(&[status]);

                builder.switch_to_block(absent);
                let no = builder.ins().iconst(types::I8, 0);
                builder.ins().store(TRUSTED, no, present, 0);
                builder.ins().return_(&[status]);
            }
        }
        Ok(())
    })?;
    Ok(Some(exposed))
}

/// A behavior a module of this object declares with no body, which a host implements: the
/// function its symbol is defined as, and what it takes and answers.
pub(crate) struct Injected<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub answered_by: FuncId,
    pub takes: Vec<Ty>,
    pub answers: Ty,
}

/// The runtime's functions a registration is kept by ([`souther_native_abi::INJECTION_GET`] and
/// [`souther_native_abi::INJECTION_EXCHANGE`]).
pub(crate) struct Registrations {
    pub get: FuncId,
    pub exchange: FuncId,
}

/// Defines each behavior in `injected` as a call to what a host registered for it on the calling
/// thread, and what a host registers one through, and puts both on `surface`.
///
/// Each has a key: a byte of this object's own that nothing reads, whose address is which behavior
/// a registration is for. Writable, so no link lays two keys at one address because their bytes
/// are alike.
///
/// Refused where a host cannot hand over what the behavior answers or be handed what it takes: a
/// host is the only thing that answers one, so a behavior no host can answer is one nothing can.
pub(crate) fn define_injections(
    emitting: &mut Emitting,
    surface: &mut Surface,
    injected: &[Injected],
    registrations: &Registrations,
) -> Lowered<()> {
    for behavior in injected {
        let spelt = format!("{}.{}", behavior.module, behavior.name);
        let Some(handed) = behavior
            .takes
            .iter()
            .map(Host::of)
            .collect::<Option<Vec<_>>>()
        else {
            return Err(not_lowered(format!(
                "the injected behavior {spelt}, which takes what a host cannot hand over"
            )));
        };
        let Some(answered) = Host::of(&behavior.answers) else {
            return Err(not_lowered(format!(
                "the injected behavior {spelt}, which answers what a host cannot be handed"
            )));
        };
        let mut takes: Vec<HostParameter> = handed.iter().flat_map(|host| host.given()).collect();
        takes.extend(answered.room());
        let implementation = HostFunction {
            symbol: host_implementation_type(behavior.module, behavior.name),
            takes,
            answers: Some(HostWord::Status),
        };

        let key = accepted(emitting.module.declare_data(
            &format!("$injection${spelt}"),
            Linkage::Local,
            true,
            false,
        ));
        let mut byte = DataDescription::new();
        byte.define(TOKEN.into());
        accepted(emitting.module.define_data(key, &byte));

        let register = host_register_symbol(behavior.module, behavior.name);
        let mut registering = ir::Signature::new(emitting.call_conv);
        registering.params.push(ir::AbiParam::new(POINTER));
        registering.returns.push(ir::AbiParam::new(POINTER));
        let id = accepted(emitting.module.declare_function(
            &register,
            Linkage::Export,
            &registering,
        ));
        let exchange = registrations.exchange;
        emitting.function(id, registering, |builder, module, given| {
            let key = module.declare_data_in_func(key, builder.func);
            let key = builder.ins().symbol_value(POINTER, key);
            let exchanging = module.declare_func_in_func(exchange, builder.func);
            let called = builder.ins().call(exchanging, &[key, given[0]]);
            let before = builder.inst_results(called)[0];
            builder.ins().return_(&[before]);
            Ok(())
        })?;

        let signature =
            super::signature_over(&behavior.takes, &behavior.answers, emitting.call_conv)?;
        let calling = implementation.signature(emitting.call_conv);
        let get = registrations.get;
        let allocate = emitting.allocate;
        let answers = machine_type(&behavior.answers)?;
        emitting.function(behavior.answered_by, signature, |builder, module, given| {
            let (arguments, out) = given.split_at(given.len() - 1);
            let out = out[0];
            let key = module.declare_data_in_func(key, builder.func);
            let key = builder.ins().symbol_value(POINTER, key);
            let getting = module.declare_func_in_func(get, builder.func);
            let got = builder.ins().call(getting, &[key]);
            let registered = builder.inst_results(got)[0];
            let bound = builder.create_block();
            let unbound = builder.create_block();
            builder.ins().brif(registered, bound, &[], unbound, &[]);

            builder.switch_to_block(unbound);
            let status = builder
                .ins()
                .iconst(types::I32, i64::from(INJECTION_UNBOUND));
            builder.ins().return_(&[status]);

            builder.switch_to_block(bound);
            let mut handing = Vec::with_capacity(arguments.len() + 2);
            for (host, argument) in handed.iter().zip(arguments) {
                match host {
                    Host::Whole(_) => handing.push(*argument),
                    Host::Present(word) => {
                        let (present, value) = presence(builder, *argument, machine(*word));
                        handing.push(present);
                        handing.push(value);
                    }
                }
            }
            let rooms: Vec<ir::Value> = match answered {
                Host::Whole(_) => vec![out_slot(builder)],
                Host::Present(_) => vec![out_slot(builder), out_slot(builder)],
            };
            handing.extend(&rooms);
            let calling = builder.import_signature(calling.clone());
            let called = builder.ins().call_indirect(calling, registered, &handing);
            let status = builder.inst_results(called)[0];

            let is_answered = builder
                .ins()
                .icmp_imm_s(IntCC::Equal, status, i64::from(ANSWERED));
            let answered_block = builder.create_block();
            let otherwise = builder.create_block();
            builder
                .ins()
                .brif(is_answered, answered_block, &[], otherwise, &[]);

            // Handed on as it came where an implementation may answer it, and the protocol broken
            // where it may not: not a status this object hands a caller as if a computation of
            // the model had come to it.
            builder.switch_to_block(otherwise);
            let mut handed_on = builder
                .ins()
                .iconst(types::I32, i64::from(INJECTION_PROTOCOL_VIOLATION));
            for may in IMPLEMENTATION_ANSWERS.iter().filter(|it| **it != ANSWERED) {
                let is = builder
                    .ins()
                    .icmp_imm_s(IntCC::Equal, status, i64::from(*may));
                handed_on = builder.ins().select(is, status, handed_on);
            }
            builder.ins().return_(&[handed_on]);

            builder.switch_to_block(answered_block);
            let value = match answered {
                Host::Whole(word) => {
                    assert_eq!(machine(word), answers);
                    builder.ins().load(machine(word), TRUSTED, rooms[0], 0)
                }
                Host::Present(word) => {
                    let present = builder.ins().load(types::I8, TRUSTED, rooms[0], 0);
                    let value = builder.ins().load(machine(word), TRUSTED, rooms[1], 0);
                    held(builder, module, allocate, present, value)
                }
            };
            builder.ins().store(TRUSTED, value, out, 0);
            let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
            builder.ins().return_(&[ok]);
            Ok(())
        })?;

        surface.injection(
            behavior.module,
            behavior.name,
            &behavior.takes,
            &behavior.answers,
            emitting.declared,
            &implementation,
            &register,
        );
    }
    Ok(())
}

/// An optional as a host is handed one, from the address the generated code holds it at: whether
/// there is a value, and the value, nought where there is none.
fn presence(
    builder: &mut FunctionBuilder,
    holding: ir::Value,
    word: types::Type,
) -> (ir::Value, ir::Value) {
    let there = builder.create_block();
    let absent = builder.create_block();
    let joined = builder.create_block();
    builder.append_block_param(joined, types::I8);
    builder.append_block_param(joined, word);
    let is_there = builder.ins().icmp_imm_s(IntCC::NotEqual, holding, NOTHING);
    builder.ins().brif(is_there, there, &[], absent, &[]);

    builder.switch_to_block(there);
    let slot = builder
        .ins()
        .load(types::I64, TRUSTED, holding, HELD as i32);
    let value = out_of_slot(builder, slot, word);
    let yes = builder.ins().iconst(types::I8, 1);
    builder.ins().jump(joined, &[yes.into(), value.into()]);

    builder.switch_to_block(absent);
    let no = builder.ins().iconst(types::I8, 0);
    let nought = builder.ins().iconst(word, 0);
    builder.ins().jump(joined, &[no.into(), nought.into()]);

    builder.switch_to_block(joined);
    let params = builder.block_params(joined);
    (params[0], params[1])
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
        Host::Whole(word) => {
            let value = out_of_slot(builder, slot, machine(word));
            builder.ins().return_(&[value]);
        }
        Host::Present(word) => {
            let room = given[1];
            let there = builder.create_block();
            let absent = builder.create_block();
            let present = builder.ins().icmp_imm_s(IntCC::NotEqual, slot, NOTHING);
            builder.ins().brif(present, there, &[], absent, &[]);

            builder.switch_to_block(there);
            let held = builder.ins().load(types::I64, TRUSTED, slot, HELD as i32);
            let value = out_of_slot(builder, held, machine(word));
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
