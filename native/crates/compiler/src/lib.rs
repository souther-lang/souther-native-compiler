//! Lowers what crossed to Cranelift IR, and lets Cranelift write the object.
//!
//! The object is for the machine this runs on. Choosing a target for another machine is a question
//! about linkers and a runtime built for it, and answering that before the code generation works
//! would be answering the easier question first.

mod boundary;
mod closures;
pub mod transport;

use anyhow::{Result, anyhow, bail};
use closures::{ClosureSites, Site};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{
    AbiParam, Function, InstBuilder, MemFlagsData, TrapCode, UserFuncName, types,
};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::codegen::settings::{self, Configurable};
use cranelift::codegen::{Context, ir};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift::module::{DataDescription, DataId, FuncId, Linkage, Module, default_libcall_names};
use cranelift::object::{ObjectBuilder, ObjectModule};
use souther_native_abi::{
    ALLOCATE, ANSWERED, HELD, NOTHING, SLOT, STRING_COMPARE, STRING_CONCAT, Status, TEXT_BYTES,
    TEXT_LENGTH, TOKEN, WHICH, behavior_symbol, boundary_symbol, example_symbol, field_at,
    held_symbol, member_at, room_for_fields, room_for_held, room_for_members, room_for_text,
    type_symbol, value_symbol,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use transport::{
    AbortKind, AlternativesForm, Answers, Arm, Case, CodecShape, Declaration, DeclaredBy,
    Definition, Node, Op, Prim, Program, Publication, Reaches, Routing, Selects, Stage,
    TRANSPORT_VERSION, Target, Ty,
};

/// A fork that ran out of arms, which is this compiler having emitted the wrong test rather than
/// anything a program can be written as: the checker settles that a fork always answers. Still a
/// trap and not a status: this is this compiler's own invariant failing, not a Souther computation
/// ending without a value, and the two are told apart by which channel answers for them.
const NO_ARM: u8 = 2;

/// Every reason a Souther computation ends without a value, mapped to the wire number a generated
/// function's status answers with — `souther_native_abi::ANSWERED` reserves zero, so every member
/// here gets one of what is left.
///
/// No default arm, for the reason `KernelContracts::abortsOf` on the Java side has none: a member
/// `AbortKind` adds and this does not answer for is a mapping nobody wrote rather than one that
/// silently agrees with the last one written for something else. What number a member gets is a
/// decision of this crate's alone — the `abi` crate states the wire's width and its one reserved
/// value and nothing about what any other value of it means.
///
/// `pub`, and not only for `lower`'s own sake: `Running`'s Java test harness reads a status this
/// answers back off a compiled run and has to turn it back into the `AbortKind` it came from to
/// assert anything about it, which means it holds a second, hand-written copy of this same table.
/// `tests/abort_status.rs` is what keeps the two from drifting apart unnoticed — the same role
/// `vocabularies.rs` plays for `Op`, `Prim` and the rest, and the same reason: a member spelt
/// (here, numbered) differently on the two sides reads without complaint and means something
/// other than what either side thinks it does.
pub fn native_status(kind: AbortKind) -> Status {
    match kind {
        AbortKind::InvariantNotHeld => 1,
        AbortKind::EnsuresNotHeld => 2,
        AbortKind::UnreachableReached => 3,
        AbortKind::DivisionByZero => 4,
        AbortKind::RequiredFormHasNoPlace => 5,
        AbortKind::InvalidBounds => 6,
    }
}

/// The one status an arithmetic site that may leave the range its type holds jumps to the abort
/// block with.
///
/// Read off the site's own `aborts` — `program.abortsAt(site)`'s answer, carried on the `Node` —
/// rather than assumed from which operator or which kernel this is: what a machine condition here
/// means is a fact `CheckedProgram` already settled, and asking the transport for it instead of
/// deciding it again here is the one thing issue #9 exists to change.
///
/// An arithmetic site the checker gave zero or more than one reason for is refused rather than
/// trusted blindly: a machine condition this backend can fire and a checker answer of zero
/// reasons for it are the two halves disagreeing about what kind of site this is, and answering a
/// wrong value because the checker said `NONE` would be worse than refusing the program.
/// `Core.Neg` is such a site today (souther-lang/souther#1878) — its own caller in `lower` checks
/// `aborts` before this is reached and refuses with `NotLowered` rather than asking this to fabricate
/// a status for an empty answer.
fn overflow_status(aborts: &[AbortKind]) -> Result<Status> {
    match aborts {
        [only] => Ok(native_status(*only)),
        _ => bail!(
            "an arithmetic site that may leave its type's range names {} reasons for ending \
             without a value, and this backend answers only where there is exactly one",
            aborts.len()
        ),
    }
}

/// Something the language admits and this driver does not lower yet.
///
/// Its own type because the half that started this has to tell it from a document it could not
/// read: one says the program is ahead of this backend, the other that the two halves disagree
/// about what they are saying to each other, and a caller that heard one word for both would
/// report a compiler's own fault as the author's.
#[derive(Debug)]
pub struct NotLowered(pub String);

impl fmt::Display for NotLowered {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

impl std::error::Error for NotLowered {}

/// How the driver answers, for the half that started it.
///
/// A process says one number and the reader of it is in another language, so this is a contract
/// with a party the compiler cannot check. What holds the two together is a test that runs the
/// whole way through rather than each side's reading of this comment.
pub mod ended {
    /// The object is on stdout.
    pub const WITH_AN_OBJECT: u8 = 0;

    /// Something went wrong here: a document this driver could not read, or a machine it could not
    /// write for. Not the program's author's to fix.
    pub const BADLY: u8 = 1;

    /// The program is one the language admits and this backend does not write yet. What it was is
    /// on stderr.
    pub const NOT_LOWERED: u8 = 2;
}

fn not_lowered(what: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(NotLowered(what.into()))
}

/// The object holding every behavior the document carries.
pub fn object_for(document: &str) -> Result<Vec<u8>> {
    let program: Program = serde_json::from_str(document)?;
    if program.transport != TRANSPORT_VERSION {
        bail!(
            "this driver reads transport {TRANSPORT_VERSION} and was handed {}",
            program.transport
        );
    }

    let mut flags = settings::builder();
    // A call out of this object reaches its callee the way the platform's linker expects, which on
    // both of the hosts this runs on means position-independent. Said here rather than left to the
    // host, so that what is emitted is the same wherever it is built.
    flags.set("is_pic", "true")?;
    let isa = cranelift::native::builder()
        .map_err(|it| anyhow!("no code generator for this host: {it}"))?
        .finish(settings::Flags::new(flags))?;

    let builder = ObjectBuilder::new(isa, "souther", default_libcall_names())?;
    let mut module = ObjectModule::new(builder);
    let mut context = Context::new();
    let mut shapes = FunctionBuilderContext::new();
    let frontend = module.isa().frontend_config();
    let call_conv = module.isa().default_call_conv();

    let mut taking_room = ir::Signature::new(call_conv);
    taking_room.params.push(AbiParam::new(types::I64));
    taking_room.returns.push(AbiParam::new(POINTER));
    let allocate = module.declare_function(ALLOCATE, Linkage::Import, &taking_room)?;

    // What two strings are compared and joined through. Neither is emitted here: a comparison of
    // text is a walk over two runs of bytes, and one written into every site that says `==` would
    // be the same walk written as many times as the program says it.
    let mut over_two_strings = ir::Signature::new(call_conv);
    over_two_strings.params.push(AbiParam::new(POINTER));
    over_two_strings.params.push(AbiParam::new(POINTER));
    let mut comparing = over_two_strings.clone();
    comparing.returns.push(AbiParam::new(types::I64));
    let compare_text = module.declare_function(STRING_COMPARE, Linkage::Import, &comparing)?;
    let mut joining = over_two_strings;
    joining.returns.push(AbiParam::new(POINTER));
    let join_text = module.declare_function(STRING_CONCAT, Linkage::Import, &joining)?;

    let declared = Declared::of(&program.declarations)?;
    let literals = Literals::default();

    // The token every declaration at home in this object is tagged by, defined whether anything
    // here builds a value of one or not. A declaration has one home and it is the object of the
    // build that checked its module, so what another build's object names is resolved here or
    // nowhere.
    //
    // A sum is skipped because nothing is ever tagged with one: an arm tests the leaves a case
    // resolved to, and a token nothing is tagged with would be a name in the table standing for a
    // value that cannot exist.
    let mut token = DataDescription::new();
    token.define(TOKEN.into());
    for declaration in &program.declarations {
        if matches!(declaration, Declaration::Sum { .. }) || declaration.by() != DeclaredBy::AModule
        {
            continue;
        }
        let id = declared.tag(&mut module, &declaration.key())?;
        module.define_data(id, &token)?;
    }

    // Every behavior the document names, by the key a reference to it says. Built once so a
    // lookup here is one hash rather than a walk over every target the document carries — which
    // repeating for every local definition, and now for every composition's every stage, would
    // make quadratic in nothing this document did.
    let targets = Targets::of(&program.behaviors);
    for target in &program.behaviors {
        if let transport::BoundaryOutput::Cases { ty, cases, form } = &target.output {
            declared.settled(&target.declared(), cases, form)?;
            declared.descends_to(&target.declared(), ty, cases)?;
        }
    }

    // Every closure site the document holds, found once over the whole program, and the lifted
    // function declared for each — before any body is defined, the same two-phase shape every
    // other declaration here keeps. A site nested inside one body may be referenced from another
    // (a closure returned from one function and applied by another), so nothing about defining a
    // body may assume every site it itself needs was already declared by the time it runs; all of
    // them are, because this runs before any of them does.
    let closures = ClosureSites::of_program(&program)?;
    let mut lifted: BTreeMap<usize, FuncId> = BTreeMap::new();
    for (&site, plan) in closures.iter() {
        let fn_ = plan.signature;
        let signature = lifted_signature(&fn_.takes, &fn_.answers, call_conv)?;
        let symbol = format!("$closure${site}");
        let id = module.declare_function(&symbol, Linkage::Local, &signature)?;
        lifted.insert(site, id);
    }

    // Every local definition this object holds, by the name it defines — not only what the
    // module declaring it says about the name, but the definition itself, because what a target
    // says a name answers with and what its local definition actually is are two readings of one
    // fact once `Composed` is a local definition too, and this driver reads a document strictly:
    // the two are checked against each other below rather than one of them read on trust.
    let mut locals: HashMap<&str, &Definition> = HashMap::new();
    for written in &program.modules {
        for definition in &written.definitions {
            locals.insert(definition.declared(), definition);
        }
    }

    // Every local definition this object holds agrees with what its own target says — checked
    // once, exhaustively, from the local definition's side. The declaration loop below checks
    // the other direction — that a target answering `Body` or `Composed` has a local definition
    // at all — which is a different question: existence, not kind. Answered from this side and
    // not folded into that loop, because that loop only ever visits a target whose `is` is
    // already `Body` or `Composed`; a target answering `Injected`, `Elsewhere` or `Unwritten`
    // that nonetheless has a local definition sitting under its name — the two halves disagreeing
    // about the one thing that matters most, whether this object defines the name at all — would
    // never reach it.
    for (&name, &local) in &locals {
        let target = targets.named(name)?;
        agrees_with_its_target(name, target, local, &targets, &declared)?;
    }

    // Every function is declared before any is defined, because a body may reach one written
    // after it — a definition that calls itself reaches itself, and two that call each other
    // reach one another. Nothing here orders the program to make that go away.
    let mut reachable = Reachable::default();
    // The row entries, which nothing reaches: they are what the object offers to whoever runs its
    // rows, so they are held by their symbol rather than beside what a call can reach.
    let mut entries: HashMap<String, FuncId> = HashMap::new();
    for target in &program.behaviors {
        let symbol = behavior_symbol(&target.module, &target.name);
        let signature = signature_over(&target.takes(), &target.answers(), call_conv)?;
        let linkage = match target.is {
            // Defined here, so what the table carries for it is this object's answer about a name
            // the declaring module has already decided. A body or a composition with no such
            // answer is a local definition of a module this document does not carry, which is the
            // two halves disagreeing rather than something to fall back from. That the definition
            // found, if any, is the kind of definition this target says was already checked above.
            Answers::Body | Answers::Composed => {
                let declared = target.declared();
                let local = locals.get(declared.as_str()).copied().ok_or_else(|| {
                    anyhow!(
                        "{declared} answers with a local definition no module of this document \
                         carries"
                    )
                })?;
                linkage_of(local.publication())
            }
            // Named and not defined. What answers it is settled where the object is linked, and
            // the two reasons a body is absent are one call to whoever reaches in.
            Answers::Injected | Answers::Elsewhere => {
                crosses_objects(target)?;
                Linkage::Import
            }
            Answers::Unwritten => {
                return Err(not_lowered(format!(
                    "the unwritten behavior {}",
                    target.declared()
                )));
            }
        };
        let id = module.declare_function(&symbol, linkage, &signature)?;
        reachable.behavior(&target.declared(), id)?;
    }
    for written in &program.modules {
        // Named out in full, and not `..`'d away, so a field `transport::Module` starts carrying
        // tomorrow is a compile error at this one destructure until it is given a home below.
        // `Module::entries` (a module's own published-value entries) is bound to `value_entries`
        // rather than `entries`, which stays free for this loop's own row-entry table below.
        let transport::Module {
            name,
            helpers,
            values,
            entries: value_entries,
            definitions: _,
            examples,
        } = written;
        for held in helpers {
            let symbol = held_symbol(name, &held.declared);
            let signature = signature_over(&held.takes, &held.answers, call_conv)?;
            // Held and not exported: a definition a module holds is that module's copy, and
            // nothing outside the object reaches one.
            let id = module.declare_function(&symbol, Linkage::Local, &signature)?;
            reachable.held(name, &held.declared, id)?;
        }
        for value in values {
            // As private as a helper's method, and named the same way: a value's home is this
            // module's own business (ADR-0074) — nothing outside this object reaches it directly,
            // whether outside this object's other modules or another object altogether. A caller
            // elsewhere goes through the entry declared below instead.
            let takes = handover_types(value);
            let symbol = held_symbol(name, &value.declared());
            let signature = signature_over(&takes, &value.answers, call_conv)?;
            let id = module.declare_function(&symbol, Linkage::Local, &signature)?;
            reachable.value(name, &value.declared(), id)?;
        }
        for entry in value_entries {
            // Exported under value_symbol, which is the one thing about a value ever addressed
            // from outside the module that declares it: the entry takes nothing at the language
            // level (ADR-0074), and its answer is read off its own body rather than the value's —
            // the two agree by the invariant CheckedModule already holds, and re-deriving one from
            // the other here would be the checker's decision read a second time.
            let signature = signature_over(&[], entry.body.ty(), call_conv)?;
            let symbol = value_symbol(&entry.value.module, &entry.value.name);
            let id = module.declare_function(&symbol, Linkage::Export, &signature)?;
            reachable.published_value(&entry.value.module, &entry.value.name, id)?;
        }
        for example in examples {
            let signature = running_a_row(&targets, name, &example.behavior, call_conv)?;
            let symbol = example_symbol(name, &example.behavior, example.at);
            // Reached from outside whatever the module says about the behavior's own name: what
            // this runs is a row, and a row of a kept name is as much a row as any other.
            let id = module.declare_function(&symbol, Linkage::Export, &signature)?;
            entries.insert(symbol, id);
        }
    }
    // Every published value a call anywhere in this document reaches, and the answer type its
    // call sites carry — read here rather than at the call site during lowering, because a value
    // this program does not itself declare an entry for still needs a symbol declared before any
    // function that calls it is defined, the same two-phase shape every other declaration in this
    // file keeps. A value this program does declare an entry for was just given one above, so
    // only a genuinely foreign one reaches this loop.
    for ((module_name, value_name), ty) in published_value_calls(&program)? {
        if reachable.is_published(&module_name, &value_name) {
            continue;
        }
        // The same boundary a behavior answering from another object crosses (`crosses_objects`),
        // and held to the same condition: an entry is an ordinary call across an object boundary,
        // and a value this backend would refuse a behavior for answering does not become
        // reachable just because a value happened to answer it instead.
        crosses_object(
            &format!("`{module_name}`'s published value {value_name} answers"),
            &ty,
        )?;
        let signature = signature_over(&[], &ty, call_conv)?;
        let symbol = value_symbol(&module_name, &value_name);
        let id = module.declare_function(&symbol, Linkage::Import, &signature)?;
        reachable.published_value(&module_name, &value_name, id)?;
    }

    for written in &program.modules {
        let transport::Module {
            name,
            helpers,
            values,
            entries: value_entries,
            definitions,
            examples,
        } = written;
        for held in helpers {
            let signature = signature_over(&held.takes, &held.answers, call_conv)?;
            let id = reachable.of_held(name, &held.declared)?;
            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            let lowering = Lowering {
                declared: &declared,
                reachable: &reachable,
                carrier: name,
                allocate,
                compare_text,
                join_text,
                closures: &closures,
                lifted: &lifted,
                literals: &literals,
            };
            define(
                &mut context.func,
                &mut shapes,
                &held.takes,
                &held.body,
                frontend,
                &lowering,
                &mut module,
            )?;
            module.define_function(id, &mut context)?;
        }
        for value in values {
            let takes = handover_types(value);
            let signature = signature_over(&takes, &value.answers, call_conv)?;
            let id = reachable.of_value(name, &value.declared())?;
            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            let lowering = Lowering {
                declared: &declared,
                reachable: &reachable,
                carrier: name,
                allocate,
                compare_text,
                join_text,
                closures: &closures,
                lifted: &lifted,
                literals: &literals,
            };
            define(
                &mut context.func,
                &mut shapes,
                &takes,
                &value.body,
                frontend,
                &lowering,
                &mut module,
            )?;
            module.define_function(id, &mut context)?;
        }
        for entry in value_entries {
            let signature = signature_over(&[], entry.body.ty(), call_conv)?;
            let id = reachable.of_published_value(&entry.value.module, &entry.value.name)?;
            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            let lowering = Lowering {
                declared: &declared,
                reachable: &reachable,
                carrier: name,
                allocate,
                compare_text,
                join_text,
                closures: &closures,
                lifted: &lifted,
                literals: &literals,
            };
            // Taking nothing, the same as a row's entry: what a value needs is handed over inside
            // its own body (`Reaches::Value`, threading each handover), never by a caller of this
            // entry.
            define(
                &mut context.func,
                &mut shapes,
                &[],
                &entry.body,
                frontend,
                &lowering,
                &mut module,
            )?;
            module.define_function(id, &mut context)?;
        }
        for local in definitions {
            match local {
                Definition::Body {
                    declared: behavior_name,
                    body,
                    ..
                } => {
                    let target = targets.named(behavior_name)?;
                    let takes = &target.takes();
                    let signature = signature_over(&target.takes(), &target.answers(), call_conv)?;
                    let id = reachable.of_behavior_named(behavior_name)?;
                    context.clear();
                    context.func =
                        Function::with_name_signature(UserFuncName::default(), signature);
                    let lowering = Lowering {
                        declared: &declared,
                        reachable: &reachable,
                        carrier: name,
                        allocate,
                        compare_text,
                        join_text,
                        closures: &closures,
                        lifted: &lifted,
                        literals: &literals,
                    };
                    define(
                        &mut context.func,
                        &mut shapes,
                        takes,
                        body,
                        frontend,
                        &lowering,
                        &mut module,
                    )?;
                    module.define_function(id, &mut context)?;
                }
                Definition::Composed {
                    declared: behavior_name,
                    stages,
                    ..
                } => {
                    let target = targets.named(behavior_name)?;
                    let signature = signature_over(&target.takes(), &target.answers(), call_conv)?;
                    let id = reachable.of_behavior_named(behavior_name)?;
                    context.clear();
                    context.func =
                        Function::with_name_signature(UserFuncName::default(), signature);
                    let lowering = Lowering {
                        declared: &declared,
                        reachable: &reachable,
                        carrier: name,
                        allocate,
                        compare_text,
                        join_text,
                        closures: &closures,
                        lifted: &lifted,
                        literals: &literals,
                    };
                    define_composed(
                        &mut context.func,
                        &mut shapes,
                        target.takes().len(),
                        stages,
                        frontend,
                        &lowering,
                        &mut module,
                    )?;
                    module.define_function(id, &mut context)?;
                }
            }
        }
        for example in examples {
            let target = targets.named(&format!("{name}.{}", example.behavior))?;
            answers_as_its_target_says(&target.declared(), &example.body, target, &declared)?;
            let signature = running_a_row(&targets, name, &example.behavior, call_conv)?;
            let symbol = example_symbol(name, &example.behavior, example.at);
            let id = *entries
                .get(&symbol)
                .ok_or_else(|| anyhow!("no entry was declared for {symbol}"))?;
            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            let lowering = Lowering {
                declared: &declared,
                reachable: &reachable,
                carrier: name,
                allocate,
                compare_text,
                join_text,
                closures: &closures,
                lifted: &lifted,
                literals: &literals,
            };
            // Taking nothing: what the row states is written into the body, so an entry with
            // parameters would be a row whose values came from whoever ran it.
            define(
                &mut context.func,
                &mut shapes,
                &[],
                &example.body,
                frontend,
                &lowering,
                &mut module,
            )?;
            module.define_function(id, &mut context)?;
        }
    }

    // Every lifted function, defined after every ordinary body: a site's own body may itself hold
    // a nested site, or reach one returned from elsewhere, and every one of them was declared
    // above regardless of which body it is nested under.
    for (&site, plan) in closures.iter() {
        let fn_ = plan.signature;
        let signature = lifted_signature(&fn_.takes, &fn_.answers, call_conv)?;
        let id = *lifted
            .get(&site)
            .ok_or_else(|| anyhow!("closure site {site} was never declared a lifted function"))?;
        context.clear();
        context.func = Function::with_name_signature(UserFuncName::default(), signature);
        let lowering = Lowering {
            declared: &declared,
            reachable: &reachable,
            carrier: plan.module,
            allocate,
            compare_text,
            join_text,
            closures: &closures,
            lifted: &lifted,
            literals: &literals,
        };
        define_closure(
            &mut context.func,
            &mut shapes,
            plan,
            frontend,
            &lowering,
            &mut module,
        )?;
        module.define_function(id, &mut context)?;
    }

    // What a host reaches for an answer as the language writes it: every behavior this object
    // defines and publishes, and every row. A behavior another build implements is that build's
    // to give a boundary to, so one object never answers for a second entry under the same name.
    let mut boundaries = Vec::new();
    for target in &program.behaviors {
        if !matches!(target.is, Answers::Body | Answers::Composed) {
            continue;
        }
        let declared = target.declared();
        let local = locals.get(declared.as_str()).copied().ok_or_else(|| {
            anyhow!("{declared} answers with a local definition no module carries")
        })?;
        if local.publication() != Publication::Published {
            continue;
        }
        boundaries.push(boundary::Boundary {
            symbol: boundary_symbol(&behavior_symbol(&target.module, &target.name)),
            runs: reachable.of_behavior_named(&declared)?,
            takes: target.takes(),
            output: &target.output,
        });
    }
    for written in &program.modules {
        for example in &written.examples {
            let target = targets.named(&format!("{}.{}", written.name, example.behavior))?;
            let entry = example_symbol(&written.name, &example.behavior, example.at);
            let runs = *entries
                .get(&entry)
                .ok_or_else(|| anyhow!("no entry was declared for {entry}"))?;
            boundaries.push(boundary::Boundary {
                symbol: boundary_symbol(&entry),
                runs,
                takes: Vec::new(),
                output: &target.output,
            });
        }
    }
    boundary::define(
        boundary::Emitting {
            module: &mut module,
            context: &mut context,
            shapes: &mut shapes,
            frontend,
            call_conv,
            declared: &declared,
            literals: &literals,
        },
        &boundaries,
    )?;

    Ok(module.finish().emit()?)
}

/// Every behavior the document names, by the key a reference to it says.
///
/// Built once and read by a hash rather than a walk, because a behavior a call reaches is asked
/// for by more than one caller of this — a held definition, an entry for a row, a composition's
/// every stage — and a document with many of any of those would make a walk over every target
/// quadratic in nothing about the program itself.
struct Targets<'a> {
    by_name: HashMap<String, &'a Target>,
}

impl<'a> Targets<'a> {
    fn of(behaviors: &'a [Target]) -> Self {
        let mut by_name = HashMap::with_capacity(behaviors.len());
        for target in behaviors {
            by_name.insert(target.declared(), target);
        }
        Targets { by_name }
    }

    /// The behavior a name in this document reaches, as the table of targets says it.
    fn named(&self, declared: &str) -> Result<&'a Target> {
        self.by_name
            .get(declared)
            .copied()
            .ok_or_else(|| anyhow!("{declared}, which no target names"))
    }
}

/// That a local definition is the one thing its own target says it is.
///
/// Once `Composed` was a local definition beside `Body`, what a name answers with and what its
/// local definition actually is became two readings of one fact — a `Target.is` written by one
/// pass over the checker's program and a `Definition`'s own tag written by another — and nothing
/// upstream holds them to each other the way one Java value holding both would. So this reads a
/// document strictly, the way every other closed set here does: the two halves are checked against
/// each other rather than one of them taken on trust because the other named it.
///
/// A composition carries three more readings of facts its own stages and its own target already
/// answer, so those are checked here too: a stage's own answer against the target it names, the
/// first stage's routing against what the language settles it always is (spec
/// §sequential-composition — the first stage takes the composition's own arguments, so nothing is
/// routed into it), and the first stage's target against what the composition itself is declared
/// to take, since that is where a composition's own parameters are read off (spec
/// §sequential-composition — "the pipeline takes whatever its first stage takes").
fn agrees_with_its_target(
    name: &str,
    target: &Target,
    local: &Definition,
    targets: &Targets,
    declared: &Declared,
) -> Result<()> {
    match (target.is, local) {
        // A body's parameters are the target's inputs, one for one, and what the body answers is
        // a value of what the target says it answers — the same type, or a case of it. Each is a
        // fact crossed twice, and the boundary writes the answer by the target's reading of it.
        (
            Answers::Body,
            Definition::Body {
                parameters, body, ..
            },
        ) => {
            if parameters.len() != target.inputs.len() {
                bail!(
                    "{name} names {} parameters in its body and takes {} at the target that \
                     reaches it: the two halves disagree about what it takes",
                    parameters.len(),
                    target.inputs.len()
                );
            }
            answers_as_its_target_says(name, body, target, declared)
        }
        (
            Answers::Composed,
            Definition::Composed {
                answers, stages, ..
            },
        ) => {
            if answers != &target.answers() {
                bail!(
                    "{name} answers {} as a composition and {} at the target that reaches it: \
                     the two halves disagree about what it answers",
                    answers.spelt(),
                    target.answers().spelt()
                );
            }
            let first = stages
                .first()
                .ok_or_else(|| anyhow!("{name} is a composition composing nothing"))?;
            if !matches!(first.routing, Routing::Always) {
                bail!(
                    "{name}'s first stage is routed rather than always applied: the first stage \
                     of a composition takes the composition's own arguments, and nothing is \
                     routed into it"
                );
            }
            let leads = targets.named(&first.behavior)?;
            if leads.takes() != target.takes() {
                bail!(
                    "{name} takes {} and its first stage {} takes {}: a composition takes \
                     whatever its first stage takes, and the two halves disagree about what \
                     that is",
                    spelt(&target.takes()),
                    first.behavior,
                    spelt(&leads.takes())
                );
            }
            for stage in stages {
                let reached = targets.named(&stage.behavior)?;
                if stage.answers != reached.answers() {
                    bail!(
                        "{}'s stage naming {} answers {} on the wire and {} at the target it \
                         reaches: the two halves disagree",
                        name,
                        stage.behavior,
                        stage.answers.spelt(),
                        reached.answers().spelt()
                    );
                }
            }
            Ok(())
        }
        (is, _) => bail!(
            "{name} crosses as {is:?} in the table of targets, and as a different kind of local \
             definition: the two halves disagree about how it is defined"
        ),
    }
}

/// Refuses a body, or a row's call, whose answer is not a value of what the target answers.
///
/// Asked only once the answer is known to have a layout here: a collection the checker lets a
/// body answer covariantly is not lowered, and saying the halves disagree about it would be this
/// side answering the checker's question without the checker's rules.
fn answers_as_its_target_says(
    name: &str,
    body: &Node,
    target: &Target,
    declared: &Declared,
) -> Result<()> {
    let answers = target.answers();
    if !declared.fits(body.ty(), &answers)? {
        bail!(
            "{name} answers {} where the target that reaches it answers {}: the two halves \
             disagree about what it answers",
            body.ty().spelt(),
            answers.spelt()
        );
    }
    Ok(())
}

/// Several types, spelt the way one reads a diagnostic naming a signature.
fn spelt(types: &[Ty]) -> String {
    types.iter().map(Ty::spelt).collect::<Vec<_>>().join(", ")
}

/// What an entry that runs one of a behavior's rows takes and answers.
///
/// Nothing, and what the behavior answers. The answer's type is read off the behavior's target
/// rather than off the row, because what a behavior answers is the behavior's and a row that said
/// it too would be a second place it was written down.
fn running_a_row(
    targets: &Targets,
    module: &str,
    behavior: &str,
    call_conv: CallConv,
) -> Result<ir::Signature> {
    let target = targets.named(&format!("{module}.{behavior}"))?;
    signature_over(&[], &target.answers(), call_conv)
}

/// What the object makes reachable, from what the module says together with what the object is
/// for.
///
/// Two questions and not one. What a module publishes is its surface in the language; what a
/// symbol table carries is this artifact's answer. An object that read the first as the second
/// would be publishing whatever surface suited the shape it happened to be built in.
///
/// This object is one whole program. A name the module keeps is named by that module alone and
/// every module of the program is in here, so it is reached in here and nowhere else. A name the
/// module publishes may be reached by a host linking this in or by another Souther build's object,
/// and neither of those reaches a symbol the table does not carry.
fn linkage_of(published: Publication) -> Linkage {
    match published {
        Publication::Published => Linkage::Export,
        Publication::Kept => Linkage::Local,
    }
}

/// Everything a body can reach, by the name the document reaches it under.
///
/// A definition a module holds is keyed by both modules — the one holding it and the one that
/// declared it — because two modules holding one declaration hold a copy each and a call reaches
/// the copy its own module holds.
///
/// A value's own home is kept apart from a helper's copy (`values`, not folded into `held`),
/// because the two are different identities even where a document never confuses them: a helper
/// is carried, a value is declared, and souther's own `CheckedModule` already refuses to hold one
/// declaration as both. A published entry is kept apart again (`published_values`), keyed by the
/// declaring module and not by a carrier: unlike a helper or a local value home, an entry is one
/// symbol the whole program shares, addressed by every object that calls it, this one included
/// where it happens to be the declaring module's own.
#[derive(Default)]
struct Reachable {
    held: HashMap<(String, String), FuncId>,
    values: HashMap<(String, String), FuncId>,
    behaviors: HashMap<String, FuncId>,
    published_values: HashMap<(String, String), FuncId>,
}

impl Reachable {
    fn held(&mut self, carrier: &str, declared: &str, id: FuncId) -> Result<()> {
        let key = (carrier.to_string(), declared.to_string());
        if self.held.insert(key, id).is_some() {
            bail!("{carrier} holds two definitions both called {declared}");
        }
        Ok(())
    }

    fn value(&mut self, carrier: &str, declared: &str, id: FuncId) -> Result<()> {
        let key = (carrier.to_string(), declared.to_string());
        if self.values.insert(key, id).is_some() {
            bail!("{carrier} builds two values both called {declared}");
        }
        Ok(())
    }

    fn behavior(&mut self, declared: &str, id: FuncId) -> Result<()> {
        if self.behaviors.insert(declared.to_string(), id).is_some() {
            bail!("two behaviors are both written {declared}");
        }
        Ok(())
    }

    fn published_value(&mut self, module: &str, name: &str, id: FuncId) -> Result<()> {
        let key = (module.to_string(), name.to_string());
        if self.published_values.insert(key, id).is_some() {
            bail!("`{module}` publishes two entries both called {name}");
        }
        Ok(())
    }

    /// Whether an entry for `module`'s value `name` has already been declared — asked before
    /// declaring one as an import, so a value this program's own modules publish is never given a
    /// second, importing declaration of the same symbol.
    fn is_published(&self, module: &str, name: &str) -> bool {
        self.published_values
            .contains_key(&(module.to_string(), name.to_string()))
    }

    fn of_held(&self, carrier: &str, declared: &str) -> Result<FuncId> {
        self.held
            .get(&(carrier.to_string(), declared.to_string()))
            .copied()
            .ok_or_else(|| anyhow!("{carrier} reaches {declared}, which it holds no copy of"))
    }

    fn of_value(&self, carrier: &str, declared: &str) -> Result<FuncId> {
        self.values
            .get(&(carrier.to_string(), declared.to_string()))
            .copied()
            .ok_or_else(|| {
                anyhow!("{carrier} reaches the value {declared}, which it builds no home for")
            })
    }

    fn of_behavior_named(&self, declared: &str) -> Result<FuncId> {
        self.behaviors
            .get(declared)
            .copied()
            .ok_or_else(|| anyhow!("a call reaching {declared}, which the program does not name"))
    }

    fn of_published_value(&self, module: &str, name: &str) -> Result<FuncId> {
        self.published_values
            .get(&(module.to_string(), name.to_string()))
            .copied()
            .ok_or_else(|| {
                anyhow!(
                    "a call reaching `{module}`'s published value {name}, which no entry was \
                         declared for"
                )
            })
    }
}

/// The machine parameters a value's own method takes: its handovers' types, in the order
/// `ProgramWriter` numbered their binders — the same order [`define`]'s own `takes`/binding
/// convention already expects, so nothing here has to renumber anything.
fn handover_types(value: &transport::Value) -> Vec<Ty> {
    value
        .handovers
        .iter()
        .map(|handover| handover.ty.clone())
        .collect()
}

/// Every published value a call anywhere in this document reaches, by the module and the name the
/// call names, with the answer type one of its call sites carries — read once, over every body
/// this document holds, rather than at each call site during lowering: a value this program's own
/// modules do not declare an entry for still needs a symbol declared before anything that calls it
/// is defined, which is the two-phase shape (declare, then define) every other function in this
/// object already keeps.
///
/// A `BTreeMap` and not a `HashMap`, because this is walked to declare symbols in whatever order
/// it hands them back, and nowhere else in this file lets an unordered map decide an order that
/// ends up in the object it emits — the token loop above walks `program.declarations` itself for
/// exactly that reason. A `HashMap` here would make two builds of one document free to declare
/// these imports in different orders for no reason the source states.
///
/// One type per value and not one per call: every call to one value answers with the same type,
/// since it is one declaration. Held to that rather than assumed — a second call site naming a
/// different type for a value already found is the checker and this reading of its document
/// disagreeing about something more basic than this side not having built it yet, and is refused
/// the way every other such disagreement in this file is, rather than silently kept as whichever
/// type was found first.
fn published_value_calls(program: &Program) -> Result<BTreeMap<(String, String), Ty>> {
    let mut found = BTreeMap::new();
    for written in &program.modules {
        for held in &written.helpers {
            walk_calls(&held.body, &mut found)?;
        }
        for value in &written.values {
            walk_calls(&value.body, &mut found)?;
        }
        for entry in &written.entries {
            walk_calls(&entry.body, &mut found)?;
        }
        for local in &written.definitions {
            if let Definition::Body { body, .. } = local {
                walk_calls(body, &mut found)?;
            }
        }
    }
    Ok(found)
}

/// Every `Reaches::PublishedValue` under `node`, depth first. No default arm: a `Node` variant
/// this misses is a value call this walk silently never finds, which is exactly the silent drop
/// declaring a value's import symbol exists to end.
fn walk_calls(node: &Node, found: &mut BTreeMap<(String, String), Ty>) -> Result<()> {
    match node {
        Node::Call {
            reaches,
            arguments,
            ty,
            ..
        } => {
            if let Reaches::PublishedValue { module, name } = reaches {
                match found.get(&(module.clone(), name.clone())) {
                    Some(already) if already != ty => {
                        bail!(
                            "a call reaches `{module}`'s published value {name} as {}, and \
                             another reaches it as {} — one declaration does not answer two ways",
                            already.spelt(),
                            ty.spelt()
                        );
                    }
                    Some(_) => {}
                    None => {
                        found.insert((module.clone(), name.clone()), ty.clone());
                    }
                }
            }
            for argument in arguments {
                walk_calls(argument, found)?;
            }
        }
        Node::Binary { left, right, .. } => {
            walk_calls(left, found)?;
            walk_calls(right, found)?;
        }
        Node::Neg { operand, .. } => walk_calls(operand, found)?,
        Node::Let { value, body, .. } => {
            walk_calls(value, found)?;
            walk_calls(body, found)?;
        }
        Node::If {
            cond, then, els, ..
        } => {
            walk_calls(cond, found)?;
            walk_calls(then, found)?;
            walk_calls(els, found)?;
        }
        Node::Construct { values, .. } => {
            for value in values {
                walk_calls(value, found)?;
            }
        }
        Node::Field { target, .. } => walk_calls(target, found)?,
        Node::Match { subject, arms, .. } => {
            walk_calls(subject, found)?;
            for arm in arms {
                walk_calls(&arm.body, found)?;
            }
        }
        Node::Some { value, .. } => walk_calls(value, found)?,
        Node::Tuple { members, .. } => {
            for member in members {
                walk_calls(member, found)?;
            }
        }
        Node::Member { tuple, .. } => walk_calls(tuple, found)?,
        Node::Block { body, .. } => walk_calls(body, found)?,
        Node::Apply {
            function,
            arguments,
            ..
        } => {
            walk_calls(function, found)?;
            for argument in arguments {
                walk_calls(argument, found)?;
            }
        }
        Node::Int { .. }
        | Node::Read { .. }
        | Node::Bool { .. }
        | Node::Str { .. }
        | Node::Unit { .. }
        | Node::None { .. } => {}
    }
    Ok(())
}

/// Every declared type of the program, by the key a reference to one says.
///
/// Two questions are asked of this and they are not one question. What a type is made of decides
/// where a field sits and what a value costs to make; what a type *is* decides which arm a fork
/// takes. The second used to be answered out of the first — a declaration's position among the
/// ones one document happened to bring — and that is exactly what made it this object's own.
///
/// So what a value is tagged with is not held here. It is a symbol, and what resolves it is the
/// linker; this resolves a key to the declaration that says what the symbol is called.
struct Declared<'a> {
    shapes: HashMap<String, &'a Declaration>,
}

impl<'a> Declared<'a> {
    fn of(declarations: &'a [Declaration]) -> Result<Self> {
        let mut shapes = HashMap::new();
        for declaration in declarations {
            let key = declaration.key();
            if shapes.insert(key.clone(), declaration).is_some() {
                bail!("two declarations are both written {key}");
            }
        }
        let declared = Declared { shapes };
        for declaration in declarations {
            if let Declaration::Sum { cases, form, .. } = declaration {
                declared.settled(&declaration.key(), cases, form)?;
            }
        }
        Ok(declared)
    }

    /// Refuses a set of alternatives the checker could not have settled, as the two halves
    /// disagreeing. The relation is the one `Boundary` decides upstream, stated whole and not
    /// only the direction that has bitten:
    ///
    /// - the set travels as an enumeration exactly when it has cases and every one of them is a
    ///   declared unit — so an enumeration over a case with fields, and a discriminated form over
    ///   nothing but units, are both refused;
    /// - a discriminated form's tag is a key no product case lays a field under, since the case's
    ///   fields and the tag stand in one object and one of the two would be lost.
    ///
    /// Asked of every sum when the document is read, and of every answer union ([`object_for`]),
    /// so nothing downstream is handed a form and cases that disagree.
    fn settled(&self, owner: &str, cases: &[Case], form: &AlternativesForm) -> Result<()> {
        let mut not_a_unit = None;
        for case in cases {
            let unit = match case {
                Case::Declared { declared } => {
                    matches!(self.shape(declared)?, Declaration::Unit { .. })
                }
                Case::Primitive { .. } | Case::Language { .. } => false,
            };
            if !unit && not_a_unit.is_none() {
                not_a_unit = Some(case.spelt());
            }
        }
        let every_one_a_unit = !cases.is_empty() && not_a_unit.is_none();
        match form {
            AlternativesForm::Enumeration if !every_one_a_unit => bail!(
                "{owner} travels as an enumeration and its case {} is not a unit: the two halves \
                 disagree about its form",
                not_a_unit.unwrap_or_else(|| "list is empty".to_string())
            ),
            AlternativesForm::Discriminated { .. } if every_one_a_unit => bail!(
                "{owner} travels discriminated and every one of its cases is a unit, which is an \
                 enumeration: the two halves disagree about its form"
            ),
            AlternativesForm::Enumeration => Ok(()),
            AlternativesForm::Discriminated { tag, .. } => {
                for case in cases {
                    let Case::Declared { declared } = case else {
                        continue;
                    };
                    if let Declaration::Product { fields, .. } = self.shape(declared)?
                        && fields.iter().any(|field| field.name == *tag)
                    {
                        bail!(
                            "{declared} lays a field under {tag} and stands as a case of {owner}, \
                             whose tag stands under the same key: the two halves disagree, since \
                             the checker refuses the field"
                        );
                    }
                }
                Ok(())
            }
        }
    }

    /// Refuses an answer union whose cases are not what its type descends to. The two cross apart
    /// on purpose — the type as its members were written, the cases as the boundary walked them —
    /// and the cases are what a value is written by; so they are checked against each other here
    /// and nothing is worked out again from the type for writing.
    fn descends_to(&self, owner: &str, ty: &Ty, cases: &[Case]) -> Result<()> {
        let Ty::Union { union } = ty else {
            bail!(
                "{owner} answers cases under {}, which is not a union",
                ty.spelt()
            );
        };
        let walked = self.leaves_of(union)?;
        if walked != cases {
            bail!(
                "{owner} answers {} and is written by the cases {}: the two halves disagree about \
                 what it answers",
                ty.spelt(),
                cases
                    .iter()
                    .map(Case::spelt)
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
        }
        Ok(())
    }

    /// The cases a value of any of `members` can be, a sum walked into and each case kept at the
    /// place it was first reached, which is the order the checker gives a sum's own.
    fn leaves_of(&self, members: &[Case]) -> Result<Vec<Case>> {
        let mut leaves: Vec<Case> = Vec::new();
        for member in members {
            let reached = match member {
                Case::Declared { declared } => match self.shape(declared)? {
                    Declaration::Sum { cases, .. } => cases.clone(),
                    _ => vec![member.clone()],
                },
                _ => vec![member.clone()],
            };
            for leaf in reached {
                if !leaves.contains(&leaf) {
                    leaves.push(leaf);
                }
            }
        }
        Ok(leaves)
    }

    /// Whether every value of `actual` is a value of `expected`: the same scalar, or, for
    /// declared types and unions of them, every case the one descends to being among the other's.
    ///
    /// Asked only of types this backend lays out. Whether one type's values are another's is the
    /// checker's question, and it has answers here — a collection's covariance among them — that
    /// this side has no reason to know until it lays a collection out. So a type with no layout
    /// here is refused as not lowered before anything is compared, and the question is answered
    /// only as far as the nominal membership the declarations already hold.
    fn fits(&self, actual: &Ty, expected: &Ty) -> Result<bool> {
        machine_type(actual)?;
        machine_type(expected)?;
        if actual == expected {
            return Ok(true);
        }
        match (self.cases_of(actual)?, self.cases_of(expected)?) {
            (Some(actual), Some(expected)) => Ok(actual.iter().all(|case| expected.contains(case))),
            (None, None) if matches!((actual, expected), (Ty::Prim { .. }, Ty::Prim { .. })) => {
                Ok(false)
            }
            (Some(_), None) | (None, Some(_)) => Ok(false),
            (None, None) => bail!(
                "whether {} is {}, which nothing here has a reason to ask",
                actual.spelt(),
                expected.spelt()
            ),
        }
    }

    fn cases_of(&self, ty: &Ty) -> Result<Option<Vec<Case>>> {
        Ok(match ty {
            Ty::Declared { declared } => Some(self.leaves_of(&[Case::Declared {
                declared: declared.clone(),
            }])?),
            Ty::Union { union } => Some(self.leaves_of(union)?),
            _ => None,
        })
    }

    /// Whether a value of `actual` is one a field carrying `codec` holds. Asked, as [`fits`] is,
    /// only of what this backend lays out: a field carrying a collection is not lowered, whatever
    /// it would have been handed.
    ///
    /// [`fits`]: Declared::fits
    fn carries(&self, codec: &CodecShape, actual: &Ty) -> Result<bool> {
        machine_type(&codec.ty())?;
        machine_type(actual)?;
        Ok(match (codec, actual) {
            (CodecShape::Scalar { scalar }, Ty::Prim { prim }) => scalar.prim() == *prim,
            (CodecShape::Named { declared }, Ty::Declared { .. } | Ty::Union { .. }) => self.fits(
                actual,
                &Ty::Declared {
                    declared: declared.clone(),
                },
            )?,
            // Laid out, and its own layout says nothing of what it holds; so what it holds is
            // asked the same question, and refused there if that has no layout.
            (CodecShape::OptionOf { present }, Ty::Option { option }) => {
                self.carries(present.shape(), option)?
            }
            _ => false,
        })
    }

    fn shape(&self, declared: &str) -> Result<&'a Declaration> {
        self.shapes
            .get(declared)
            .copied()
            .ok_or_else(|| anyhow!("a value of {declared}, which no declaration crossed for"))
    }

    /// The token a value of this type is tagged by, as this object names it.
    ///
    /// Asked for by the key, and the symbol built from what the declaration carries — never from
    /// the key itself. The key is how a reference reaches a declaration; splitting one back up
    /// would be this side working out an identity it was handed, which is the same mistake as
    /// counting one.
    ///
    /// Declared here and not before, because naming a token is what makes it a name this object
    /// wants resolved. A declaration at home in this object has its token defined whether anything
    /// here builds a value of it or not — that is what being its home means — and one from another
    /// build is named only by the object that builds or forks on a value of it. Asking twice is
    /// asking once: a declaration is one symbol and Cranelift answers with the one it already has.
    fn tag(&self, module: &mut ObjectModule, declared: &str) -> Result<DataId> {
        let declaration = self.shape(declared)?;
        if let Declaration::Sum { .. } = declaration {
            bail!(
                "a tag for {declared}, which is a sum: nothing is ever tagged with one, since an \
                 arm tests the leaves a case resolved to"
            );
        }
        let linkage = match declaration.by() {
            // At home here. Exported rather than kept, because another build naming this
            // declaration reaches this object's token and nothing else — and whether the module
            // publishes the type is a question the program API answers for a behavior and not yet
            // for a declaration, so this object cannot ask it.
            DeclaredBy::AModule => Linkage::Export,
            DeclaredBy::OnThePath => Linkage::Import,
            DeclaredBy::TheLanguage => {
                return Err(not_lowered(format!(
                    "a value of {declared}, which the language declares and no build of a module \
                     defines"
                )));
            }
        };
        let symbol = type_symbol(declaration.module(), declaration.name());
        Ok(module.declare_data(&symbol, linkage, false, false)?)
    }
}

/// The address of the declaration's token, as a value of it says which type it is.
fn tag_of(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    declared: &str,
) -> Result<ir::Value> {
    let token = lowering.declared.tag(module, declared)?;
    let named = module.declare_data_in_func(token, builder.func);
    Ok(builder.ins().symbol_value(POINTER, named))
}

/// A value the lowering has made, and what the language says it is.
///
/// The two travel together because apart they are what a wrong answer is made of. An `Int` and
/// every address are both `i64` here, so an instruction chosen from the machine value alone cannot
/// tell a number from where a value is kept, and Cranelift has nothing to object to: the widths
/// agree. What went wrong once already was a comparison emitted from one operand's type while the
/// other was something else, and it emitted an `icmp` over a number and an address.
///
/// Made from a node and what that node lowered to, so the type cannot have come from somewhere
/// other than the value did.
#[derive(Clone, Copy)]
struct Held<'a> {
    value: ir::Value,
    ty: &'a Ty,
}

impl<'a> Held<'a> {
    fn of(node: &'a Node, value: ir::Value) -> Held<'a> {
        Held {
            value,
            ty: node.ty(),
        }
    }
}

/// What the lowering of one function needs besides the function itself.
struct Lowering<'a> {
    declared: &'a Declared<'a>,
    reachable: &'a Reachable,
    /// The module whose copy of a definition a call from here reaches.
    carrier: &'a str,
    allocate: FuncId,
    compare_text: FuncId,
    join_text: FuncId,
    /// Every closure site the whole document holds, and what each one reaches — read here rather
    /// than re-walked per body, since a `Node::Block` nested under one top-level body may be
    /// referenced (its captures restored) while defining a different site's own lifted function.
    closures: &'a ClosureSites<'a>,
    /// The lifted function declared for each site, by the site's own number — declared before any
    /// body is defined, the same two-phase shape every other declaration in this object keeps.
    lifted: &'a BTreeMap<usize, FuncId>,
    /// What string literals this object already holds.
    literals: &'a Literals,
}

impl Lowering<'_> {
    /// Room for `bytes` bytes, from the arena the caller brackets.
    ///
    /// Bytes and not slots, because how many slots a value is made of is a fact about its layout
    /// and the layout is stated in the `abi` crate. A caller counting slots here would be working
    /// out for itself where the fields start, which is the other half of a fact it is already
    /// reading from there.
    fn room(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        bytes: i64,
    ) -> ir::Value {
        let taking = module.declare_func_in_func(self.allocate, builder.func);
        let size = builder.ins().iconst(types::I64, bytes);
        let taken = builder.ins().call(taking, &[size]);
        builder.inst_results(taken)[0]
    }
}

/// A generated function's signature, in the `status + out` shape every one of them shares.
///
/// The value crosses through one more parameter than a caller reading only `takes` and `answers`
/// would expect — a pointer the answer is written through — and the return says whether it is
/// there to read: `ANSWERED` if so, a language abort's wire number if not. A plain return of the
/// answer can only ever say the first of those, which is exactly the gap issue #9 closes; see
/// `define`'s own doc for the rest of the shape this signature is half of.
fn signature_over(takes: &[Ty], answers: &Ty, call_conv: CallConv) -> Result<ir::Signature> {
    let mut signature = ir::Signature::new(call_conv);
    for taken in takes {
        signature.params.push(AbiParam::new(machine_type(taken)?));
    }
    // Validated for the same reason it always was — a primitive with no representation is refused
    // here — even though its width no longer decides what this function returns.
    machine_type(answers)?;
    signature.params.push(AbiParam::new(POINTER));
    signature.returns.push(AbiParam::new(types::I32));
    Ok(signature)
}

/// A lifted function's signature: the same `status + out` shape [`signature_over`] gives every
/// other generated function, with one more parameter prepended — the closure calling it, which
/// *is* its environment (see this module's own doc on the flat-closure layout) and not a second
/// pointer beside one.
///
/// Every lifted function takes this hidden parameter whether its own site captures anything or
/// not, so a caller reaching one through `closure[0]` never has to ask which is which: the
/// signature `call_indirect` builds and the signature this declares always agree.
fn lifted_signature(takes: &[Ty], answers: &Ty, call_conv: CallConv) -> Result<ir::Signature> {
    let mut signature = ir::Signature::new(call_conv);
    signature.params.push(AbiParam::new(POINTER));
    for taken in takes {
        signature.params.push(AbiParam::new(machine_type(taken)?));
    }
    machine_type(answers)?;
    signature.params.push(AbiParam::new(POINTER));
    signature.returns.push(AbiParam::new(types::I32));
    Ok(signature)
}

/// What a value of this type is on the machine.
///
/// A number, a truth, or the address of what a value is made of. Every other primitive is a value
/// with a representation to design — how it is held, who owns it, what frees it — and none of that
/// is decided by giving it a width here.
fn machine_type(ty: &Ty) -> Result<types::Type> {
    match ty {
        Ty::Declared { .. } | Ty::Option { .. } | Ty::Tuple { .. } => Ok(POINTER),
        // What holds a union holds one of its members, and says which by the token at the front of
        // it. A primitive or a case the language gives carries no token, so a union with one among
        // its members has no representation here yet: the members would not say which they are.
        Ty::Union { union } => match union.iter().find(|it| !matches!(it, Case::Declared { .. })) {
            None => Ok(POINTER),
            Some(case) => Err(not_lowered(format!(
                "a value of {}, whose case {} carries no token to say which case it is",
                ty.spelt(),
                case.spelt()
            ))),
        },
        // A collection is a value with a layout to design, and none is designed yet. Read whole
        // off the wire all the same: whether a type crosses and whether it can be laid out here
        // are two questions, and only this one is this backend's.
        Ty::List { .. } | Ty::Set { .. } | Ty::Map { .. } => {
            Err(not_lowered(format!("a value of type {}", ty.spelt())))
        }
        // A flat closure: one pointer, the same as every other compound value. Slot 0 holds the
        // lifted function's code address and every slot after it a capture — see `closures` — but
        // none of that is a second machine type; a function value is a pointer here exactly as a
        // tuple or a declared value is.
        Ty::Fn { .. } => Ok(POINTER),
        // Every primitive is named. A set the language closed is one this has to answer for member
        // by member: caught by an arm standing for the rest, a primitive added to the language
        // would arrive here as something with no representation and nothing would have said so.
        Ty::Prim { prim } => match prim {
            Prim::Int => Ok(types::I64),
            Prim::Bool => Ok(types::I8),
            // The address of a count of bytes and the text that follows it. Where that stands —
            // the object, for a literal, or the arena, for one a run worked out — is not something
            // the value says, and nothing that reads one has to ask.
            Prim::String => Ok(POINTER),
            Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => Err(not_lowered(format!("a value of type {}", prim.spelt()))),
        },
    }
}

/// Holds the signature of a behavior the object does not define to what a value still means in
/// another object.
///
/// Said here, where the signature is declared, because that is the one place the two scopes meet.
fn crosses_objects(target: &Target) -> Result<()> {
    for ty in target.takes().iter().chain([&target.answers()]) {
        crosses_object(
            &format!("{}.{} takes or answers", target.module, target.name),
            ty,
        )?;
    }
    Ok(())
}

/// Refuses `ty` where a value of it means something different once it has crossed into an object
/// built from another document — `clause` is read straight into the message, ending just short of
/// the type, so a caller states what it is asking about (`"m.f takes or answers"`, `` "`m`'s
/// published value x answers" ``) rather than this function guessing a grammar for every caller.
///
/// The one check every cross-object boundary this backend admits is held to, [`crosses_objects`]'s
/// behaviors and a foreign [`Reaches::PublishedValue`]'s answer alike, so the two cannot drift into
/// being checked two different ways — which is exactly how a published value answering
/// `Option<Decimal>` would slip past a check a behavior answering the same type refuses, had this
/// been written twice instead of shared.
fn crosses_object(clause: &str, ty: &Ty) -> Result<()> {
    if !means_the_same_elsewhere(ty) {
        return Err(not_lowered(format!(
            "{clause} {}, which has no representation an object built from another document \
             reads the same way, and it is reached across objects",
            ty.spelt()
        )));
    }
    Ok(())
}

/// Whether a value of this type means in another object what it means in this one.
///
/// Asked of the representation and answered over the type it is made of, because that is the shape
/// of the question: a tuple means what its members mean, and an optional means what it holds. A
/// list of the type names that happen to be admitted today would be a different thing — it would
/// have to be argued over again every time a representation was designed, and the argument would
/// be about the name rather than about what was designed.
fn means_the_same_elsewhere(ty: &Ty) -> bool {
    match ty {
        // Named member by member for the same reason the widths are: a primitive added to the
        // language must be answered for here rather than admitted by an arm standing for the rest,
        // since what this decides is whether a value of it may leave the object.
        Ty::Prim { prim } => match prim {
            Prim::Int | Prim::Bool => true,
            // A count of bytes and then that many bytes of text, at an address. The layout is
            // stated in the crate both halves of this backend read, so two objects it built agree
            // about it for the reason they agree that an `Int` is sixty-four bits wide. No linker
            // is involved: there is no name here for one to resolve.
            Prim::String => true,
            Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => false,
        },
        // A value of a declared type says which type it is with the address of its declaration's
        // token, which the linker resolves. Two objects naming one declaration reach one address,
        // so the comparison a fork makes is about the same thing on either side.
        //
        // Which is what a value says it is, and not where its fields are. Both objects read the
        // field order off their own copy of the declaration, so they agree while they were checked
        // against the one build of the module that declares it — and whether the object handed to
        // the linker is that build is not something an object can ask. That is a separate
        // question, and answering it here would be answering it with the wrong thing.
        Ty::Declared { .. } => true,
        // Written nowhere at run time: what holds a union holds one of its members, and each of
        // those says which type it is.
        Ty::Union { union } => union.iter().all(|it| matches!(it, Case::Declared { .. })),
        Ty::Option { option } => means_the_same_elsewhere(option),
        // No layout, so nothing another object could read the same way.
        Ty::List { .. } | Ty::Set { .. } | Ty::Map { .. } => false,
        Ty::Tuple { tuple } => tuple.iter().all(means_the_same_elsewhere),
        // Unconditionally, and not by asking whether its parameters and its answer do: what a
        // closure's pointer holds — a code address, and after it whatever it captured — is a
        // contract between this object's own generated code and no one else's. Nothing about a
        // capture's own representation is why; even a closure over nothing but `Int`s carries a
        // code address into whatever links this in, and no other object built by this compiler
        // yet agrees on a layout to read one back from, or on what calling through one means.
        // That is a contract to publish deliberately (a closure header and an invocation
        // convention in `souther-native-abi`) and not one to grant by recursing into a signature
        // that happens to be built from types that already cross.
        Ty::Fn { .. } => false,
    }
}

/// What holds the address of a value made of fields.
///
/// One width, and the host's. Nothing here is written for a machine this is not running on, and a
/// pointer narrower or wider than a slot would make a field's offset a question about the host
/// rather than a multiplication.
const POINTER: types::Type = types::I64;

/// A function value's layout: one pointer, its slot 0 the lifted function's code address and every
/// slot after it a capture, in the order `closures::Site::captures` gives them.
///
/// Compiler-private and not in the `abi` crate: nothing outside code this same compiler generates
/// ever reads one of these — a caller across an object boundary reaches a closure at all only by
/// refusing to, since `means_the_same_elsewhere(Ty::Fn)` is unconditionally `false`. Read the doc
/// on that arm for why it is unconditional and not asked of the captures themselves.
const CLOSURE_CODE: i64 = 0;

/// Where a capture at this position among a closure's own sits, by the same one-slot-per-value
/// convention every other layout in this file keeps.
fn capture_at(position: usize) -> i64 {
    SLOT * (position as i64 + 1)
}

/// How much room a closure over this many captures takes: one slot for the code pointer, one for
/// each capture.
fn room_for_closure(captures: usize) -> i64 {
    SLOT * (captures as i64 + 1)
}

/// Everything a value is made of sits in a slot of one width, so what is put in one is widened to
/// it and what comes out is narrowed back.
///
/// A `Bool` is the only thing narrower today. Widening it here rather than laying it out where it
/// fits keeps a field's offset a fact about its position and not about the types before it.
/// How this reads and writes what it has just made room for.
///
/// Aligned and not trapping, which is what the arena answers: room is handed out a slot at a time
/// and a pointer from it is one nothing else is using. Written once here so that every access says
/// the same thing rather than each site deciding what it trusts.
const TRUSTED: MemFlagsData = MemFlagsData::trusted();

fn into_slot(builder: &mut FunctionBuilder, value: ir::Value) -> ir::Value {
    if builder.func.dfg.value_type(value) == types::I64 {
        value
    } else {
        builder.ins().uextend(types::I64, value)
    }
}

fn out_of_slot(builder: &mut FunctionBuilder, held: ir::Value, wanted: types::Type) -> ir::Value {
    if wanted == types::I64 {
        held
    } else {
        builder.ins().ireduce(wanted, held)
    }
}

/// A behavior's or a helper's body, lowered to the one shape every generated function shares:
/// `takes` ordinary parameters, one more the answer is written through, and a status answered in
/// place of the value itself.
///
/// The block this makes room for beside the entry — `abort` — is where every way this function's
/// body ends without a value meets: an arithmetic site leaving its type's range jumps to it
/// directly, and a call this body makes that itself answers a status other than `ANSWERED` is
/// forwarded to it by `call_reached` rather than read as a value. Sharing the one block is what
/// keeps a calling convention or an abort ABI that changes a change made once, in `call_reached`'s
/// own words — every site that can end without a value reaches this the same way, a jump with the
/// status as its argument, so nothing downstream has to know which of them it was.
///
/// Not sealed until the whole body is lowered, because a block sealed before every jump that
/// reaches it is written is a block Cranelift has already closed the door on — and which sites
/// jump here is exactly what lowering the body decides.
fn define(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    takes: &[Ty],
    body: &Node,
    frontend: TargetFrontendConfig,
    lowering: &Lowering,
    module: &mut ObjectModule,
) -> Result<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut bindings = Bindings::default();
    for (at, taken) in takes.iter().enumerate() {
        let variable = builder.declare_var(machine_type(taken)?);
        let given = builder.block_params(entry)[at];
        builder.def_var(variable, given);
        bindings.at(at, variable);
    }
    let out = builder.block_params(entry)[takes.len()];

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);

    let answer = lower(&mut builder, lowering, module, &mut bindings, abort, body)?;
    builder.ins().store(TRUSTED, answer, out, 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);

    builder.seal_block(abort);
    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.finalize(frontend);
    Ok(())
}

/// A lifted function's own body: the same `status + out` shape [`define`] gives every other body,
/// with one more thing to do before any of it runs — restore every capture the site's own plan
/// says it closed over, from the closure this function was called through.
///
/// The hidden closure parameter is `entry`'s own first parameter (see [`lifted_signature`]); it is
/// never bound into `Bindings` under a binding number of its own, because nothing in the checker's
/// Core ever reads it — a capture is read by the binding number it had where the block was
/// written, not by a name for the environment carrying it. Restoring one is a load at the capture's
/// own slot ([`capture_at`]) narrowed the way any other value out of a slot is
/// ([`out_of_slot`]), bound to a fresh variable under that same number, exactly as if the
/// enclosing body's own `Let` had just bound it here — which, from this function's own body's
/// point of view, is exactly what happened.
fn define_closure(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    site: &Site,
    frontend: TargetFrontendConfig,
    lowering: &Lowering,
    module: &mut ObjectModule,
) -> Result<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let closure = builder.block_params(entry)[0];

    let mut bindings = Bindings::default();
    for (position, capture) in site.captures.iter().enumerate() {
        let wanted = machine_type(&capture.ty)?;
        let held = builder
            .ins()
            .load(types::I64, TRUSTED, closure, capture_at(position) as i32);
        let restored = out_of_slot(&mut builder, held, wanted);
        let variable = builder.declare_var(wanted);
        builder.def_var(variable, restored);
        bindings.at(capture.binding, variable);
    }

    let takes = &site.signature.takes;
    for (at, parameter) in site.parameters.iter().enumerate() {
        let variable = builder.declare_var(machine_type(&takes[at])?);
        let given = builder.block_params(entry)[1 + at];
        builder.def_var(variable, given);
        bindings.at(parameter.binding, variable);
    }
    let out = builder.block_params(entry)[1 + site.parameters.len()];

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);

    let answer = lower(
        &mut builder,
        lowering,
        module,
        &mut bindings,
        abort,
        site.body,
    )?;
    builder.ins().store(TRUSTED, answer, out, 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);

    builder.seal_block(abort);
    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.finalize(frontend);
    Ok(())
}

/// A behavior written as stages applied in order, each offered what the one before answered.
///
/// Nothing here is worked out again: which cases a stage is offered and what leaves the main line
/// are the checker's answers, written on `routing`, and this only realises them. The running value
/// starts as the first stage's answer — the first stage takes the composition's own arguments, so
/// nothing is routed into it. Every stage after either takes it on (`Always`), or is offered it
/// only where it is one of the cases named (`OnCases`) — and where it is not, the composition
/// answers with the value as it stands, which is a return and not a value carried into whatever
/// this stage's own test happens to be. A value that left the main line at one stage does not
/// rejoin it because a later stage's cases happen to overlap with what it is; nothing here tests it
/// against anything again. A value never carries a mark saying it once left the main line; which
/// path a run took is structural, decided by which block Cranelift is in.
fn define_composed(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    takes: usize,
    stages: &[Stage],
    frontend: TargetFrontendConfig,
    lowering: &Lowering,
    module: &mut ObjectModule,
) -> Result<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);
    let out = builder.block_params(entry)[takes];

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);

    let (first, rest) = stages
        .split_first()
        .ok_or_else(|| anyhow!("a composition composes something"))?;
    let arguments: Vec<ir::Value> = builder.block_params(entry)[..takes].to_vec();
    let mut running = {
        let reached = lowering.reachable.of_behavior_named(&first.behavior)?;
        let answers = machine_type(&first.answers)?;
        call_reached(&mut builder, module, abort, reached, answers, &arguments)?
    };

    for stage in rest {
        match &stage.routing {
            Routing::Always => {
                let reached = lowering.reachable.of_behavior_named(&stage.behavior)?;
                let answers = machine_type(&stage.answers)?;
                running = call_reached(&mut builder, module, abort, reached, answers, &[running])?;
            }
            Routing::OnCases { accepted } => {
                let accepts =
                    is_one_of_declared_cases(&mut builder, lowering, module, running, accepted)?;
                let offer = builder.create_block();
                let leave = builder.create_block();
                builder.ins().brif(accepts, offer, &[], leave, &[]);
                builder.seal_block(offer);
                builder.seal_block(leave);

                // What left the main line is answered here, at the stage that did not accept it,
                // rather than carried along to be tested against a stage further on.
                builder.switch_to_block(leave);
                builder.ins().store(TRUSTED, running, out, 0);
                let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
                builder.ins().return_(&[ok]);

                builder.switch_to_block(offer);
                let reached = lowering.reachable.of_behavior_named(&stage.behavior)?;
                let answers = machine_type(&stage.answers)?;
                running = call_reached(&mut builder, module, abort, reached, answers, &[running])?;
            }
        }
    }

    builder.ins().store(TRUSTED, running, out, 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);

    builder.seal_block(abort);
    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.finalize(frontend);
    Ok(())
}

/// A call to a behavior this object either defines or names, and what it answered.
///
/// Shared by a `Core.Call` and a composition's stage, which both reach a behavior the same way: a
/// FuncId already resolved, arguments already lowered. Neither writes the call twice, so a
/// calling convention or an abort ABI that changes moves once and not at every site that reaches
/// out.
///
/// Every generated function answers `status + out` (see `signature_over`'s own doc), so every call
/// here hands over one more argument than `arguments` shows — room on this function's own stack
/// the answer is written through — and reads the status back before trusting what is in it. A
/// status other than `ANSWERED` is not this call's to interpret: it already went through
/// `native_status` once, at whichever site first left its range or ran out of representation, and
/// asking what it means a second time here would be the reclassification issue #9 exists to rule
/// out. So it is not read; it is forwarded, to `abort`, exactly as it arrived — which is what makes
/// a callee's abort cross a call boundary the same way an answer does, transparently, all the way
/// out to whichever caller first receives a status that is not zero.
fn call_reached(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    abort: ir::Block,
    reached: FuncId,
    answers: types::Type,
    arguments: &[ir::Value],
) -> Result<ir::Value> {
    let reaching = module.declare_func_in_func(reached, builder.func);
    let out = out_slot(builder);
    let mut given = arguments.to_vec();
    given.push(out);
    let called = builder.ins().call(reaching, &given);
    let status = builder.inst_results(called)[0];
    Ok(status_or_answer(builder, abort, status, out, answers))
}

/// A closure applied: `Core.Apply`, lowered as an indirect call through the code pointer its own
/// slot 0 holds, with the closure itself handed over as the hidden environment argument every
/// lifted function's own signature reserves ([`lifted_signature`]).
///
/// Shares [`status_or_answer`] with [`call_reached`] rather than repeating it, so an indirect call
/// forwards a callee's abort exactly the way a direct one does — the two calling conventions differ
/// only in what is called and what the first argument is, not in how a status that is not
/// `ANSWERED` reaches this function's own `abort` block.
fn call_indirect_reached(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    closure: ir::Value,
    call_conv: CallConv,
    takes: &[Ty],
    answers: types::Type,
    arguments: &[ir::Value],
) -> Result<ir::Value> {
    let mut signature = ir::Signature::new(call_conv);
    signature.params.push(AbiParam::new(POINTER));
    for taken in takes {
        signature.params.push(AbiParam::new(machine_type(taken)?));
    }
    signature.params.push(AbiParam::new(POINTER));
    signature.returns.push(AbiParam::new(types::I32));
    let sig_ref = builder.import_signature(signature);

    let code = builder
        .ins()
        .load(POINTER, TRUSTED, closure, CLOSURE_CODE as i32);
    let out = out_slot(builder);
    let mut given = Vec::with_capacity(arguments.len() + 2);
    given.push(closure);
    given.extend_from_slice(arguments);
    given.push(out);
    let called = builder.ins().call_indirect(sig_ref, code, &given);
    let status = builder.inst_results(called)[0];
    Ok(status_or_answer(builder, abort, status, out, answers))
}

/// Where an indirect call's answer is written through, and where a direct one's is: a stack slot
/// this function owns, wide enough for one slot, read back once the callee has answered
/// `ANSWERED`.
fn out_slot(builder: &mut FunctionBuilder) -> ir::Value {
    let slot = builder.create_sized_stack_slot(ir::StackSlotData::new(
        ir::StackSlotKind::ExplicitSlot,
        SLOT as u32,
        0,
    ));
    builder.ins().stack_addr(POINTER, slot, 0)
}

/// What every call in this file does with a callee's answer: read the status back, forward
/// anything other than `ANSWERED` to `abort` exactly as it arrived, and load the value through
/// `out` only once the status says it is there.
///
/// Shared by a direct call ([`call_reached`]) and an indirect one (a closure's own `Apply`), since
/// forwarding a callee's abort transparently is not a fact about which of the two reached it.
fn status_or_answer(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    status: ir::Value,
    out: ir::Value,
    answers: types::Type,
) -> ir::Value {
    let ok = builder.create_block();
    let bad = builder.create_block();
    let answered = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    let is_ok = builder.ins().icmp(IntCC::Equal, status, answered);
    builder.ins().brif(is_ok, ok, &[], bad, &[]);
    builder.seal_block(ok);
    builder.seal_block(bad);

    builder.switch_to_block(bad);
    builder.ins().jump(abort, &[status.into()]);

    builder.switch_to_block(ok);
    builder.ins().load(answers, TRUSTED, out, 0)
}

/// What the document's numbers for a behavior's bindings stand for here.
///
/// Held by the number rather than pushed in the order they are met: the writer numbers a binder
/// where it writes it and this lowers a binding's value before the binder exists, so an order
/// either side happened to have would only agree until a binding's value held a binding of its own.
#[derive(Default)]
struct Bindings {
    held: Vec<Option<Variable>>,
}

impl Bindings {
    fn at(&mut self, number: usize, variable: Variable) {
        if self.held.len() <= number {
            self.held.resize(number + 1, None);
        }
        self.held[number] = Some(variable);
    }

    fn of(&self, number: usize) -> Result<Variable> {
        self.held
            .get(number)
            .copied()
            .flatten()
            .ok_or_else(|| anyhow!("a read of binding {number}, which nothing bound"))
    }
}

fn lower(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    abort: ir::Block,
    node: &Node,
) -> Result<ir::Value> {
    Ok(match node {
        Node::Int { value, ty, .. } => builder.ins().iconst(machine_type(ty)?, *value),
        Node::Read { binding, .. } => {
            let variable = bindings.of(*binding)?;
            builder.use_var(variable)
        }
        Node::Neg { operand, .. } if matches!(operand.as_ref(), Node::Int { .. }) => {
            // A negated literal — `-5` crosses as `Neg(Int(5))` rather than folded, because
            // ProgramWriter projects the checker's Core node for node and a literal's own sign
            // is not something a writer decides. Every `$row` helper the checker builds for a
            // negative example value is one of these (see `ProgramWriter`'s own doc on what a
            // helper is), so this is not a rare shape: refusing it would refuse most programs
            // with a negative literal anywhere in an `example`, for a reason that has nothing to
            // do with the runtime overflow question below.
            //
            // It needs no check and no trust in program.abortsAt either way: what the parser
            // wrote as a literal's magnitude is already an i64 in range, and negating anything in
            // [0, i64::MAX] is an i64 in range too — the one value this could not represent,
            // `-Int.MIN`, is not one this literal's own token could have named in the first
            // place. So this folds it here rather than routing a compile-time fact through a
            // runtime sign-bit check.
            let Node::Int { value, ty, .. } = operand.as_ref() else {
                unreachable!("matched above");
            };
            builder.ins().iconst(machine_type(ty)?, -*value)
        }
        Node::Neg {
            operand, aborts, ..
        } => {
            // `program.abortsAt` answers AbortSet.NONE for Core.Neg today (souther-lang/souther
            // #1878), citing only the JVM backend's own codegen — which is exactly the kind of
            // backend-specific re-derivation issue #9 exists to stop this file from doing on its
            // own account. So this does not trust it the way every other arithmetic site here
            // trusts what it is given: `-Int.MIN` overflows for the same representational reason
            // `Int.MIN - 1` does, and answering it as though it did not would be a wrong value
            // returned as a right one — worse than refusing a program this backend can lower
            // correctly once #1878 is resolved. (A literal operand does not reach here at all —
            // see the arm above — so this is only ever a variable's own value.)
            if aborts.is_empty() {
                return Err(not_lowered(
                    "a negation of something other than a literal, whose overflow this backend \
                     does not yet trust program.abortsAt for — see souther-lang/souther#1878",
                ));
            }
            let held = lower(builder, lowering, module, bindings, abort, operand)?;
            let nought = builder.ins().iconst(machine_type(operand.ty())?, 0);
            difference(builder, abort, overflow_status(aborts)?, nought, held)?
        }
        Node::Let {
            binding,
            value,
            body,
            ..
        } => {
            let held = lower(builder, lowering, module, bindings, abort, value)?;
            let variable = builder.declare_var(machine_type(value.ty())?);
            builder.def_var(variable, held);
            bindings.at(*binding, variable);
            lower(builder, lowering, module, bindings, abort, body)?
        }
        Node::Bool { value, ty, .. } => builder.ins().iconst(machine_type(ty)?, i64::from(*value)),
        Node::Str { value, .. } => text_in_the_object(builder, module, lowering.literals, value)?,
        Node::Binary {
            op,
            left,
            right,
            aborts,
            ..
        } => binary(
            builder,
            lowering,
            module,
            bindings,
            abort,
            *op,
            Operands {
                left,
                right,
                aborts,
            },
        )?,
        Node::If {
            cond,
            then,
            els,
            ty,
            ..
        } => {
            let asked = lower(builder, lowering, module, bindings, abort, cond)?;
            let answers = machine_type(ty)?;
            fork(builder, asked, answers, |builder, taken| {
                lower(
                    builder,
                    lowering,
                    module,
                    bindings,
                    abort,
                    if taken { then } else { els },
                )
            })?
        }
        Node::Unit { declared, .. } => {
            let flags = TRUSTED;
            let value = lowering.room(builder, module, room_for_fields(0));
            let which = tag_of(builder, lowering, module, declared)?;
            builder.ins().store(flags, which, value, WHICH as i32);
            value
        }
        Node::Construct {
            declared, values, ..
        } => {
            let shape = lowering.declared.shape(declared)?;
            // A construction runs the type's clauses and stops at the first that does not hold,
            // which is an abort and not a value. Nothing here runs one, and building the value
            // without running them would make a type's invariant true of what this emits by
            // omission.
            if shape.invariants() > 0 {
                return Err(not_lowered(format!(
                    "a construction of {declared}, which states what every one of its values owes"
                )));
            }
            if shape.field_count() != values.len() {
                bail!(
                    "{declared} is declared with {} fields and is built here from {}",
                    shape.field_count(),
                    values.len()
                );
            }
            // What each field carries is what a boundary reads its slot as, and what is put in it
            // here is what the slot holds. The two are one fact crossed twice, and an `Int` and a
            // `String` are one slot wide, so a disagreement would be written out and not caught.
            for (field, value) in shape.fields().iter().zip(values) {
                if !lowering.declared.carries(&field.codec, value.ty())? {
                    bail!(
                        "{declared}'s field {} carries {} and is built here from {}: the two \
                         halves disagree about what it holds",
                        field.name,
                        field.codec.ty().spelt(),
                        value.ty().spelt()
                    );
                }
            }
            // The fields are worked out before any room is taken, because working one out can
            // take room of its own and what is half-written is not a value.
            let mut held = Vec::with_capacity(values.len());
            for value in values {
                let answered = lower(builder, lowering, module, bindings, abort, value)?;
                held.push(into_slot(builder, answered));
            }
            let flags = TRUSTED;
            let value = lowering.room(builder, module, room_for_fields(values.len()));
            let which = tag_of(builder, lowering, module, declared)?;
            builder.ins().store(flags, which, value, WHICH as i32);
            for (at, field) in held.into_iter().enumerate() {
                builder
                    .ins()
                    .store(flags, field, value, field_at(at) as i32);
            }
            value
        }
        Node::Field {
            target, field, ty, ..
        } => {
            let of = target.ty();
            let Ty::Declared { declared } = of else {
                bail!("a field of {}, which holds no fields", of.spelt());
            };
            let shape = lowering.declared.shape(declared)?;
            let at = shape
                .position_of(field)
                .ok_or_else(|| anyhow!("{declared} declares no field {field}"))?;
            let carries = &shape.fields()[at].codec;
            if !lowering.declared.carries(carries, ty)? {
                bail!(
                    "{declared}'s field {field} carries {} and is read here as {}: the two halves \
                     disagree about what it holds",
                    carries.ty().spelt(),
                    ty.spelt()
                );
            }
            let value = lower(builder, lowering, module, bindings, abort, target)?;
            let flags = TRUSTED;
            let held = builder
                .ins()
                .load(types::I64, flags, value, field_at(at) as i32);
            out_of_slot(builder, held, machine_type(ty)?)
        }
        Node::Match {
            subject, arms, ty, ..
        } => {
            let value = lower(builder, lowering, module, bindings, abort, subject)?;
            fork_on_what_it_is(
                builder,
                lowering,
                module,
                bindings,
                abort,
                value,
                ForkArms {
                    arms,
                    answers: machine_type(ty)?,
                },
            )?
        }
        Node::Some { value, .. } => {
            let held = lower(builder, lowering, module, bindings, abort, value)?;
            let held = into_slot(builder, held);
            let flags = TRUSTED;
            let holding = lowering.room(builder, module, room_for_held());
            builder.ins().store(flags, held, holding, HELD as i32);
            holding
        }
        Node::None { .. } => builder.ins().iconst(POINTER, NOTHING),
        Node::Tuple { members, .. } => {
            let mut held = Vec::with_capacity(members.len());
            for member in members {
                let answered = lower(builder, lowering, module, bindings, abort, member)?;
                held.push(into_slot(builder, answered));
            }
            let flags = TRUSTED;
            let value = lowering.room(builder, module, room_for_members(members.len()));
            for (at, member) in held.into_iter().enumerate() {
                builder
                    .ins()
                    .store(flags, member, value, member_at(at) as i32);
            }
            value
        }
        Node::Call {
            reaches,
            arguments,
            ty,
            aborts,
        } => match reaches {
            // Four different references, and one thing done for all of them: look up the FuncId
            // the declare phase gave the method this call reaches, and call it with whatever
            // arguments the checker already threaded through — a value is not a special case
            // here. `Reaches::Value` reaches a method of this same object exactly the way a
            // helper's copy does (souther's JVM backend calls it through the identical path a
            // recursive helper's own call is); `Reaches::PublishedValue` reaches an entry across
            // an object boundary the same way a behavior implemented elsewhere does. Neither
            // needs a runtime cache: "runs once" is ADR-0074's checker-level guarantee that the
            // region reading a value's reference builds each dependency once, which is what
            // `arguments` already carries in from the handovers `ProgramWriter` threaded — not
            // something this side re-derives or memoizes.
            Reaches::Helper { .. }
            | Reaches::Behavior { .. }
            | Reaches::Value { .. }
            | Reaches::PublishedValue { .. } => {
                let reached = match reaches {
                    Reaches::Helper { declared } => {
                        lowering.reachable.of_held(lowering.carrier, declared)?
                    }
                    Reaches::Behavior { declared } => {
                        lowering.reachable.of_behavior_named(declared)?
                    }
                    Reaches::Value { module, name } => lowering
                        .reachable
                        .of_value(lowering.carrier, &format!("{module}.{name}"))?,
                    Reaches::PublishedValue { module, name } => {
                        lowering.reachable.of_published_value(module, name)?
                    }
                    Reaches::Kernel { .. } => unreachable!(),
                };
                let mut given = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    given.push(lower(builder, lowering, module, bindings, abort, argument)?);
                }
                call_reached(builder, module, abort, reached, machine_type(ty)?, &given)?
            }
            // Which kernels this backend already answers instructions for is this match's own
            // list and nowhere else's — kept short on purpose, so a kernel this has not met yet
            // falls straight through to NotLowered rather than a table here claiming to know.
            //
            // A kernel this arm does recognise but that arrived with the wrong number of
            // arguments is not that: the language does not admit `int.add` at any arity but two,
            // so a document naming one anyway is not the language ahead of this backend — it is
            // this driver's own reading of the transport disagreeing with what `KernelContract`
            // declared, the same halves-disagreeing failure every other shape mismatch here bails
            // on rather than reports as this backend not having gotten round to a program yet.
            Reaches::Kernel { kernel } => match kernel.as_str() {
                "int.add" => {
                    if arguments.len() != 2 {
                        bail!(
                            "the kernel int.add reached this driver with {} arguments rather \
                             than the two its own contract declares",
                            arguments.len()
                        );
                    }
                    let a = Held::of(
                        &arguments[0],
                        lower(builder, lowering, module, bindings, abort, &arguments[0])?,
                    );
                    let b = Held::of(
                        &arguments[1],
                        lower(builder, lowering, module, bindings, abort, &arguments[1])?,
                    );
                    arithmetic(builder, abort, Op::Add, a, b, aborts)?
                }
                _ => return Err(not_lowered(format!("a call to the kernel {kernel}"))),
            },
        },
        Node::Member { tuple, at, ty, .. } => {
            let value = lower(builder, lowering, module, bindings, abort, tuple)?;
            let flags = TRUSTED;
            let held = builder
                .ins()
                .load(types::I64, flags, value, member_at(*at) as i32);
            out_of_slot(builder, held, machine_type(ty)?)
        }
        Node::Block { site, .. } => {
            let plan = lowering
                .closures
                .site(*site)
                .ok_or_else(|| anyhow!("closure site {site}, which nothing planned"))?;
            let code_id = *lowering.lifted.get(site).ok_or_else(|| {
                anyhow!("closure site {site}, which no lifted function was declared for")
            })?;

            let flags = TRUSTED;
            let value = lowering.room(builder, module, room_for_closure(plan.captures.len()));

            let code_ref = module.declare_func_in_func(code_id, builder.func);
            let code = builder.ins().func_addr(POINTER, code_ref);
            builder.ins().store(flags, code, value, CLOSURE_CODE as i32);

            for (position, capture) in plan.captures.iter().enumerate() {
                let variable = bindings.of(capture.binding)?;
                let held = builder.use_var(variable);
                let held = into_slot(builder, held);
                builder
                    .ins()
                    .store(flags, held, value, capture_at(position) as i32);
            }
            value
        }
        Node::Apply {
            function,
            arguments,
            ty,
            ..
        } => {
            let Ty::Fn { fn_ } = function.ty() else {
                bail!(
                    "an application of {}, which is not a function type",
                    function.ty().spelt()
                );
            };
            // `fn_` (the applied function's own type) and `arguments`/`ty` (this `Apply` node's
            // own arguments and own type) are independent statements of one fact, the same way a
            // `Node::Block`'s own type, parameters and body are (see `closures`'s own doc) — and
            // this one is never checked before now, since nothing builds a `Site` for an `Apply`.
            // Checked before any of this node's own operands are lowered, so a document that fails
            // this never leaves behind half-lowered IR for it.
            //
            // The answer and the arguments are held to two different standards, on purpose. `ty`
            // is not this call's own decision the way an ordinary call's answer type is derived
            // from a signature elsewhere — souther's checker builds `Core.Apply`'s own `type`
            // straight from the applied local's `Type.FnOf`, through `applySignature()`'s result,
            // with no assignability in between (`Core.Apply`'s own construction upstream). So
            // `fn_.answers` and `ty` are one type fact written twice, exactly the way a
            // `Node::Block`'s own `answers` and its `body`'s type are — held to full `Ty` equality
            // there and held to it here for the same reason. An argument against a parameter is a
            // different question: the language's own assignability may legitimately hand a wider
            // argument type to a narrower parameter, which this backend has no business
            // re-deciding, so those are checked at machine representation only — the one thing an
            // indirect call's own ABI actually needs to agree about. Loosening the answer check to
            // machine representation, the way the arguments are, would let two different declared
            // types that happen to share one representation (both `POINTER`) pass a document where
            // they disagree: the closure would store one type's address into `out` and the caller
            // would read it back as the other — not a crash, since Cranelift has nothing to object
            // to, just the wrong type read from a real address from then on.
            if fn_.takes.len() != arguments.len() {
                bail!(
                    "an application naming {} arguments to a function type taking {}: `Apply`'s \
                     own arguments and its function's own type are two statements of one fact and \
                     this document's disagree",
                    arguments.len(),
                    fn_.takes.len()
                );
            }
            if fn_.answers.as_ref() != ty {
                bail!(
                    "an application answering {} at its function's own type and {} at its own \
                     type: the two are statements of one fact and this document's disagree",
                    fn_.answers.spelt(),
                    ty.spelt()
                );
            }
            for (at, taken) in fn_.takes.iter().enumerate() {
                if machine_type(taken)? != machine_type(arguments[at].ty())? {
                    bail!(
                        "an application's argument {at} is {} at its function's own type and {} \
                         where it is written: the two have no representation in common, and this \
                         document's two statements of what is handed over disagree",
                        taken.spelt(),
                        arguments[at].ty().spelt()
                    );
                }
            }
            let closure = lower(builder, lowering, module, bindings, abort, function)?;
            let mut given = Vec::with_capacity(arguments.len());
            for argument in arguments {
                given.push(lower(builder, lowering, module, bindings, abort, argument)?);
            }
            let call_conv = module.isa().default_call_conv();
            call_indirect_reached(
                builder,
                abort,
                closure,
                call_conv,
                &fn_.takes,
                machine_type(ty)?,
                &given,
            )?
        }
    })
}

/// A fork on what a value is, arm by arm.
///
/// The arms are tried in the order they are written, because that is the order the language reads
/// them in. What an arm tests is what the checker resolved it to and not the name it was written
/// under, so a case that is itself a sum arrives here as the several types it stands for.
///
/// Running out of arms is this compiler having emitted the wrong test: the checker settles that a
/// fork always answers, so nothing a program can be written as reaches the end of this.
/// What `fork_on_what_it_is` asks over, beyond the four it already threads through every call a
/// lowering makes: which arms, and the width the fork as a whole answers at. Bundled so this stays
/// within the width every function here is held to instead of adding a sixth thing this and
/// `lower` would otherwise both have to keep passing down separately.
struct ForkArms<'a> {
    arms: &'a [Arm],
    answers: types::Type,
}

fn fork_on_what_it_is(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    abort: ir::Block,
    value: ir::Value,
    over: ForkArms,
) -> Result<ir::Value> {
    let ForkArms { arms, answers } = over;
    let after = builder.create_block();
    builder.append_block_param(after, answers);

    for arm in arms {
        let taken = builder.create_block();
        let next = builder.create_block();
        let asked = tests(builder, lowering, module, value, &arm.selects)?;
        builder.ins().brif(asked, taken, &[], next, &[]);
        builder.seal_block(taken);
        builder.seal_block(next);

        builder.switch_to_block(taken);
        if let Some(number) = arm.binding {
            let read_as = arm.binds.as_ref().ok_or_else(|| {
                anyhow!("an arm binds a value and does not say what it reads it as")
            })?;
            let held = binds(builder, value, &arm.selects, machine_type(read_as)?);
            let variable = builder.declare_var(machine_type(read_as)?);
            builder.def_var(variable, held);
            bindings.at(number, variable);
        }
        let answered = lower(builder, lowering, module, bindings, abort, &arm.body)?;
        builder.ins().jump(after, &[answered.into()]);

        builder.switch_to_block(next);
    }

    builder
        .ins()
        .trap(TrapCode::user(NO_ARM).expect("a trap code of its own"));

    builder.seal_block(after);
    builder.switch_to_block(after);
    Ok(builder.block_params(after)[0])
}

/// Whether the value is one of the cases this arm answers for.
///
/// What a value says it is and what a case is are both the address of a declaration's token, so
/// this is a comparison of two addresses. The one the value carries was written where it was built
/// — possibly in an object built from another document — and the one compared against is named
/// here; they are equal exactly when the linker resolved both to the one declaration, which is
/// what makes the answer mean the same thing on either side of an object boundary.
fn tests(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    value: ir::Value,
    selects: &[Selects],
) -> Result<ir::Value> {
    let mut asked: Option<ir::Value> = None;
    for one in selects {
        let this = match one {
            Selects::Which { atoms } => {
                is_one_of_declared_cases(builder, lowering, module, value, atoms)?
            }
            Selects::Held => builder.ins().icmp_imm_s(IntCC::NotEqual, value, NOTHING),
            Selects::Nothing => builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING),
        };
        asked = Some(match asked {
            None => this,
            Some(before) => builder.ins().bor(before, this),
        });
    }
    asked.ok_or_else(|| anyhow!("an arm answers for at least one case"))
}

/// Whether the value is one of these declared cases.
///
/// What a value says it is and what a case is are both the address of a declaration's token, so
/// this is a comparison of two addresses. The one the value carries was written where it was built
/// — possibly in an object built from another document — and the one compared against is named
/// here; they are equal exactly when the linker resolved both to the one declaration, which is
/// what makes the answer mean the same thing on either side of an object boundary.
///
/// Shared by a `match` arm testing what a value is and a composition's routing testing what a
/// stage accepts: a composition's routing is that same test at a different place, not a second
/// kind of test, and the primitive both read is the one Issue #6 settled — a type token's address
/// compared as the linker resolves it.
fn is_one_of_declared_cases(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    value: ir::Value,
    cases: &[Case],
) -> Result<ir::Value> {
    let flags = TRUSTED;
    let which = builder.ins().load(POINTER, flags, value, WHICH as i32);
    let mut any: Option<ir::Value> = None;
    for case in cases {
        let Case::Declared { declared } = case else {
            return Err(not_lowered(format!(
                "a test for the case {}, which carries no token to compare",
                case.spelt()
            )));
        };
        let expected = tag_of(builder, lowering, module, declared)?;
        let same = builder.ins().icmp(IntCC::Equal, which, expected);
        any = Some(match any {
            None => same,
            Some(before) => builder.ins().bor(before, same),
        });
    }
    any.ok_or_else(|| anyhow!("a routing offers no case for a stage to accept"))
}

/// What the arm reads the value as, once it is known to be one of its cases.
///
/// An arm over an optional's present carrier reads what it holds; every other arm reads the value
/// itself, which is already the case it selected. What comes out of the slot is narrowed to what
/// the arm says it reads the value as — which the arm carries, because the test it was selected by
/// does not say it.
fn binds(
    builder: &mut FunctionBuilder,
    value: ir::Value,
    selects: &[Selects],
    read_as: types::Type,
) -> ir::Value {
    if selects.iter().any(|it| matches!(it, Selects::Held)) {
        let held = builder.ins().load(types::I64, TRUSTED, value, HELD as i32);
        out_of_slot(builder, held, read_as)
    } else {
        value
    }
}

/// A value that is one of two, worked out on the side the condition took.
///
/// Both sides are written and only one runs, which is what makes this a fork rather than a choice
/// between two values: what the arm not taken would have computed is not computed, and for an arm
/// that aborts that is the difference between a run that answers and one that does not.
fn fork<A>(
    builder: &mut FunctionBuilder,
    asked: ir::Value,
    answers: types::Type,
    mut arm: A,
) -> Result<ir::Value>
where
    A: FnMut(&mut FunctionBuilder, bool) -> Result<ir::Value>,
{
    let when_taken = builder.create_block();
    let otherwise = builder.create_block();
    let after = builder.create_block();
    builder.append_block_param(after, answers);

    builder.ins().brif(asked, when_taken, &[], otherwise, &[]);
    builder.seal_block(when_taken);
    builder.seal_block(otherwise);

    for (block, taken) in [(when_taken, true), (otherwise, false)] {
        builder.switch_to_block(block);
        let answered = arm(builder, taken)?;
        builder.ins().jump(after, &[answered.into()]);
    }

    builder.seal_block(after);
    builder.switch_to_block(after);
    Ok(builder.block_params(after)[0])
}

/// Every operator the language has is named, and the ones with no lowering say so.
///
/// Named one by one rather than caught by an arm that stands for the rest: an operator added to the
/// language is then something this has to answer for, which is the only way the list stays the
/// language's rather than this file's memory of it.
///
/// The operands are lowered here and not before, because two of these decide whether the right one
/// runs at all.
/// The two operands of a binary operator, plus the one fact `arithmetic` needs and no other arm
/// of `op` does: which reason (if any) this exact site may end without a value for. Bundled with
/// the operands rather than threaded as a fourth thing beside them, since a caller already has all
/// three off one `Node::Binary`.
struct Operands<'a> {
    left: &'a Node,
    right: &'a Node,
    aborts: &'a [AbortKind],
}

fn binary(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    abort: ir::Block,
    op: Op,
    operands: Operands,
) -> Result<ir::Value> {
    let Operands {
        left,
        right,
        aborts,
    } = operands;
    match op {
        // `&&` and `||` stop as soon as the answer is settled, and which operands run is part of
        // what they mean rather than something a backend decides: a condition narrows what its
        // right side may compute, and run eagerly the right side would abort at the value the
        // condition exists to exclude.
        Op::And | Op::Or => {
            let settles_it = matches!(op, Op::Or);
            let asked = lower(builder, lowering, module, bindings, abort, left)?;
            // What the left one is, which is what the whole of it is: a condition answers what its
            // operands answer, and reading that off the operand rather than knowing it here keeps
            // the width a fact that crossed.
            let answers = machine_type(left.ty())?;
            fork(builder, asked, answers, |builder, taken| {
                if taken == settles_it {
                    Ok(builder.ins().iconst(answers, i64::from(settles_it)))
                } else {
                    lower(builder, lowering, module, bindings, abort, right)
                }
            })
        }
        _ => {
            // Both of the operands' types, because one of them does not say what the other is.
            // A bare literal takes the newtype of the value it is compared with, so `0 == amount`
            // is an `Int` against a declared type and is as much a comparison of two amounts as
            // `amount == 0` is; a case value compared with its sum is two declared types that are
            // not the same one. Read off the left alone, both of those are whatever the left one
            // happened to be.
            let a = Held::of(
                left,
                lower(builder, lowering, module, bindings, abort, left)?,
            );
            let b = Held::of(
                right,
                lower(builder, lowering, module, bindings, abort, right)?,
            );
            match op {
                Op::Add | Op::Sub | Op::Mul => arithmetic(builder, abort, op, a, b, aborts),
                Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                    compare(builder, lowering, module, op, a, b)
                }
                // `/` answers the exact quotient, which is not a whole number and has no
                // representation here yet.
                Op::Div => Err(not_lowered(format!("the operator {}", op.spelt()))),
                Op::Concat => join(builder, lowering, module, a, b),
                Op::And | Op::Or => {
                    unreachable!("answered above, where the right side may not run")
                }
            }
        }
    }
}

/// What a comparison compares.
///
/// Decided by the types the operands have in Souther and not by the widths they are held in. The
/// two agree for an `Int` and for a `Bool`, and that agreement is the whole reason every comparison
/// could be one `icmp` until now. It does not hold past them. A value of a declared type is held as
/// the address of what it is made of, and Souther's `==` over one of those is its fields compared
/// one by one — so an `icmp` over the two addresses answers whether they are the same value rather
/// than whether they are equal, and says false of two that were built separately.
///
/// Both types and not the left one, because an operator here is not given two values of one type.
/// A bare literal takes the newtype of the operand it is compared with, and a case value is a value
/// of its sum, so a legitimate comparison arrives with `Int` on one side and a declared type on the
/// other, or with two declared types that are not the same one. Read off the left alone, `0 ==
/// amount` would be compared as two `Int`s — one of which is an address — while `amount == 0`
/// was refused, and which of the two a program got would be the order its author wrote them in.
///
/// So what is refused here was being answered wrongly before, which is why it is refused rather
/// than left. An object that links and answers is what a wrong answer comes out of.
fn compare(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    op: Op,
    left: Held,
    right: Held,
) -> Result<ir::Value> {
    let (a, b) = (left.value, right.value);
    match (left.ty, right.ty) {
        // Every primitive is named, for the reason `machine_type` names them: one added to the
        // language would otherwise arrive here and be compared as whatever it is held as.
        (Ty::Prim { prim }, Ty::Prim { prim: also }) if prim == also => match prim {
            Prim::Int => Ok(builder.ins().icmp(as_a_whole_number(op), a, b)),
            // Two truths are equal or they are not, and nothing orders them. `<` over a `Bool` is
            // not a program the language admits, so one arriving is the two halves disagreeing
            // about what they are saying to each other rather than this backend being behind.
            Prim::Bool => match op {
                Op::Eq => Ok(builder.ins().icmp(IntCC::Equal, a, b)),
                Op::Ne => Ok(builder.ins().icmp(IntCC::NotEqual, a, b)),
                _ => bail!(
                    "a Bool is not ordered, and {} is written over two of them here",
                    op.spelt()
                ),
            },
            // One call and then the six operators over what it answered. What text is ordered by
            // is the runtime's to say and it is a walk, not a comparison of the two addresses and
            // not a comparison of the bytes either.
            Prim::String => {
                let comparing = module.declare_func_in_func(lowering.compare_text, builder.func);
                let compared = builder.ins().call(comparing, &[a, b]);
                let answered = builder.inst_results(compared)[0];
                Ok(builder.ins().icmp_imm_s(as_a_whole_number(op), answered, 0))
            }
            Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => Err(not_lowered(format!(
                "a comparison of two values of type {}",
                prim.spelt()
            ))),
        },
        // A declared type on either side, which covers every legitimate comparison whose operands
        // are not two values of one primitive: two values of one declared type, a value against a
        // bare literal of what its newtype wraps, and a sum against one of its cases. What each of
        // them comes to is the fields compared one by one, the wrapped value compared, or which
        // case the value is — and none of those is written here, while the address a value is held
        // as answers none of them.
        //
        // Named together rather than told apart, because what tells them apart is not in the
        // document: a newtype says what it is called and what its field is called, and not what it
        // wraps. A reading that guessed would be this side deciding a question the checker has
        // already answered.
        (Ty::Declared { .. } | Ty::Union { .. }, _)
        | (_, Ty::Declared { .. } | Ty::Union { .. }) => Err(not_lowered(format!(
            "a comparison of {} against {}, which is what they are made of compared rather \
                 than where they are",
            left.ty.spelt(),
            right.ty.spelt()
        ))),
        // An optional and a tuple have equality and no order: what the language orders is a number,
        // text, an amount, a moment, an enumeration, and a newtype over one of those. So `==` here
        // is a comparison still to be written, and `<` is the two halves disagreeing — the same
        // pair of answers a `Bool` gets, and for the same reason.
        (Ty::Option { .. }, Ty::Option { .. }) | (Ty::Tuple { .. }, Ty::Tuple { .. }) => match op {
            Op::Eq | Op::Ne => Err(not_lowered(format!(
                "a comparison of {} against {}, which is what they hold compared rather than \
                 where they are",
                left.ty.spelt(),
                right.ty.spelt()
            ))),
            _ => bail!(
                "{} is not ordered, and {} is written over two of them here",
                left.ty.spelt(),
                op.spelt()
            ),
        },
        // Two values the language does not compare at all: two primitives that are not one
        // primitive, or a tuple against an optional. Only values of one type are compared, and the
        // two ways that is widened — a bare literal and a case value — are both answered above.
        _ => bail!(
            "{} is compared with {} here, which the language does not compare",
            left.ty.spelt(),
            right.ty.spelt()
        ),
    }
}

/// A sum, a difference or a product, over the types the operands have in Souther.
///
/// The same question the comparisons ask, asked of the arithmetic because the answer is not
/// obviously the same. The language admits `+` and `-` over a single-value newtype, and a value of
/// one is held here as the address of what it is made of — so were one to arrive, an `iadd` over
/// two of them would answer an address that points at neither.
///
/// One does not arrive: what crosses for `a + b` over a newtype is already a construction of the
/// newtype over the sum of what the two wrap, and that holds of `a + 1` too — the literal is added
/// to the wrapped number and not to the value. So the operands are two `Int`s by the time this
/// reads them. That is the checker's arrangement and not this driver's, which is why anything else
/// is the two halves disagreeing rather than a lowering that is still to be written.
fn arithmetic(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    op: Op,
    left: Held,
    right: Held,
    aborts: &[AbortKind],
) -> Result<ir::Value> {
    let (a, b) = (left.value, right.value);
    match (left.ty, right.ty) {
        (Ty::Prim { prim }, Ty::Prim { prim: also }) if prim == also => match prim {
            Prim::Int => match op {
                Op::Add => {
                    let sum = builder.ins().iadd(a, b);
                    let past = builder.ins().bxor(a, sum);
                    let also = builder.ins().bxor(b, sum);
                    abort_where_the_sign_bit_is_set(
                        builder,
                        abort,
                        overflow_status(aborts)?,
                        past,
                        also,
                    );
                    Ok(sum)
                }
                Op::Sub => difference(builder, abort, overflow_status(aborts)?, a, b),
                Op::Mul => product(builder, abort, overflow_status(aborts)?, a, b),
                _ => unreachable!("reached from a sum, a difference or a product and nothing else"),
            },
            Prim::Decimal => Err(not_lowered(format!(
                "{} over two values of type {}",
                op.spelt(),
                prim.spelt()
            ))),
            // The language writes no arithmetic over any of these, so one arriving is the two
            // halves disagreeing rather than a representation that is still to be designed.
            Prim::Bool
            | Prim::String
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => bail!(
                "{} is written over two values of type {}, which the language does not add",
                op.spelt(),
                prim.spelt()
            ),
        },
        _ => bail!(
            "{} is written over {} and {}, which reaches this driver as arithmetic over what they \
             are made of or does not reach it at all",
            op.spelt(),
            left.ty.spelt(),
            right.ty.spelt()
        ),
    }
}

/// Two values joined, which the language writes over two strings and over two lists.
///
/// A list has no type this writer can name yet, so nothing but two strings reaches here; the rest
/// is the two halves disagreeing rather than a join that is still to be written.
fn join(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    left: Held,
    right: Held,
) -> Result<ir::Value> {
    let (a, b) = (left.value, right.value);
    match (left.ty, right.ty) {
        (Ty::Prim { prim }, Ty::Prim { prim: also }) if prim == also => match prim {
            Prim::String => {
                let joining = module.declare_func_in_func(lowering.join_text, builder.func);
                let joined = builder.ins().call(joining, &[a, b]);
                Ok(builder.inst_results(joined)[0])
            }
            Prim::Int
            | Prim::Bool
            | Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => bail!(
                "two values of type {} are joined here, which the language does not join",
                prim.spelt()
            ),
        },
        _ => bail!(
            "{} is joined with {} here, which the language does not join",
            left.ty.spelt(),
            right.ty.spelt()
        ),
    }
}

/// Every string literal this object holds, one per text however many places spell it.
///
/// Held for the whole object rather than asked of each site, because a site is not what a literal
/// is: two places spelling one text are one literal, and data declared per site would be the same
/// bytes written as many times as the program says them.
#[derive(Default)]
struct Literals {
    held: RefCell<HashMap<String, DataId>>,
}

/// A string the object carries, and the address of it.
///
/// A literal says the same text every run, so it is written into the object rather than worked out
/// into the arena. What comes back is the address of a string like any other: a comparison and a
/// join read it the way they read one a run made, and nothing in the value says which of the two
/// it is. That is what keeps where a string is kept out of what a string means.
///
/// One data object per text, shared by every site that spells it ([`Literals`]), and anonymous
/// because nothing outside this object reaches one.
fn text_in_the_object(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    literals: &Literals,
    value: &str,
) -> Result<ir::Value> {
    let already = literals.held.borrow().get(value).copied();
    let id = match already {
        Some(id) => id,
        None => {
            let id = literal(module, value)?;
            literals.held.borrow_mut().insert(value.to_string(), id);
            id
        }
    };
    let named = module.declare_data_in_func(id, builder.func);
    Ok(builder.ins().symbol_value(POINTER, named))
}

/// A literal's bytes, laid out as the runtime lays a string out, defined once in the object.
fn literal(module: &mut ObjectModule, value: &str) -> Result<DataId> {
    let length = i64::try_from(value.len()).expect("a literal is shorter than an Int");
    let mut written = vec![0u8; room_for_text(length) as usize];
    written[TEXT_LENGTH as usize..][..SLOT as usize].copy_from_slice(&length.to_ne_bytes());
    written[TEXT_BYTES as usize..].copy_from_slice(value.as_bytes());

    let mut held = DataDescription::new();
    held.define(written.into_boxed_slice());
    // Aligned as everything the arena answers is. The count before the text is read as a slot, and
    // every access this emits says the address is aligned rather than checking that it is.
    held.set_align(SLOT as u64);
    let id = module.declare_anonymous_data(false, false)?;
    module.define_data(id, &held)?;
    Ok(id)
}

/// Which machine condition one of the six comparisons is, over a signed whole number.
///
/// Every operator is named rather than the six being picked out and the rest left to an arm
/// standing for them, for the reason the primitives are named: one added to the language would
/// otherwise be answered for here by an arm written for something else.
fn as_a_whole_number(op: Op) -> IntCC {
    match op {
        Op::Eq => IntCC::Equal,
        Op::Ne => IntCC::NotEqual,
        Op::Lt => IntCC::SignedLessThan,
        Op::Le => IntCC::SignedLessThanOrEqual,
        Op::Gt => IntCC::SignedGreaterThan,
        Op::Ge => IntCC::SignedGreaterThanOrEqual,
        Op::And | Op::Or | Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Concat => {
            unreachable!("reached from a comparison and nothing else")
        }
    }
}

/// A subtraction that left the range an `Int` holds ends the computation.
///
/// The operands disagreeing in sign and the answer disagreeing with the left one is what that is.
/// A negation is this against nought, which is why the two are one function: negating the smallest
/// `Int` there is leaves the range exactly as any other subtraction does.
fn difference(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    status: Status,
    a: ir::Value,
    b: ir::Value,
) -> Result<ir::Value> {
    let difference = builder.ins().isub(a, b);
    let apart = builder.ins().bxor(a, b);
    let moved = builder.ins().bxor(a, difference);
    abort_where_the_sign_bit_is_set(builder, abort, status, apart, moved);
    Ok(difference)
}

/// A product that left the range an `Int` holds ends the computation.
///
/// Worked out at twice the width and held to what comes back when it is narrowed: the two agree
/// exactly when the product is one an `Int` holds. Said this way rather than as a division, which
/// has an operand pair of its own that no `Int` answers for.
fn product(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    status: Status,
    a: ir::Value,
    b: ir::Value,
) -> Result<ir::Value> {
    let a_wide = builder.ins().sextend(types::I128, a);
    let b_wide = builder.ins().sextend(types::I128, b);
    let wide = builder.ins().imul(a_wide, b_wide);
    let held = builder.ins().ireduce(types::I64, wide);
    let back = builder.ins().sextend(types::I128, held);
    let past = builder.ins().icmp(IntCC::NotEqual, wide, back);
    abort_where(builder, abort, status, past);
    Ok(held)
}

/// Ends the computation where both of these have their sign bit set.
///
/// What leaving the range looks like is two facts about signs holding at once, and which two
/// depends on the operation. Each caller works out its own pair and this is what they end on.
fn abort_where_the_sign_bit_is_set(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    status: Status,
    one: ir::Value,
    other: ir::Value,
) {
    let both = builder.ins().band(one, other);
    let past = builder.ins().ushr_imm_u(both, 63);
    abort_where(builder, abort, status, past);
}

/// A machine condition that leaves an `Int` outside the range it holds is this backend's own
/// invariant answering rather than the language's — the checker already settled that the site
/// carries exactly this one reason (see `overflow_status`) — so what happens when `condition` is
/// true is a jump to `abort` with that reason's status, and lowering carries on in a fresh block
/// for the case it is false. Not a trap: a trap is this compiler's own bug answering, and a Souther
/// `Int` leaving its range is not that — it is the language's own answer to the computation, and
/// `abort` is the one place every such answer in this function leaves through.
fn abort_where(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    status: Status,
    condition: ir::Value,
) {
    let past = builder.create_block();
    let ok = builder.create_block();
    builder.ins().brif(condition, past, &[], ok, &[]);
    builder.seal_block(past);
    builder.seal_block(ok);

    builder.switch_to_block(past);
    let code = builder.ins().iconst(types::I32, i64::from(status));
    builder.ins().jump(abort, &[code.into()]);

    builder.switch_to_block(ok);
}
