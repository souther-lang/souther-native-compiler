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
//! a published behavior's the other way round, and made of the same words: a behavior's boundary
//! has no optional, so either way it is a word for each parameter and room for one answer.
//!
//! A list is handed across as one word too, an address a host never reads behind, and a host
//! builds one and reads one through functions the object defines ([`define_lists`]). Those put an
//! element in its slot and take one out as a field of the element's type is handed across, so a
//! list crosses wherever its element does and in no other way: a list of optionals, a list of
//! lists and an optional list are the same few functions over the same words.
//!
//! Every function is emitted from the [`HostFunction`] a host is told about, and put on the
//! [`Surface`] where it is emitted, so what the object defines for a host and what the header and
//! the manifest say it defines are one decision.

use super::{
    COUNT_NO_LIST_HOLDS, Declared, Emitting, Lowered, NO_ARM, POINTER, Runs, TRUSTED, Tagged,
    accepted, into_slot, machine_type, not_lowered, out_of_slot, out_slot,
};
use crate::codec::write::Writing;
use crate::codec::{Codecs, Runtime};
use crate::interface::{DeclarationSurface, HostFunction, HostImplementation, Surface, machine};
use crate::manifest;
use crate::transport::{
    BoundaryInput, BoundaryOutput, Case, Declaration, DeclaredBy, Prim, Program, Requirement, Ty,
};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, TrapCode, types};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{DataDescription, FuncId, Linkage, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    ANSWERED, HELD, HostListOperation, HostParameter, HostWord, IMPLEMENTATION_ANSWERS,
    INJECTION_PROTOCOL_VIOLATION, INJECTION_UNBOUND, LIST_ELEMENTS, LIST_LENGTH, NOTHING, SLOT,
    TOKEN, field_at, host_behavior_answer_case_symbol, host_behavior_symbol, host_case_symbol,
    host_constructor_symbol, host_decode_symbol, host_encode_symbol, host_field_symbol,
    host_implementation_type, host_list_symbol, host_register_symbol, host_value_symbol,
    room_for_held, room_for_list,
};
use std::collections::BTreeMap;

/// How a value of a type is handed to a host and taken from one.
///
/// A whole value in one machine word, or a presence beside one, and nothing else: a host is handed
/// what it can hold without asking where anything is kept. A type this does not answer for is not
/// handed across, and what is refused for it is only the operation that would hand it: a
/// constructor needs every field handed over, a reader only its own field, so a field with no way
/// across keeps its type from being built by a host and keeps none of its siblings from being read.
///
/// A list is one word here, whatever its elements are. How an element crosses is a `Host` of its
/// own, which decides which functions a host builds and reads the list through ([`Lists`]), and
/// not how the list itself is handed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
        self.words().into_iter().map(HostParameter::Room).collect()
    }

    /// The words a value of this is handed over as, in order.
    fn words(self) -> Vec<HostWord> {
        match self {
            Host::Whole(word) => vec![word],
            Host::Present(word) => vec![HostWord::Bool, word],
        }
    }

    /// A value of this as the generated code holds one, out of the words a host handed over for it,
    /// taken from `given` in order: the word itself, or room holding the value where the presence
    /// beside it is not nought, and nothing where it is.
    fn taken(
        self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        allocate: FuncId,
        given: &mut impl Iterator<Item = ir::Value>,
    ) -> ir::Value {
        match self {
            Host::Whole(_) => given.next().expect("a word for every value handed over"),
            Host::Present(_) => {
                let present = given.next().expect("a presence for every optional");
                let value = given.next().expect("a value beside every presence");
                held(builder, module, allocate, present, value)
            }
        }
    }

    /// A value of this, out of the slot the generated code holds it in, as a host is handed one:
    /// the value itself, or whether there is one, with the value written through `room` only where
    /// there is. `room` is for an optional, and none for anything else.
    fn handed(
        self,
        builder: &mut FunctionBuilder,
        slot: ir::Value,
        room: Option<ir::Value>,
    ) -> ir::Value {
        match self {
            Host::Whole(word) => out_of_slot(builder, slot, machine(word)),
            Host::Present(word) => {
                let room = room.expect("room for the value of an optional");
                let there = builder.create_block();
                let absent = builder.create_block();
                let joined = builder.create_block();
                builder.append_block_param(joined, types::I8);
                let present = builder.ins().icmp_imm_s(IntCC::NotEqual, slot, NOTHING);
                builder.ins().brif(present, there, &[], absent, &[]);

                builder.switch_to_block(there);
                let held = builder.ins().load(types::I64, TRUSTED, slot, HELD as i32);
                let value = out_of_slot(builder, held, machine(word));
                builder.ins().store(TRUSTED, value, room, 0);
                let yes = builder.ins().iconst(types::I8, 1);
                builder.ins().jump(joined, &[yes.into()]);

                builder.switch_to_block(absent);
                let no = builder.ins().iconst(types::I8, 0);
                builder.ins().jump(joined, &[no.into()]);

                builder.switch_to_block(joined);
                builder.block_params(joined)[0]
            }
        }
    }

    /// How a manifest says an element crosses as this.
    fn element(self) -> manifest::Element {
        match self {
            Host::Whole(word) => manifest::Element::Whole(word.into()),
            Host::Present(word) => manifest::Element::Present(word.into()),
        }
    }
}

/// Each way an element of a list crosses where a list crosses to a host, by the module whose
/// functions the list crosses in, in the order they were first needed.
///
/// Worked out from the positions that cross and nothing else, so a list no host is handed or hands
/// over has no functions defined for it. What an element crosses as is all a function here needs:
/// the functions for a list of one declared type are the ones for a list of any other.
#[derive(Default)]
pub(crate) struct Lists {
    needed: BTreeMap<String, Vec<Host>>,
}

impl Lists {
    /// Every list `ty` is or holds, where a value of `ty` crosses in a function of `module`'s: a
    /// list of lists needs the functions for the outer one and for the inner one, and an optional
    /// list those for the list.
    fn need(&mut self, module: &str, ty: &Ty) {
        match ty {
            Ty::List { list } => {
                let element = Host::of(list).expect("a list crosses only where its element does");
                let needed = self.needed.entry(module.to_string()).or_default();
                if !needed.contains(&element) {
                    needed.push(element);
                }
                self.need(module, list);
            }
            Ty::Option { option } => self.need(module, option),
            _ => {}
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
        // The address of the list, where its element crosses: a host builds and reads one through
        // the functions for what the element crosses as, so a list whose element does not cross
        // is one a host could hold and do nothing with.
        Ty::List { list } => Host::of(list).map(|_| HostWord::List),
        // What holds a union holds one of its members, each of which says which it is. A host is
        // handed one where every member is a declared type, and asks which through a sum's own
        // reader, or, for a union a behavior answers, through the behavior's. What carries a
        // primitive or a case the language gives is not something a host is handed yet.
        Ty::Union { union } => union
            .iter()
            .all(|case| matches!(case, Case::Declared { .. }))
            .then_some(HostWord::Value),
        // An optional inside an optional would need a presence for each, and nothing asks for one.
        Ty::Option { .. } => None,
        // No layout yet, and when there is one a host reaches it through operations of its own.
        Ty::Set { .. } | Ty::Map { .. } => None,
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
/// and puts each type and what reaches it on `surface`, and every list a field crosses as on
/// `lists`.
pub(crate) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    surface: &mut Surface,
    lists: &mut Lists,
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
            let sum = Ty::Declared {
                declared: key.clone(),
            };
            let symbol = host_case_symbol(module_name, name);
            if let Some(casing) = expose_case(emitting, symbol, &sum, cases)? {
                described.cased_by(&casing);
            }
            surface.declaration(module_name, described);
            continue;
        }
        let fields = declaration.fields();
        let handed: Option<Vec<Host>> = fields.iter().map(|it| Host::of(&it.codec.ty())).collect();
        if let Some(handed) = handed {
            for field in fields {
                lists.need(module_name, &field.codec.ty());
            }
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
            lists.need(module_name, &field.codec.ty());
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

/// Defines what a host asks which of `cases` a value of `ty` is through, under `symbol`, where every
/// one of them is a declared type. None where one is not: a host is handed a value of a declared
/// type, and never what carries a primitive or a case the language gives.
///
/// The one reader of a case a host is given, whatever the cases are the cases of: a sum's, or a
/// union's a behavior answers.
fn expose_case(
    emitting: &mut Emitting,
    symbol: String,
    ty: &Ty,
    cases: &[Case],
) -> Lowered<Option<HostFunction>> {
    if !cases
        .iter()
        .all(|case| matches!(case, Case::Declared { .. }))
    {
        return Ok(None);
    }
    let casing = HostFunction {
        symbol,
        takes: vec![HostParameter::Given(HostWord::Value)],
        answers: Some(HostWord::Case),
    };
    let declared = emitting.declared;
    expose(emitting, casing, &mut |builder, module, given| {
        let value = Tagged::of(given[0], ty);
        which_case(builder, module, declared, cases, value)
    })
    .map(Some)
}

/// A behavior or a published value's entry, as a host would call it: what it runs, what each
/// parameter arrives as, and what it answers. A value takes nothing.
pub(crate) struct Entry<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub runs: FuncId,
    pub inputs: &'a [BoundaryInput],
    /// The names the declaration gives `inputs`, and none for a composition, which declares no
    /// parameters. A value takes nothing, and names nothing.
    pub names: Option<&'a [String]>,
    pub answers: Ty,
    /// The cases `answers` descends to, where it is a union no declaration names and a behavior's
    /// answer. None for a value, which a host is told nothing of the cases of yet.
    pub cases: Option<&'a [Case]>,
    /// What constructing it requires injected, in order, and nothing for a value, which is not
    /// constructed.
    pub requires: &'a [Requirement],
}

/// The word a behavior's parameter is handed over in, where a host can hand one over.
///
/// A word and never a presence beside one: what a parameter arrives as is a boundary shape, and no
/// boundary shape is an optional (the checker's E1402), so a behavior's crossing has no absence to
/// say. Read off the shape rather than off a type made of it, so that is not forgotten on the way.
fn taken_word(input: &BoundaryInput) -> Option<HostWord> {
    whole(&input.ty())
}

/// The word a behavior's answer is handed over in, for the same reason a word (E1313).
fn answered_word(output: &BoundaryOutput) -> Option<HostWord> {
    whole(&output.ty())
}

/// Defines what a host calls each published behavior this object defines through, and puts every
/// one of them on `surface`, with the function where a host can hand over what it takes and be
/// handed what it answers.
pub(crate) fn define_behaviors(
    emitting: &mut Emitting,
    surface: &mut Surface,
    lists: &mut Lists,
    behaviors: &[Entry],
) -> Lowered<()> {
    for behavior in behaviors {
        let symbol = host_behavior_symbol(behavior.module, behavior.name);
        let call = forward(emitting, lists, symbol, behavior)?;
        // Which case an answer is, asked of what a host was handed by the call, so only where
        // there is a call to be handed one by.
        let union = match behavior.cases {
            Some(cases) => {
                let case = match call {
                    Some(_) => expose_case(
                        emitting,
                        host_behavior_answer_case_symbol(behavior.module, behavior.name),
                        &behavior.answers,
                        cases,
                    )?,
                    None => None,
                };
                Some((cases, case))
            }
            None => None,
        };
        let takes: Vec<Ty> = behavior.inputs.iter().map(BoundaryInput::ty).collect();
        surface.behavior(
            behavior.module,
            behavior.name,
            behavior.names,
            &takes,
            &behavior.answers,
            union.as_ref().map(|(cases, case)| (*cases, case.as_ref())),
            behavior.requires,
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
    lists: &mut Lists,
    values: &[Entry],
) -> Lowered<()> {
    for value in values {
        let symbol = host_value_symbol(value.module, value.name);
        let read = forward(emitting, lists, symbol, value)?;
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
    lists: &mut Lists,
    symbol: String,
    entry: &Entry,
) -> Lowered<Option<HostFunction>> {
    let Some(handed) = entry
        .inputs
        .iter()
        .map(taken_word)
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    // A published value's answer may be an optional, which a behavior's never is.
    let Some(answered) = Host::of(&entry.answers) else {
        return Ok(None);
    };
    for input in entry.inputs {
        lists.need(entry.module, &input.ty());
    }
    lists.need(entry.module, &entry.answers);
    let mut takes: Vec<HostParameter> = handed.iter().copied().map(HostParameter::Given).collect();
    takes.extend(answered.room());
    let function = HostFunction {
        symbol,
        takes,
        answers: Some(HostWord::Status),
    };
    let runs = entry.runs;
    let answers = machine_type(&entry.answers)?;
    let exposed = expose(emitting, function, &mut |builder, module, params| {
        let mut given = params.iter().copied();
        let mut arguments = Vec::with_capacity(handed.len() + 1);
        // What a host hands over is what the entry takes, word for word.
        for (word, taken) in handed.iter().zip(entry.inputs) {
            assert_eq!(machine(*word), machine_type(&taken.ty())?);
            arguments.push(given.next().expect("a parameter for every one handed over"));
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
                // What the entry answered is an optional as a slot holds one.
                let holding = builder.ins().load(types::I64, TRUSTED, out, 0);
                let is = Host::Present(word).handed(builder, holding, Some(room));
                builder.ins().store(TRUSTED, is, present, 0);
                builder.ins().return_(&[status]);
            }
        }
        Ok(())
    })?;
    Ok(Some(exposed))
}

/// A behavior a module of this object declares with no body, which a host implements: the
/// function its symbol is defined as, and what each parameter arrives as and the answer leaves as.
pub(crate) struct Injected<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub answered_by: FuncId,
    pub inputs: &'a [BoundaryInput],
    /// The names the declaration gives `inputs`, which a behavior a host implements always has.
    pub names: &'a [String],
    pub output: &'a BoundaryOutput,
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
    lists: &mut Lists,
    injected: &[Injected],
    registrations: &Registrations,
) -> Lowered<()> {
    for behavior in injected {
        let spelt = format!("{}.{}", behavior.module, behavior.name);
        let takes: Vec<Ty> = behavior.inputs.iter().map(BoundaryInput::ty).collect();
        let answers = behavior.output.ty();
        let Some(handed) = behavior
            .inputs
            .iter()
            .map(taken_word)
            .collect::<Option<Vec<_>>>()
        else {
            return Err(not_lowered(format!(
                "the injected behavior {spelt}, which takes what a host cannot hand over"
            )));
        };
        let Some(answered) = answered_word(behavior.output) else {
            return Err(not_lowered(format!(
                "the injected behavior {spelt}, which answers what a host cannot be handed"
            )));
        };
        for taken in &takes {
            lists.need(behavior.module, taken);
        }
        lists.need(behavior.module, &answers);
        let mut given: Vec<HostParameter> =
            handed.iter().copied().map(HostParameter::Given).collect();
        given.push(HostParameter::Room(answered));
        let implementation = HostImplementation {
            type_name: host_implementation_type(behavior.module, behavior.name),
            takes: given,
            answers: HostWord::Status,
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

        let signature = super::signature_over(&takes, &answers, emitting.call_conv)?;
        let calling = implementation.signature(emitting.call_conv);
        let get = registrations.get;
        // What the behavior takes and answers is what a host hands over and is handed, word for
        // word, so the arguments go to the implementation as they came and its room is read as
        // the answer.
        for (word, taken) in handed.iter().zip(&takes) {
            assert_eq!(machine(*word), machine_type(taken)?);
        }
        assert_eq!(machine(answered), machine_type(&answers)?);
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

            // The implementation writes into room of this function's own and not through `out`,
            // which is written only once the status says there is an answer.
            builder.switch_to_block(bound);
            let room = out_slot(builder);
            let mut handing = arguments.to_vec();
            handing.push(room);
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
            let value = builder.ins().load(machine(answered), TRUSTED, room, 0);
            builder.ins().store(TRUSTED, value, out, 0);
            let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
            builder.ins().return_(&[ok]);
            Ok(())
        })?;

        surface.injection(
            behavior.module,
            behavior.name,
            behavior.names,
            &takes,
            &answers,
            emitting.declared,
            &implementation,
            &register,
        );
    }
    Ok(())
}

/// Defines what a host builds and reads each list in `lists` through, and puts them on `surface`.
///
/// Each takes and answers the words a field of the element's type is handed across in, so an
/// element is put in its slot and taken out of it by what a constructor and a reader of such a
/// field do ([`Host::taken`], [`Host::handed`]). Nothing here asks what the element's type is.
pub(crate) fn define_lists(
    emitting: &mut Emitting,
    surface: &mut Surface,
    lists: &Lists,
) -> Lowered<()> {
    let allocate = emitting.allocate;
    for (module_name, elements) in &lists.needed {
        for &element in elements {
            let (present, word) = match element {
                Host::Whole(word) => (false, word),
                Host::Present(word) => (true, word),
            };
            let symbol = |operation| host_list_symbol(module_name, present, word, operation);
            let mut takes = vec![HostParameter::Given(HostWord::Count)];
            takes.extend(element.words().into_iter().map(HostParameter::Slice));
            let constructing = HostFunction {
                symbol: symbol(HostListOperation::Construct),
                takes,
                answers: Some(HostWord::List),
            };
            let construct = expose(emitting, constructing, &mut |builder, module, given| {
                construct(builder, module, allocate, element, given);
                Ok(())
            })?;
            let measuring = HostFunction {
                symbol: symbol(HostListOperation::Length),
                takes: vec![HostParameter::Given(HostWord::List)],
                answers: Some(HostWord::Count),
            };
            let length = expose(emitting, measuring, &mut |builder, _, given| {
                let length = builder
                    .ins()
                    .load(types::I64, TRUSTED, given[0], LIST_LENGTH as i32);
                builder.ins().return_(&[length]);
                Ok(())
            })?;
            let mut takes = vec![
                HostParameter::Given(HostWord::List),
                HostParameter::Given(HostWord::Count),
            ];
            takes.extend(element.room());
            let indexing = HostFunction {
                symbol: symbol(HostListOperation::At),
                takes,
                answers: Some(HostWord::Bool),
            };
            let at = expose(emitting, indexing, &mut |builder, _, given| {
                element_at(builder, element, given);
                Ok(())
            })?;
            surface.list(module_name, element.element(), &construct, &length, &at);
        }
    }
    Ok(())
}

/// The most elements a list can be built with: the most whose room a count of bytes can say.
const MOST_ELEMENTS: i64 = (i64::MAX - room_for_list(0)) / SLOT;

/// A host's constructor of a list: room for the count it was handed, and each element, made of
/// what each column holds at its index, put in its slot.
///
/// A column holds one word for each element, as many bytes apart as the word is wide. A count below
/// nought, or past what room can be taken for, is a host handing over something no list is; it
/// traps rather than being read as some count a list could have, which would write past the room.
fn construct(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    allocate: FuncId,
    element: Host,
    given: &[ir::Value],
) {
    let (count, columns) = given.split_first().expect("a count before the columns");
    let count = *count;
    let beyond = builder
        .ins()
        .icmp_imm_s(IntCC::UnsignedGreaterThan, count, MOST_ELEMENTS);
    builder.ins().trapnz(
        beyond,
        TrapCode::user(COUNT_NO_LIST_HOLDS).expect("a trap code of its own"),
    );
    let along = builder.ins().imul_imm_s(count, SLOT);
    let size = builder.ins().iadd_imm_s(along, room_for_list(0));
    let taking = module.declare_func_in_func(allocate, builder.func);
    let taken = builder.ins().call(taking, &[size]);
    let list = builder.inst_results(taken)[0];
    builder
        .ins()
        .store(TRUSTED, count, list, LIST_LENGTH as i32);

    let head = builder.create_block();
    builder.append_block_param(head, types::I64);
    let step = builder.create_block();
    let built = builder.create_block();
    let start = builder.ins().iconst(types::I64, 0);
    builder.ins().jump(head, &[start.into()]);

    builder.switch_to_block(head);
    let index = builder.block_params(head)[0];
    let inside = builder.ins().icmp(IntCC::SignedLessThan, index, count);
    builder.ins().brif(inside, step, &[], built, &[]);

    builder.switch_to_block(step);
    let mut words = Vec::with_capacity(columns.len());
    for (column, word) in columns.iter().zip(element.words()) {
        let wide = machine(word);
        let along = builder.ins().imul_imm_s(index, i64::from(wide.bytes()));
        let at = builder.ins().iadd(*column, along);
        words.push(builder.ins().load(wide, TRUSTED, at, 0));
    }
    let value = element.taken(builder, module, allocate, &mut words.into_iter());
    let slot = into_slot(builder, value);
    let along = builder.ins().imul_imm_s(index, SLOT);
    let at = builder.ins().iadd(list, along);
    builder.ins().store(TRUSTED, slot, at, LIST_ELEMENTS as i32);
    let next = builder.ins().iadd_imm_s(index, 1);
    builder.ins().jump(head, &[next.into()]);

    builder.switch_to_block(built);
    builder.ins().return_(&[list]);
}

/// A host's reader of a list's element: one, with the element written through the host's room,
/// where the index is inside the list, and nought, with nothing written, where it is not.
///
/// An index is inside where it is below the length read without a sign, the way `List.get` reads
/// one, so a negative one is outside as well.
fn element_at(builder: &mut FunctionBuilder, element: Host, given: &[ir::Value]) {
    let [list, index, rooms @ ..] = given else {
        unreachable!("an element is read from a list, at an index, into room")
    };
    let length = builder
        .ins()
        .load(types::I64, TRUSTED, *list, LIST_LENGTH as i32);
    let inside = builder.ins().icmp(IntCC::UnsignedLessThan, *index, length);
    let there = builder.create_block();
    let outside = builder.create_block();
    builder.ins().brif(inside, there, &[], outside, &[]);

    builder.switch_to_block(outside);
    let no = builder.ins().iconst(types::I8, 0);
    builder.ins().return_(&[no]);

    builder.switch_to_block(there);
    let along = builder.ins().imul_imm_s(*index, SLOT);
    let at = builder.ins().iadd(*list, along);
    let slot = builder
        .ins()
        .load(types::I64, TRUSTED, at, LIST_ELEMENTS as i32);
    // The first room is for the element itself, or, for an optional, for whether there is one.
    let first = element.handed(builder, slot, rooms.get(1).copied());
    builder.ins().store(TRUSTED, first, rooms[0], 0);
    let yes = builder.ins().iconst(types::I8, 1);
    builder.ins().return_(&[yes]);
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
        fields.push(host.taken(builder, module, allocate, &mut given));
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
    let answer = host.handed(builder, slot, given.get(1).copied());
    builder.ins().return_(&[answer]);
}

/// A case reader: which of the cases the checker settled for a sum, or the boundary descended to
/// for a union a behavior answers, the value is, as its place among them.
///
/// The cases a sum or a union descends to, in the checker's order, which is what the document
/// carries: a case that is a sum again is answered as the case of it the value is, so a host is
/// told the concrete case the value is. Whether a host can go on to read that case is a separate
/// question, which the case's own publication answers: a case the module keeps has no readers
/// here. A value that is none of them is not a value of the sum or the union, which is a host
/// having handed over something else or this compiler having built it wrongly, and traps the way a
/// fork that runs out of arms does rather than answering a number that means nothing.
///
/// Defined only where every case is a declared type, since a host is handed nothing else.
fn which_case(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    declared: &Declared,
    cases: &[Case],
    value: Tagged,
) -> Lowered<()> {
    let which = value.which(builder);
    for (place, case) in cases.iter().enumerate() {
        let Case::Declared { declared: key } = case else {
            unreachable!("a case reader is defined only where every case is declared");
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
