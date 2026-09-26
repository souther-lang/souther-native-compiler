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
//! only how a value of each type is handed across, the shape it crosses in ([`crossing`]), and that
//! is decided apart from whether another object built by this compiler reads it the same way: the
//! two questions agree on most types and are not one question — an optional is where they part.
//!
//! The shape is decided once, here, and everything else follows it: the words each function takes
//! and answers, how a value is taken from those words and written into them ([`take`], [`write`],
//! [`words_of`]), which functions a list or a function value of that shape needs ([`Crossings`]),
//! and what the manifest says of it. A binding reads the shape off the manifest and never works it
//! out again. Where a value does not cross, [`crossing`] says why and where in the type, and that is
//! what the manifest says beside the function that is not there.
//!
//! What is reached is what the declaring module publishes, and only in the object of the build
//! that declared it. A type a module keeps is reached by nothing here, whatever sum it is a case
//! of: what another party may do with a type is the module's answer about its surface, and working
//! out a second one from which sums reach it would be this side deciding visibility. A row's entry
//! and a boundary are not here either: they are how this project's own tests run the object, and
//! a host is told nothing of them.
//!
//! A host is also what answers a behavior with no body that declares nothing to depend on. The
//! object of the build that declares one makes a capability of what a host implements it as, and a
//! behavior requiring it calls through that ([`define_injections`]). That crossing is a published
//! behavior's the other way round, and a host's own function value is the same again
//! ([`define_crossings`]): the code either holds calls what a host wrote through [`call_hosted`],
//! which is the one place what an implementation answers is held.
//!
//! A behavior is called with the capabilities of what it was constructed with, first, and a host
//! makes the capability of one to hand where another requires it ([`souther_native_abi::host_bind_symbol`]).
//!
//! Every function is emitted from the [`HostFunction`] a host is told about, and put on the
//! [`Surface`] where it is emitted, so what the object defines for a host and what the header and
//! the manifest say it defines are one decision.

use super::{
    COUNT_NO_LIST_HOLDS, Declared, Emitting, Lowered, NO_ARM, POINTER, Runs, TRUSTED, Tagged,
    accepted, into_slot, invocation_signature, machine_type, not_lowered, out_of_slot, out_slot,
    token_of,
};
use crate::codec::write::Writing;
use crate::codec::{Codecs, Runtime};
use crate::interface::{
    DeclarationSurface, HostCall, HostFunction, HostImplementation, Surface, machine,
};
use crate::manifest::{Reason, Refusal, Step};
use crate::transport::{
    BoundaryInput, BoundaryOutput, Case, Declaration, DeclaredBy, Prim, Program, Requirement, Ty,
};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{self, InstBuilder, TrapCode, types};
use cranelift::frontend::FunctionBuilder;
use cranelift::module::{FuncId, Linkage, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    ANSWERED, CAPABILITY_ENVIRONMENT, CAPABILITY_INVOKE, FUNCTION_INVOKE, HELD,
    HOSTED_FUNCTION_HOSTED, HOSTED_IMPLEMENTATION, HOSTED_USERDATA, HostFunctionOperation,
    HostLeaf, HostListOperation, HostParameter, HostShape, HostWord, IMPLEMENTATION_ANSWERS,
    INJECTION_PROTOCOL_VIOLATION, INJECTION_UNBOUND, LIST_ELEMENTS, LIST_LENGTH, NOTHING, SLOT,
    field_at, host_behavior_answer_case_symbol, host_behavior_symbol, host_bind_symbol,
    host_case_symbol, host_constructor_symbol, host_decode_host_value_symbol, host_decode_symbol,
    host_encode_symbol, host_field_symbol, host_function_symbol, host_implement_symbol,
    host_implementation_type, host_list_symbol, host_value_symbol, member_at, room_for_held,
    room_for_list, room_for_members,
};
use std::collections::BTreeMap;

/// Which way a value crosses: handed over by a host, or handed to one.
///
/// The words are the same either way, and what may cross is not: a union no declaration names is
/// handed over as a value of one of its members, which already says which it is, and handed to a
/// host with nothing to say which it is but what a behavior's answer says ([`crossing`]). What a
/// function value takes crosses the other way round from the value itself: a host handed a
/// function hands over what it calls it with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Direction {
    /// A host hands it over.
    Given,
    /// A host is handed it.
    Handed,
}

impl Direction {
    fn turned(self) -> Direction {
        match self {
            Direction::Given => Direction::Handed,
            Direction::Handed => Direction::Given,
        }
    }
}

/// The shape a value of `ty` crosses between a host and the object in, crossing the way
/// `direction` says, or why it does not and where in `ty` that stands.
///
/// Every type named, for the reason `machine_type` names them: one added to the language has to be
/// answered for here, not admitted by an arm standing for the rest. Not `machine_type` itself, which
/// answers how the generated code holds a value, and this answers how a host is handed one — though
/// the two agree on the word a value is held in ([`held_as`]), which each function built from a
/// shape holds them to.
///
/// A union handed to a host is refused here wherever it stands. A behavior's answer is the one
/// place it is not, since the behavior says which case it is ([`host_behavior_answer_case_symbol`]),
/// and that is the behavior's to decide and not this.
pub(crate) fn crossing(ty: &Ty, direction: Direction) -> Result<HostShape, Refusal> {
    let refused = |reason| Refusal {
        reason,
        path: Vec::new(),
    };
    match ty {
        Ty::Prim { prim } => match prim {
            Prim::Int => Ok(HostShape::Leaf(HostLeaf::Int)),
            Prim::Bool => Ok(HostShape::Leaf(HostLeaf::Bool)),
            // Made and read through the runtime's own functions, which is where a host already
            // makes one to hand a behavior.
            Prim::String => Ok(HostShape::Leaf(HostLeaf::String)),
            Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => Err(refused(Reason::NoRepresentation)),
        },
        Ty::Ref {
            named: Case::Declared { .. },
        } => Ok(HostShape::Leaf(HostLeaf::Value)),
        // A primitive named as a type is laid out nowhere here (`named_as_a_type`), and a case the
        // language gives is one no behavior may take or answer on its own (the checker's E1325),
        // so neither is handed to a host.
        Ty::Ref {
            named: Case::Primitive { .. } | Case::Language { .. },
        } => Err(refused(Reason::NoRepresentation)),
        // What holds a union holds one of its members, each of which says which it is. A host
        // hands one over where it could hand over each member: a declared case is a value as it
        // is, a primitive is carried through the runtime where a host hands the primitive over at
        // all, and a case the language gives holds nothing. Handed one, a host has nothing to ask
        // which it is.
        Ty::Union { union } => match direction {
            Direction::Handed => Err(refused(Reason::NoDiscriminator)),
            Direction::Given => {
                for case in union.iter() {
                    if let Case::Primitive { prim } = case {
                        crossing(&Ty::Prim { prim: *prim }, direction)?;
                    }
                }
                Ok(HostShape::Leaf(HostLeaf::Value))
            }
        },
        Ty::Option { option } => Ok(HostShape::Option(Box::new(
            crossing(option, direction).map_err(|it| under(Step::Option, it))?,
        ))),
        Ty::Tuple { tuple } => Ok(HostShape::Product(
            tuple
                .iter()
                .enumerate()
                .map(|(at, member)| {
                    crossing(member, direction).map_err(|it| under(Step::Member(at), it))
                })
                .collect::<Result<_, _>>()?,
        )),
        // A list crosses as its address, and its elements through the functions for their shape,
        // so a list whose element does not cross is one a host could hold and do nothing with.
        Ty::List { list } => Ok(HostShape::List(Box::new(
            crossing(list, direction).map_err(|it| under(Step::Element, it))?,
        ))),
        // Called by whoever holds it: what it takes is handed the other way from it.
        Ty::Fn { fn_ } => Ok(HostShape::Function {
            takes: fn_
                .takes
                .iter()
                .enumerate()
                .map(|(at, taken)| {
                    crossing(taken, direction.turned()).map_err(|it| under(Step::Takes(at), it))
                })
                .collect::<Result<_, _>>()?,
            answers: Box::new(
                crossing(&fn_.answers, direction).map_err(|it| under(Step::Answers, it))?,
            ),
        }),
        // No layout yet, and when there is one a host reaches it through operations of its own.
        Ty::Set { .. } | Ty::Map { .. } => Err(refused(Reason::NoRepresentation)),
        Ty::Var { var } => crate::laid_out_nowhere(*var),
        // No value of either is made, so none is handed to a host or taken from one.
        Ty::Nothing { .. } | Ty::Never { .. } => Err(refused(Reason::NoValue)),
    }
}

/// `refusal`, as it stands one step further in.
fn under(step: Step, mut refusal: Refusal) -> Refusal {
    refusal.path.insert(0, step);
    refusal
}

/// The machine type the generated code holds a value crossing in `shape` in: the word itself where
/// it is one, and an address where it is anything else — of what holds an optional's value, of a
/// tuple's members, of a list, of a function value.
///
/// What `machine_type` answers for any type crossing in the shape, which each function built from a
/// type's shape asserts; a function value's shape is all the code it holds is built from, so this
/// is where the two are held to one answer.
fn held_as(shape: &HostShape) -> types::Type {
    match shape {
        HostShape::Leaf(leaf) => machine(leaf.word()),
        HostShape::Option(_)
        | HostShape::Product(_)
        | HostShape::List(_)
        | HostShape::Function { .. } => POINTER,
    }
}

/// Holds `shape` to being how the generated code holds a value of `ty`.
fn holds(shape: &HostShape, ty: &Ty) -> Lowered<()> {
    assert_eq!(
        held_as(shape),
        machine_type(ty)?,
        "{} crosses in {shape:?}, which is not how it is held",
        ty.spelt()
    );
    Ok(())
}

/// What a value is made in: the object, and the runtime's function taking room from the arena.
struct Making<'m> {
    module: &'m mut ObjectModule,
    allocate: FuncId,
}

impl Making<'_> {
    /// Room of `size` bytes, taken from the arena.
    fn room(&mut self, builder: &mut FunctionBuilder, size: i64) -> ir::Value {
        let taking = self
            .module
            .declare_func_in_func(self.allocate, builder.func);
        let size = builder.ins().iconst(types::I64, size);
        let taken = builder.ins().call(taking, &[size]);
        builder.inst_results(taken)[0]
    }
}

/// A value crossing in `shape` as the generated code holds one, out of the words a host handed over
/// for it, taken from `given` in order: a word as it is; an optional as room holding its value
/// where the presence before the value's words is not nought, and nothing where it is; a product as
/// room holding each member. The words of an optional's value are taken whether or not it holds
/// one, since a host hands them over either way, and read only where it does.
fn take(
    builder: &mut FunctionBuilder,
    making: &mut Making,
    shape: &HostShape,
    given: &mut dyn Iterator<Item = ir::Value>,
) -> ir::Value {
    match shape {
        HostShape::Leaf(_) | HostShape::List(_) | HostShape::Function { .. } => {
            given.next().expect("a word for every value handed over")
        }
        HostShape::Option(of) => {
            let present = given.next().expect("a presence for every optional");
            let words: Vec<ir::Value> = (0..of.words().len())
                .map(|_| given.next().expect("the words of an optional's value"))
                .collect();
            let there = builder.create_block();
            let absent = builder.create_block();
            let joined = builder.create_block();
            builder.append_block_param(joined, POINTER);
            builder.ins().brif(present, there, &[], absent, &[]);

            builder.switch_to_block(there);
            let value = take(builder, making, of, &mut words.into_iter());
            let holding = making.room(builder, room_for_held());
            let slot = into_slot(builder, value);
            builder.ins().store(TRUSTED, slot, holding, HELD as i32);
            builder.ins().jump(joined, &[holding.into()]);

            builder.switch_to_block(absent);
            let nothing = builder.ins().iconst(POINTER, NOTHING);
            builder.ins().jump(joined, &[nothing.into()]);

            builder.switch_to_block(joined);
            builder.block_params(joined)[0]
        }
        HostShape::Product(members) => {
            let values: Vec<ir::Value> = members
                .iter()
                .map(|member| take(builder, making, member, given))
                .collect();
            let room = making.room(builder, room_for_members(members.len()));
            for (at, value) in values.into_iter().enumerate() {
                let slot = into_slot(builder, value);
                builder
                    .ins()
                    .store(TRUSTED, slot, room, member_at(at) as i32);
            }
            room
        }
    }
}

/// `value`, a value crossing in `shape` as the generated code holds one, written as a host is
/// handed it: each word through its own room in `rooms`, in order. An optional writes whether it
/// holds a value, and its value's words only where it does: the room for them is left as the host
/// had it where it holds none, which is what a host is told of it.
fn write(builder: &mut FunctionBuilder, shape: &HostShape, value: ir::Value, rooms: &[ir::Value]) {
    match shape {
        HostShape::Leaf(_) | HostShape::List(_) | HostShape::Function { .. } => {
            builder.ins().store(TRUSTED, value, rooms[0], 0);
        }
        HostShape::Option(of) => {
            let present = builder.ins().icmp_imm_s(IntCC::NotEqual, value, NOTHING);
            builder.ins().store(TRUSTED, present, rooms[0], 0);
            let there = builder.create_block();
            let done = builder.create_block();
            builder.ins().brif(present, there, &[], done, &[]);

            builder.switch_to_block(there);
            let held = builder.ins().load(types::I64, TRUSTED, value, HELD as i32);
            let held = out_of_slot(builder, held, held_as(of));
            write(builder, of, held, &rooms[1..]);
            builder.ins().jump(done, &[]);

            builder.switch_to_block(done);
        }
        HostShape::Product(members) => {
            let mut at = 0;
            for (position, member) in members.iter().enumerate() {
                let slot =
                    builder
                        .ins()
                        .load(types::I64, TRUSTED, value, member_at(position) as i32);
                let held = out_of_slot(builder, slot, held_as(member));
                let wide = member.words().len();
                write(builder, member, held, &rooms[at..at + wide]);
                at += wide;
            }
        }
    }
}

/// `value`, a value crossing in `shape` as the generated code holds one, as the words a host is
/// handed it in when it is handed them as they are rather than through room: what a host's own
/// implementation is called with. An optional holding nothing is a presence of nought and a zero
/// for each of its value's words, which the implementation is told not to read.
fn words_of(builder: &mut FunctionBuilder, shape: &HostShape, value: ir::Value) -> Vec<ir::Value> {
    match shape {
        HostShape::Leaf(_) | HostShape::List(_) | HostShape::Function { .. } => vec![value],
        HostShape::Option(of) => {
            let widths: Vec<types::Type> = shape.words().into_iter().map(machine).collect();
            let there = builder.create_block();
            let absent = builder.create_block();
            let joined = builder.create_block();
            for wide in &widths {
                builder.append_block_param(joined, *wide);
            }
            let present = builder.ins().icmp_imm_s(IntCC::NotEqual, value, NOTHING);
            builder.ins().brif(present, there, &[], absent, &[]);

            builder.switch_to_block(there);
            let held = builder.ins().load(types::I64, TRUSTED, value, HELD as i32);
            let held = out_of_slot(builder, held, held_as(of));
            let mut words = vec![builder.ins().iconst(types::I8, 1)];
            words.extend(words_of(builder, of, held));
            let words: Vec<ir::BlockArg> = words.into_iter().map(Into::into).collect();
            builder.ins().jump(joined, &words);

            builder.switch_to_block(absent);
            let words: Vec<ir::BlockArg> = widths
                .iter()
                .map(|wide| builder.ins().iconst(*wide, 0).into())
                .collect();
            builder.ins().jump(joined, &words);

            builder.switch_to_block(joined);
            builder.block_params(joined).to_vec()
        }
        HostShape::Product(members) => {
            let mut words = Vec::new();
            for (position, member) in members.iter().enumerate() {
                let slot =
                    builder
                        .ins()
                        .load(types::I64, TRUSTED, value, member_at(position) as i32);
                let held = out_of_slot(builder, slot, held_as(member));
                words.extend(words_of(builder, member, held));
            }
            words
        }
    }
}

/// Each list and each function value a host is handed or hands over, by the shape it crosses in,
/// by the module whose functions it crosses in, in the order they were first needed.
///
/// Worked out from the shapes that cross and nothing else, so a list or a function no host is
/// handed or hands over has no functions defined for it. The shape is all a function here needs:
/// the functions for a list of one declared type are the ones for a list of any other.
#[derive(Default)]
pub(crate) struct Crossings {
    lists: BTreeMap<String, Vec<HostShape>>,
    functions: BTreeMap<String, Vec<HostShape>>,
}

impl Crossings {
    /// Every list and every function value `shape` is or holds, where it crosses in a function of
    /// `module`'s: a list of lists needs the functions for the outer one and for the inner one, a
    /// function taking a list those for the list, and an optional tuple those for what its members
    /// need.
    fn need(&mut self, module: &str, shape: &HostShape) {
        let once = |needed: &mut BTreeMap<String, Vec<HostShape>>, shape: &HostShape| {
            let needed = needed.entry(module.to_string()).or_default();
            if !needed.contains(shape) {
                needed.push(shape.clone());
            }
        };
        match shape {
            HostShape::Leaf(_) => {}
            HostShape::Option(of) => self.need(module, of),
            HostShape::Product(members) => {
                for member in members {
                    self.need(module, member);
                }
            }
            HostShape::List(element) => {
                once(&mut self.lists, element);
                self.need(module, element);
            }
            HostShape::Function { takes, answers } => {
                once(&mut self.functions, shape);
                for taken in takes {
                    self.need(module, taken);
                }
                self.need(module, answers);
            }
        }
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

/// Every word of `shapes`, one after another, each as `parameter` makes a parameter of it.
fn parameters(
    shapes: &[HostShape],
    parameter: fn(HostWord) -> HostParameter,
) -> Vec<HostParameter> {
    shapes
        .iter()
        .flat_map(HostShape::words)
        .map(parameter)
        .collect()
}

/// Defines what a host reaches every type a module of this build declares and publishes through,
/// and puts each type and what reaches it on `surface`, and every list and function value a field
/// crosses as on `crossings`.
///
/// A constructor takes every field handed over and a reader hands over only its own, so a field
/// that does not cross keeps its type from being built by a host and keeps none of its siblings
/// from being read. Each says why where it is not there.
pub(crate) fn define(
    emitting: &mut Emitting,
    codecs: &mut Codecs,
    surface: &mut Surface,
    crossings: &mut Crossings,
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
                    decode(
                        builder,
                        module,
                        codecs,
                        declared,
                        &key,
                        given,
                        Runtime::DecodeBegin,
                    );
                    Ok(())
                },
            )?);
            let decoding_a_host_value = HostFunction {
                symbol: host_decode_host_value_symbol(module_name, name),
                takes: vec![
                    HostParameter::Given(HostWord::Bytes),
                    HostParameter::Given(HostWord::Count),
                    HostParameter::Room(HostWord::Decoded),
                ],
                answers: Some(HostWord::Status),
            };
            described.host_value_decoded_by(&expose(
                emitting,
                decoding_a_host_value,
                &mut |builder, module, given| {
                    decode(
                        builder,
                        module,
                        codecs,
                        declared,
                        &key,
                        given,
                        Runtime::DecodeHostBegin,
                    );
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
                        let form = writing.declared(&key, given[0]);
                        writing.call(Runtime::ExternalJson, &[form])
                    };
                    builder.ins().return_(&[json]);
                    Ok(())
                },
            )?);
        }
        if let Declaration::Sum { cases, .. } = declaration {
            let sum = Ty::declared(key.clone());
            // Where a host could hand over each case, as it could a union of them.
            let crosses = crossing(
                &Ty::Union {
                    union: cases.clone(),
                },
                Direction::Given,
            )
            .is_ok();
            if crosses {
                let symbol = host_case_symbol(module_name, name);
                let casing = expose_case(emitting, symbol, &sum, cases)?;
                described.cased_by(&casing);
            }
            surface.declaration(module_name, described);
            continue;
        }
        let fields = declaration.fields();
        let handed: Result<Vec<HostShape>, Refusal> = fields
            .iter()
            .map(|field| {
                crossing(&field.codec.ty(), Direction::Given)
                    .map_err(|it| under(Step::Field(field.name.clone()), it))
            })
            .collect();
        match handed {
            Ok(handed) => {
                for (shape, field) in handed.iter().zip(fields) {
                    holds(shape, &field.codec.ty())?;
                    crossings.need(module_name, shape);
                }
                let constructor = emitting.constructors.of(&key)?;
                let mut takes = parameters(&handed, HostParameter::Given);
                takes.push(HostParameter::Room(HostWord::Value));
                let constructing = HostFunction {
                    symbol: host_constructor_symbol(module_name, name),
                    takes,
                    answers: Some(HostWord::Status),
                };
                let function = expose(emitting, constructing, &mut |builder, module, given| {
                    build(builder, module, allocate, constructor, &handed, given);
                    Ok(())
                })?;
                described.constructed_by(function, handed.clone());
            }
            Err(refusal) => described.not_constructed(refusal),
        }
        for (at, field) in fields.iter().enumerate() {
            let shape = match crossing(&field.codec.ty(), Direction::Handed) {
                Ok(shape) => shape,
                Err(refusal) => {
                    described.field_not_read(at, refusal);
                    continue;
                }
            };
            holds(&shape, &field.codec.ty())?;
            crossings.need(module_name, &shape);
            let mut takes = vec![HostParameter::Given(HostWord::Value)];
            takes.extend(parameters(
                std::slice::from_ref(&shape),
                HostParameter::Room,
            ));
            let reading = HostFunction {
                symbol: host_field_symbol(module_name, name, &field.name),
                takes,
                answers: None,
            };
            let function = expose(emitting, reading, &mut |builder, _, given| {
                read(builder, at, &shape, given);
                Ok(())
            })?;
            described.field_read_by(at, function, shape);
        }
        surface.declaration(module_name, described);
    }
    Ok(())
}

/// Defines what a host asks which of `cases` a value of `ty` is through, under `symbol`.
///
/// The one reader of a case a host is given, whatever the cases are the cases of: a sum's, or a
/// union's a behavior answers. It answers which case, and nothing of what the case holds: a
/// declared case is the value itself, and one no declaration names is read through the runtime
/// ([`souther_native_abi::HOST_CASES`]), which lays it out the same whatever union it stands in.
/// Which case is not a value of the model, and is answered as the function's return rather than
/// through room.
fn expose_case(
    emitting: &mut Emitting,
    symbol: String,
    ty: &Ty,
    cases: &[Case],
) -> Lowered<HostFunction> {
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
}

/// A behavior or a published value's entry, as a host would call it: what it runs, what each
/// parameter arrives as, and what it answers. A value takes nothing.
pub(crate) struct Entry<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub runs: FuncId,
    /// Whether `runs` is a behavior's symbol, which takes what the behavior was constructed with
    /// first ([`super::behavior_signature`]), and not a value's entry, which takes nothing more.
    pub constructed: bool,
    pub inputs: &'a [BoundaryInput],
    /// The names the declaration gives `inputs`, and none for a composition, which declares no
    /// parameters. A value takes nothing, and names nothing.
    pub names: Option<&'a [String]>,
    pub answers: Ty,
    /// The cases `answers` descends to, where it is a union no declaration names and a behavior's
    /// answer. None for a value, which says nothing of which case a union it answers is, so a host
    /// is handed one no way.
    pub cases: Option<&'a [Case]>,
}

/// A behavior a host constructs the capabilities of what a call is made with out of, whether or not
/// a host may call it by name ([`super::constructions`]).
pub(crate) struct Construction<'a> {
    pub module: &'a str,
    pub name: &'a str,
    /// The behavior's symbol, which a capability of it holds as its code.
    pub runs: FuncId,
    /// What constructing it requires injected, in order.
    pub requires: &'a [Requirement],
    /// Whether a host makes a capability of it, to hand where something requires it
    /// ([`souther_native_abi::host_bind_symbol`]).
    pub binds: bool,
}

/// Defines what a host makes the capability of each of `constructions` through, where something
/// may require it, and puts each on `surface` with what it requires.
pub(crate) fn define_constructions(
    emitting: &mut Emitting,
    surface: &mut Surface,
    constructions: &[Construction],
) -> Lowered<()> {
    for construction in constructions {
        let bind = if construction.binds {
            Some(bind(emitting, construction)?)
        } else {
            None
        };
        surface.construction(
            construction.module,
            construction.name,
            construction.requires,
            bind.as_ref(),
        );
    }
    Ok(())
}

/// Defines what a host calls each published behavior this object defines through, and puts every
/// one of them on `surface`, with the function where a host can hand over what it takes and be
/// handed what it answers, and why not where it cannot.
pub(crate) fn define_behaviors(
    emitting: &mut Emitting,
    surface: &mut Surface,
    crossings: &mut Crossings,
    behaviors: &[Entry],
) -> Lowered<()> {
    for behavior in behaviors {
        let symbol = host_behavior_symbol(behavior.module, behavior.name);
        let call = forward(emitting, crossings, symbol, behavior)?;
        // Which case an answer is, asked of what a host was handed by the call, so only where
        // there is a call to be handed one by.
        let union = match behavior.cases {
            Some(cases) => {
                let case = match call {
                    Ok(_) => Some(expose_case(
                        emitting,
                        host_behavior_answer_case_symbol(behavior.module, behavior.name),
                        &behavior.answers,
                        cases,
                    )?),
                    Err(_) => None,
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
            emitting.declared,
            call,
        );
    }
    Ok(())
}

/// What a host makes the capability of `behavior` through, out of capabilities of what it
/// requires: the behavior's symbol as the code, and the requirements as they were handed as what
/// the code is handed first. Nothing is copied, so what a host hands over is read where the
/// behavior runs.
fn bind(emitting: &mut Emitting, behavior: &Construction) -> Lowered<HostFunction> {
    let function = HostFunction {
        symbol: host_bind_symbol(behavior.module, behavior.name),
        takes: vec![
            HostParameter::Room(HostWord::Capability),
            HostParameter::Given(HostWord::Requirements),
        ],
        answers: None,
    };
    let runs = behavior.runs;
    expose(emitting, function, &mut |builder, module, given| {
        let [into, requirements] = given else {
            unreachable!("a capability is made from room and requirements");
        };
        let code = module.declare_func_in_func(runs, builder.func);
        let code = builder.ins().func_addr(POINTER, code);
        builder
            .ins()
            .store(TRUSTED, code, *into, CAPABILITY_INVOKE as i32);
        builder
            .ins()
            .store(TRUSTED, *requirements, *into, CAPABILITY_ENVIRONMENT as i32);
        builder.ins().return_(&[]);
        Ok(())
    })
}

/// Defines what a host reads each value a module of this object publishes through, and puts every
/// one of them on `surface` the same way.
pub(crate) fn define_values(
    emitting: &mut Emitting,
    surface: &mut Surface,
    crossings: &mut Crossings,
    values: &[Entry],
) -> Lowered<()> {
    for value in values {
        let symbol = host_value_symbol(value.module, value.name);
        let read = forward(emitting, crossings, symbol, value)?;
        surface.value(
            value.module,
            value.name,
            &value.answers,
            emitting.declared,
            read,
        );
    }
    Ok(())
}

/// An entry a host calls in place of `entry`: what a host hands over, taken into what the entry
/// takes, the entry called, and what it answered written through the host's room. The status is the
/// entry's own, and nothing is written through the host's room unless it is `ANSWERED`.
///
/// Refused, with why, where a host cannot hand over something the entry takes or be handed what it
/// answers: the entry is still what another object built by this compiler calls, and a host is told
/// it is there and why it has no way in.
fn forward(
    emitting: &mut Emitting,
    crossings: &mut Crossings,
    symbol: String,
    entry: &Entry,
) -> Lowered<Result<HostCall, Refusal>> {
    let answers = machine_type(&entry.answers)?;
    let takes: Vec<Ty> = entry.inputs.iter().map(BoundaryInput::ty).collect();
    let handed: Result<Vec<HostShape>, Refusal> = takes
        .iter()
        .enumerate()
        .map(|(at, ty)| crossing(ty, Direction::Given).map_err(|it| under(Step::Takes(at), it)))
        .collect();
    let handed = match handed {
        Ok(handed) => handed,
        Err(refusal) => return Ok(Err(refusal)),
    };
    // A union a behavior answers is told its case by the behavior ([`expose_case`]), and crosses
    // as the value it is. Nothing else answered is told anything.
    let answered = match entry.cases {
        Some(_) => HostShape::Leaf(HostLeaf::Value),
        None => match crossing(&entry.answers, Direction::Handed) {
            Ok(shape) => shape,
            Err(refusal) => return Ok(Err(under(Step::Answers, refusal))),
        },
    };
    for (shape, ty) in handed.iter().zip(&takes) {
        holds(shape, ty)?;
        crossings.need(entry.module, shape);
    }
    holds(&answered, &entry.answers)?;
    crossings.need(entry.module, &answered);
    // What a behavior was constructed with, which a host hands first, as the behavior's symbol
    // takes it.
    let mut parameters_taken: Vec<HostParameter> = Vec::new();
    if entry.constructed {
        parameters_taken.push(HostParameter::Given(HostWord::Requirements));
    }
    parameters_taken.extend(parameters(&handed, HostParameter::Given));
    parameters_taken.extend(parameters(
        std::slice::from_ref(&answered),
        HostParameter::Room,
    ));
    let function = HostFunction {
        symbol,
        takes: parameters_taken,
        answers: Some(HostWord::Status),
    };
    let runs = entry.runs;
    let constructed = entry.constructed;
    let allocate = emitting.allocate;
    let exposed = expose(emitting, function, &mut |builder, module, params| {
        let mut given = params.iter().copied();
        let mut arguments = Vec::with_capacity(handed.len() + 2);
        if constructed {
            arguments.push(
                given
                    .next()
                    .expect("what the behavior was constructed with"),
            );
        }
        let mut making = Making { module, allocate };
        for shape in &handed {
            arguments.push(take(builder, &mut making, shape, &mut given));
        }
        let rooms: Vec<ir::Value> = given.collect();
        let reaching = making.module.declare_func_in_func(runs, builder.func);
        let status = answer_into(builder, &answered, answers, &rooms, |builder, out| {
            arguments.push(out);
            let called = builder.ins().call(reaching, &arguments);
            builder.inst_results(called)[0]
        });
        builder.ins().return_(&[status]);
        Ok(())
    })?;
    Ok(Ok(HostCall {
        function: exposed,
        takes: handed,
        answers: answered,
    }))
}

/// A call made by `calling`, handed room for one slot to write its answer through, and the answer,
/// held as `held`, written through `rooms` as `answered` says only once the call has answered one.
/// Answers the call's status.
fn answer_into(
    builder: &mut FunctionBuilder,
    answered: &HostShape,
    held: types::Type,
    rooms: &[ir::Value],
    calling: impl FnOnce(&mut FunctionBuilder, ir::Value) -> ir::Value,
) -> ir::Value {
    let out = out_slot(builder);
    let status = calling(builder, out);
    let there = builder.create_block();
    let done = builder.create_block();
    let is_answered = builder
        .ins()
        .icmp_imm_s(IntCC::Equal, status, i64::from(ANSWERED));
    builder.ins().brif(is_answered, there, &[], done, &[]);

    builder.switch_to_block(there);
    let value = builder.ins().load(held, TRUSTED, out, 0);
    write(builder, answered, value, rooms);
    builder.ins().jump(done, &[]);

    builder.switch_to_block(done);
    status
}

/// A behavior a module of this object declares with no body, which a host implements: what each
/// parameter arrives as and the answer leaves as.
pub(crate) struct Injected<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub inputs: &'a [BoundaryInput],
    /// The names the declaration gives `inputs`, which a behavior a host implements always has.
    pub names: &'a [String],
    pub output: &'a BoundaryOutput,
}

/// Defines, for each behavior in `injected`, what a host makes a capability of an implementation
/// of its own through, and the code that capability holds, and puts them on `surface`.
///
/// The code reads the host's function and what it is handed first out of what the host laid out,
/// and calls it through [`call_hosted`], which holds what it answered.
///
/// Refused where a host cannot be handed what the behavior takes or hand over what it answers: a
/// host is the only thing that answers one, so a behavior no host can answer is one nothing can.
pub(crate) fn define_injections(
    emitting: &mut Emitting,
    surface: &mut Surface,
    crossings: &mut Crossings,
    injected: &[Injected],
) -> Lowered<()> {
    for behavior in injected {
        let spelt = format!("{}.{}", behavior.module, behavior.name);
        let takes: Vec<Ty> = behavior.inputs.iter().map(BoundaryInput::ty).collect();
        let answers = behavior.output.ty();
        let refused = |refusal: Refusal| {
            not_lowered(format!(
                "the injected behavior {spelt}, which a host cannot answer: {refusal}"
            ))
        };
        let handed = takes
            .iter()
            .enumerate()
            .map(|(at, ty)| {
                crossing(ty, Direction::Handed).map_err(|it| under(Step::Takes(at), it))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(refused)?;
        let answered =
            crossing(&answers, Direction::Given).map_err(|it| refused(under(Step::Answers, it)))?;
        for (shape, ty) in handed.iter().zip(&takes) {
            holds(shape, ty)?;
            crossings.need(behavior.module, shape);
        }
        holds(&answered, &answers)?;
        crossings.need(behavior.module, &answered);
        let implementation = hosted(
            host_implementation_type(behavior.module, behavior.name),
            &handed,
            &answered,
        );

        let signature = super::behavior_signature(&takes, &answers, emitting.call_conv)?;
        let code = accepted(emitting.module.declare_function(
            &format!("$hosted${spelt}"),
            Linkage::Local,
            &signature,
        ));
        let calling = implementation.signature(emitting.call_conv);
        let allocate = emitting.allocate;
        emitting.function(code, signature, |builder, module, given| {
            let hosted = given[0];
            let (arguments, out) = given[1..].split_at(given.len() - 2);
            let out = out[0];
            // A capability laid out with no room for what it calls is one made of nothing.
            let laid = builder.create_block();
            let unbound = builder.create_block();
            builder.ins().brif(hosted, laid, &[], unbound, &[]);
            builder.switch_to_block(unbound);
            let status = builder
                .ins()
                .iconst(types::I32, i64::from(INJECTION_UNBOUND));
            builder.ins().return_(&[status]);

            builder.switch_to_block(laid);
            let mut making = Making { module, allocate };
            call_hosted(
                builder,
                &mut making,
                hosted,
                &handed,
                &answered,
                arguments,
                out,
                calling,
            );
            Ok(())
        })?;

        // `(into, hosted, implementation, userdata)`: the host's function and what it is handed
        // laid out in `hosted`, and a capability of the code above over `hosted` written into
        // `into`.
        let implement = host_implement_symbol(behavior.module, behavior.name);
        let mut implementing = ir::Signature::new(emitting.call_conv);
        for _ in 0..4 {
            implementing.params.push(ir::AbiParam::new(POINTER));
        }
        let id = accepted(emitting.module.declare_function(
            &implement,
            Linkage::Export,
            &implementing,
        ));
        emitting.function(id, implementing, |builder, module, given| {
            let [into, hosted, implemented, userdata] = given else {
                unreachable!("a capability is made of room for it and for what a host wrote");
            };
            lay_out_hosted(builder, *hosted, *implemented, *userdata);
            let code = module.declare_func_in_func(code, builder.func);
            let code = builder.ins().func_addr(POINTER, code);
            builder
                .ins()
                .store(TRUSTED, code, *into, CAPABILITY_INVOKE as i32);
            builder
                .ins()
                .store(TRUSTED, *hosted, *into, CAPABILITY_ENVIRONMENT as i32);
            builder.ins().return_(&[]);
            Ok(())
        })?;

        surface.injection(
            behavior.module,
            behavior.name,
            behavior.names,
            &takes,
            &answers,
            emitting.declared,
            (&handed, &answered),
            &implementation,
            &implement,
        );
    }
    Ok(())
}

/// The function a host writes to answer what takes `takes` and answers `answers`, named `type_name`:
/// handed what it was handed first where it was laid out, then each of `takes` as a host is handed
/// it, then room for each word of what it answers, answering a status.
fn hosted(type_name: String, takes: &[HostShape], answers: &HostShape) -> HostImplementation {
    let mut given = vec![HostParameter::Given(HostWord::Userdata)];
    given.extend(parameters(takes, HostParameter::Given));
    given.extend(parameters(
        std::slice::from_ref(answers),
        HostParameter::Room,
    ));
    HostImplementation {
        type_name,
        takes: given,
        answers: HostWord::Status,
    }
}

/// Lays out, in the room a host handed at `hosted`, the function it wrote and what that function is
/// handed first, as [`HOSTED_IMPLEMENTATION`] and [`HOSTED_USERDATA`] say.
fn lay_out_hosted(
    builder: &mut FunctionBuilder,
    hosted: ir::Value,
    implemented: ir::Value,
    userdata: ir::Value,
) {
    builder
        .ins()
        .store(TRUSTED, implemented, hosted, HOSTED_IMPLEMENTATION as i32);
    builder
        .ins()
        .store(TRUSTED, userdata, hosted, HOSTED_USERDATA as i32);
}

/// Calls what a host wrote to answer a call, out of the room it laid it out in at `hosted`, with
/// `arguments`, each held as the generated code holds a value crossing in its shape among `takes`,
/// and writes what it answered through `out` as the generated code holds it; answers the status the
/// call answers.
///
/// The host's function is handed what it was handed first where it was laid out, then each argument
/// as the words a host is handed it in ([`words_of`]), then room for each word of its answer, which
/// is its own room and not `out`: `out` is written only once the status says there is an answer,
/// and then with what the words it wrote are taken into ([`take`]).
///
/// What it answers is held to what an implementation may answer ([`IMPLEMENTATION_ANSWERS`]): handed
/// on as it came where it may answer it, and the protocol broken where it may not, since that is
/// not a status this object hands a caller as if a computation of the model had come to it. The one
/// place that is held: a behavior's capability of a host's implementation and a host's own function
/// value both call through this, and a capability or a function value made any other way answers
/// what it answers.
#[allow(clippy::too_many_arguments)]
fn call_hosted(
    builder: &mut FunctionBuilder,
    making: &mut Making,
    hosted: ir::Value,
    takes: &[HostShape],
    answers: &HostShape,
    arguments: &[ir::Value],
    out: ir::Value,
    calling: ir::Signature,
) {
    let implemented = builder
        .ins()
        .load(POINTER, TRUSTED, hosted, HOSTED_IMPLEMENTATION as i32);
    let userdata = builder
        .ins()
        .load(POINTER, TRUSTED, hosted, HOSTED_USERDATA as i32);
    let bound = builder.create_block();
    let unbound = builder.create_block();
    builder.ins().brif(implemented, bound, &[], unbound, &[]);

    builder.switch_to_block(unbound);
    let status = builder
        .ins()
        .iconst(types::I32, i64::from(INJECTION_UNBOUND));
    builder.ins().return_(&[status]);

    builder.switch_to_block(bound);
    let answer_words = answers.words();
    let rooms: Vec<ir::Value> = answer_words.iter().map(|_| out_slot(builder)).collect();
    let mut handing = vec![userdata];
    for (shape, argument) in takes.iter().zip(arguments) {
        handing.extend(words_of(builder, shape, *argument));
    }
    handing.extend(&rooms);
    let calling = builder.import_signature(calling);
    let called = builder.ins().call_indirect(calling, implemented, &handing);
    let status = builder.inst_results(called)[0];

    let is_answered = builder
        .ins()
        .icmp_imm_s(IntCC::Equal, status, i64::from(ANSWERED));
    let answered = builder.create_block();
    let otherwise = builder.create_block();
    builder
        .ins()
        .brif(is_answered, answered, &[], otherwise, &[]);

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

    builder.switch_to_block(answered);
    let words: Vec<ir::Value> = answer_words
        .iter()
        .zip(&rooms)
        .map(|(word, room)| builder.ins().load(machine(*word), TRUSTED, *room, 0))
        .collect();
    let value = take(builder, making, answers, &mut words.into_iter());
    builder.ins().store(TRUSTED, value, out, 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);
}

/// Defines what a host builds and reads each list, and calls and makes each function value, in
/// `crossings` through, and puts them on `surface`.
///
/// Each takes and answers the words of the shape it is for, so an element is put in its slot and
/// taken out of it, and what a function value takes and answers is handed across, by what every
/// other function here does with a value of that shape ([`take`], [`write`], [`words_of`]).
/// Nothing here asks what a type is.
pub(crate) fn define_crossings(
    emitting: &mut Emitting,
    surface: &mut Surface,
    crossings: &Crossings,
) -> Lowered<()> {
    for (module_name, elements) in &crossings.lists {
        for element in elements {
            define_list(emitting, surface, module_name, element)?;
        }
    }
    for (module_name, functions) in &crossings.functions {
        for function in functions {
            define_function(emitting, surface, module_name, function)?;
        }
    }
    Ok(())
}

/// What a host builds and reads a list of `module_name`'s whose elements cross as `element`
/// through.
fn define_list(
    emitting: &mut Emitting,
    surface: &mut Surface,
    module_name: &str,
    element: &HostShape,
) -> Lowered<()> {
    let allocate = emitting.allocate;
    let symbol = |operation| host_list_symbol(module_name, element, operation);
    let mut takes = vec![HostParameter::Given(HostWord::Count)];
    takes.extend(parameters(
        std::slice::from_ref(element),
        HostParameter::Slice,
    ));
    let constructing = HostFunction {
        symbol: symbol(HostListOperation::Construct),
        takes,
        answers: Some(HostWord::List),
    };
    let construct = expose(emitting, constructing, &mut |builder, module, given| {
        construct(builder, &mut Making { module, allocate }, element, given);
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
    takes.extend(parameters(
        std::slice::from_ref(element),
        HostParameter::Room,
    ));
    let indexing = HostFunction {
        symbol: symbol(HostListOperation::At),
        takes,
        answers: Some(HostWord::Bool),
    };
    let at = expose(emitting, indexing, &mut |builder, _, given| {
        element_at(builder, element, given);
        Ok(())
    })?;
    surface.list(module_name, element, &construct, &length, &at);
    Ok(())
}

/// What a host calls a function value of `module_name`'s crossing in `function` through, and makes
/// one of its own through, with the code a value it makes holds.
///
/// The call takes the value and what it takes, as a host hands each over, calls the value's code
/// ([`FUNCTION_INVOKE`]) as any caller of one does, and writes what it answered through the host's
/// room. A value a host makes is room it laid out ([`souther_native_abi::room_for_hosted_function`]):
/// the code at its head reads what the host wrote beside it ([`HOSTED_FUNCTION_HOSTED`]) and calls
/// it through [`call_hosted`], as the code of a capability of a host's implementation does.
fn define_function(
    emitting: &mut Emitting,
    surface: &mut Surface,
    module_name: &str,
    function: &HostShape,
) -> Lowered<()> {
    let HostShape::Function { takes, answers } = function else {
        unreachable!("{function:?} is not how a function value crosses");
    };
    let allocate = emitting.allocate;
    let held: Vec<types::Type> = takes.iter().map(held_as).collect();
    let invocation = invocation_signature(&held, emitting.call_conv);

    let mut given = vec![HostParameter::Given(HostWord::Function)];
    given.extend(parameters(takes, HostParameter::Given));
    given.extend(parameters(
        std::slice::from_ref(answers),
        HostParameter::Room,
    ));
    let calling = HostFunction {
        symbol: host_function_symbol(module_name, function, HostFunctionOperation::Call),
        takes: given,
        answers: Some(HostWord::Status),
    };
    let signature = invocation.clone();
    let call = expose(emitting, calling, &mut |builder, module, params| {
        let (value, rest) = params.split_first().expect("the function value first");
        let mut given = rest.iter().copied();
        let mut making = Making { module, allocate };
        let mut arguments = vec![*value];
        for shape in takes {
            arguments.push(take(builder, &mut making, shape, &mut given));
        }
        let rooms: Vec<ir::Value> = given.collect();
        let calling = builder.import_signature(signature.clone());
        let code = builder
            .ins()
            .load(POINTER, TRUSTED, *value, FUNCTION_INVOKE as i32);
        let status = answer_into(
            builder,
            answers,
            held_as(answers),
            &rooms,
            |builder, out| {
                arguments.push(out);
                let called = builder.ins().call_indirect(calling, code, &arguments);
                builder.inst_results(called)[0]
            },
        );
        builder.ins().return_(&[status]);
        Ok(())
    })?;

    let implementation = hosted(
        host_function_symbol(module_name, function, HostFunctionOperation::Implementation),
        takes,
        answers,
    );
    let implement = host_function_symbol(module_name, function, HostFunctionOperation::Implement);
    let code = accepted(emitting.module.declare_function(
        &format!("$hosted${implement}"),
        Linkage::Local,
        &invocation,
    ));
    let calling = implementation.signature(emitting.call_conv);
    emitting.function(code, invocation, |builder, module, given| {
        let (value, rest) = given.split_first().expect("the function value first");
        let (arguments, out) = rest.split_at(rest.len() - 1);
        let hosted = builder.ins().iadd_imm_s(*value, HOSTED_FUNCTION_HOSTED);
        let mut making = Making { module, allocate };
        call_hosted(
            builder,
            &mut making,
            hosted,
            takes,
            answers,
            arguments,
            out[0],
            calling,
        );
        Ok(())
    })?;

    // `(into, implementation, userdata) -> function`: the code above at the head of `into`, and the
    // host's function and what it is handed beside it, `into` being the value.
    let mut implementing = ir::Signature::new(emitting.call_conv);
    for _ in 0..3 {
        implementing.params.push(ir::AbiParam::new(POINTER));
    }
    implementing.returns.push(ir::AbiParam::new(POINTER));
    let id = accepted(
        emitting
            .module
            .declare_function(&implement, Linkage::Export, &implementing),
    );
    emitting.function(id, implementing, |builder, module, given| {
        let [into, implemented, userdata] = given else {
            unreachable!("a function value is made of room for it and what a host wrote");
        };
        let hosted = builder.ins().iadd_imm_s(*into, HOSTED_FUNCTION_HOSTED);
        lay_out_hosted(builder, hosted, *implemented, *userdata);
        let code = module.declare_func_in_func(code, builder.func);
        let code = builder.ins().func_addr(POINTER, code);
        builder
            .ins()
            .store(TRUSTED, code, *into, FUNCTION_INVOKE as i32);
        builder.ins().return_(&[*into]);
        Ok(())
    })?;

    surface.function(module_name, function, &call, &implementation, &implement);
    Ok(())
}

/// The most elements a list can be built with: the most whose room a count of bytes can say.
const MOST_ELEMENTS: i64 = (i64::MAX - room_for_list(0)) / SLOT;

/// A host's constructor of a list: room for the count it was handed, and each element, taken from
/// what each column holds at its index, put in its slot.
///
/// A column holds one word for each element, as many bytes apart as the word is wide. A count below
/// nought, or past what room can be taken for, is a host handing over something no list is; it
/// traps rather than being read as some count a list could have, which would write past the room.
fn construct(
    builder: &mut FunctionBuilder,
    making: &mut Making,
    element: &HostShape,
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
    let taking = making
        .module
        .declare_func_in_func(making.allocate, builder.func);
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
    let value = take(builder, making, element, &mut words.into_iter());
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
/// where the index is inside the list, and nought, with nothing written, where it is not. Whether
/// the index is inside is not a value of the model, and is answered as the function's return.
///
/// An index is inside where it is below the length read without a sign, the way `List.get` reads
/// one, so a negative one is outside as well.
fn element_at(builder: &mut FunctionBuilder, element: &HostShape, given: &[ir::Value]) {
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
    let value = out_of_slot(builder, slot, held_as(element));
    write(builder, element, value, rooms);
    let yes = builder.ins().iconst(types::I8, 1);
    builder.ins().return_(&[yes]);
}

/// A host's decoder: the bytes read as a document, the document read as a value of `key` by the
/// type's reader, and the reading handed to the host, which asks it what it came to.
///
/// `begin` is what the bytes are read as: text in the external form ([`Runtime::DecodeBegin`]), or
/// a value a host built of ordered maps ([`Runtime::DecodeHostBegin`]). The type's reader is the
/// same either way, and so is everything after the reading begins.
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
    begin: Runtime,
) {
    let [bytes, length, out] = given else {
        unreachable!("a decoder takes bytes, how many, and room for the reading")
    };
    assert!(
        matches!(begin, Runtime::DecodeBegin | Runtime::DecodeHostBegin),
        "a reading begins with one of the two ways a document is read"
    );
    let begin = codecs.runtime(module, begin);
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

/// A host's constructor: what it was handed, taken into what the declaration's own constructor
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
    handed: &[HostShape],
    params: &[ir::Value],
) {
    let mut given = params.iter().copied();
    let mut making = Making { module, allocate };
    let mut fields = Vec::with_capacity(handed.len() + 1);
    for shape in handed {
        fields.push(take(builder, &mut making, shape, &mut given));
    }
    let out = given.next().expect("room for the answer after the fields");
    fields.push(out);
    let reaching = making
        .module
        .declare_func_in_func(constructor, builder.func);
    let called = builder.ins().call(reaching, &fields);
    let status = builder.inst_results(called)[0];
    builder.ins().return_(&[status]);
}

/// A host's reader of the field at `at`: the field written through the host's room, as every value
/// a host is handed is.
fn read(builder: &mut FunctionBuilder, at: usize, shape: &HostShape, given: &[ir::Value]) {
    let (owner, rooms) = given.split_first().expect("the value a field is read off");
    let slot = builder
        .ins()
        .load(types::I64, TRUSTED, *owner, field_at(at) as i32);
    let value = out_of_slot(builder, slot, held_as(shape));
    write(builder, shape, value, rooms);
    builder.ins().return_(&[]);
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
/// A case is told by its token whatever kind it is: a declaration's, or the runtime's for a
/// primitive or a case the language gives.
fn which_case(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    declared: &Declared,
    cases: &[Case],
    value: Tagged,
) -> Lowered<()> {
    let which = value.which(builder);
    for (place, case) in cases.iter().enumerate() {
        let expected = token_of(builder, declared, module, case)?;
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

#[cfg(test)]
mod tests {
    use super::{Direction, crossing};
    use crate::manifest::{Reason, Refusal, Step};
    use crate::transport::{Bottom, Case, Cases, FnSignature, LanguageCase, MapTy, Prim, Ty};
    use souther_native_abi::{HostLeaf, HostShape};

    fn int() -> Ty {
        Ty::Prim { prim: Prim::Int }
    }

    fn optional(of: Ty) -> Ty {
        Ty::Option {
            option: Box::new(of),
        }
    }

    fn function(takes: Vec<Ty>, answers: Ty) -> Ty {
        Ty::Fn {
            fn_: FnSignature {
                takes,
                answers: Box::new(answers),
            },
        }
    }

    /// `Int | DivisionByZero`, which says which case it is by its token and nothing a host is
    /// handed.
    fn union() -> Ty {
        Ty::Union {
            union: Cases::one_or_more(vec![
                Case::Primitive { prim: Prim::Int },
                Case::Language {
                    case: LanguageCase::DivisionByZero,
                },
            ])
            .unwrap(),
        }
    }

    fn refused(reason: Reason, path: Vec<Step>) -> Result<HostShape, Refusal> {
        Err(Refusal { reason, path })
    }

    /// A refusal says its reason and where it stands in words, as a refused build is told.
    #[test]
    fn a_refusal_says_why_and_where_in_words() {
        let refusal = crossing(
            &Ty::Tuple {
                tuple: vec![
                    int(),
                    optional(Ty::Prim {
                        prim: Prim::Decimal,
                    }),
                ],
            },
            Direction::Given,
        )
        .unwrap_err();

        assert_eq!(
            refusal.to_string(),
            "no representation for a host, at the member at 1, what an optional holds"
        );
    }

    /// An optional is a presence for each depth, and a tuple its members' words one after another,
    /// however deep either stands.
    #[test]
    fn an_optional_at_any_depth_and_a_tuple_cross_as_what_they_hold() {
        let whole = HostShape::Leaf(HostLeaf::Int);
        let shape = crossing(
            &optional(Ty::Tuple {
                tuple: vec![optional(optional(Ty::Prim { prim: Prim::Bool })), int()],
            }),
            Direction::Handed,
        )
        .unwrap();

        assert_eq!(
            shape,
            HostShape::Option(Box::new(HostShape::Product(vec![
                HostShape::Option(Box::new(HostShape::Option(Box::new(HostShape::Leaf(
                    HostLeaf::Bool
                ))))),
                whole,
            ])))
        );
        assert_eq!(shape.words().len(), 5);
    }

    /// A union is handed over as the value it is, and handed to a host nowhere but a behavior's
    /// answer, which is not this. What a function value takes crosses the other way round from
    /// the value, so a host handed a function hands a union over to it, and one handed a function
    /// answering a union has nothing to say which case the answer is.
    #[test]
    fn a_union_crosses_only_where_a_host_hands_it_over() {
        assert_eq!(
            crossing(&union(), Direction::Given),
            Ok(HostShape::Leaf(HostLeaf::Value))
        );
        assert_eq!(
            crossing(&union(), Direction::Handed),
            refused(Reason::NoDiscriminator, vec![])
        );
        assert!(crossing(&function(vec![union()], int()), Direction::Handed).is_ok());
        assert_eq!(
            crossing(
                &Ty::List {
                    list: Box::new(function(vec![int()], optional(union())))
                },
                Direction::Handed
            ),
            refused(
                Reason::NoDiscriminator,
                vec![Step::Element, Step::Answers, Step::Option]
            )
        );
        assert_eq!(
            crossing(&function(vec![int(), union()], int()), Direction::Given),
            refused(Reason::NoDiscriminator, vec![Step::Takes(1)])
        );
    }

    /// What has no representation for a host, and what has no value, are refused where they
    /// stand, and the path says how deep that is.
    #[test]
    fn what_does_not_cross_is_refused_where_it_stands() {
        assert_eq!(
            crossing(
                &Ty::Tuple {
                    tuple: vec![
                        int(),
                        Ty::Prim {
                            prim: Prim::Decimal
                        }
                    ]
                },
                Direction::Given
            ),
            refused(Reason::NoRepresentation, vec![Step::Member(1)])
        );
        assert_eq!(
            crossing(
                &Ty::Map {
                    map: MapTy {
                        key: Box::new(int()),
                        value: Box::new(int())
                    }
                },
                Direction::Handed
            ),
            refused(Reason::NoRepresentation, vec![])
        );
        assert_eq!(
            crossing(
                &Ty::List {
                    list: Box::new(Ty::Nothing { nothing: Bottom {} })
                },
                Direction::Handed
            ),
            refused(Reason::NoValue, vec![Step::Element])
        );
    }
}
