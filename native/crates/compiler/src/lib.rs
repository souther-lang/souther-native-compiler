//! Lowers what crossed to Cranelift IR, and lets Cranelift write the object.
//!
//! The object is for the machine this runs on. Choosing a target for another machine is a question
//! about linkers and a runtime built for it, and answering that before the code generation works
//! would be answering the easier question first.

pub mod transport;

use anyhow::{Result, anyhow, bail};
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
    TEXT_LENGTH, TOKEN, WHICH, behavior_symbol, example_symbol, field_at, held_symbol, member_at,
    room_for_fields, room_for_held, room_for_members, room_for_text, type_symbol,
};
use std::collections::HashMap;
use std::fmt;
use transport::{
    AbortKind, Answers, Arm, Declaration, DeclaredBy, Definition, Node, Op, Prim, Program,
    Publication, Reaches, Routing, Selects, Stage, TRANSPORT_VERSION, Target, Ty,
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
        if matches!(declaration, Declaration::Sum { .. })
            || declaration.by() != DeclaredBy::AModule
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
        agrees_with_its_target(name, target, local, &targets)?;
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
        let signature = signature_over(&target.takes, &target.answers, call_conv)?;
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
        for held in &written.helpers {
            let symbol = held_symbol(&written.name, &held.declared);
            let signature = signature_over(&held.takes, &held.answers, call_conv)?;
            // Held and not exported: a definition a module holds is that module's copy, and
            // nothing outside the object reaches one.
            let id = module.declare_function(&symbol, Linkage::Local, &signature)?;
            reachable.held(&written.name, &held.declared, id)?;
        }
        for example in &written.examples {
            let signature = running_a_row(&targets, &written.name, &example.behavior, call_conv)?;
            let symbol = example_symbol(&written.name, &example.behavior, example.at);
            // Reached from outside whatever the module says about the behavior's own name: what
            // this runs is a row, and a row of a kept name is as much a row as any other.
            let id = module.declare_function(&symbol, Linkage::Export, &signature)?;
            entries.insert(symbol, id);
        }
    }

    for written in &program.modules {
        for held in &written.helpers {
            let signature = signature_over(&held.takes, &held.answers, call_conv)?;
            let id = reachable.of_held(&written.name, &held.declared)?;
            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            let lowering = Lowering {
                declared: &declared,
                reachable: &reachable,
                carrier: &written.name,
                allocate,
                compare_text,
                join_text,
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
        for local in &written.definitions {
            match local {
                Definition::Body { declared: name, body, .. } => {
                    let target = targets.named(name)?;
                    let takes = &target.takes;
                    let signature = signature_over(&target.takes, &target.answers, call_conv)?;
                    let id = reachable.of_behavior_named(name)?;
                    context.clear();
                    context.func = Function::with_name_signature(UserFuncName::default(), signature);
                    let lowering = Lowering {
                        declared: &declared,
                        reachable: &reachable,
                        carrier: &written.name,
                        allocate,
                        compare_text,
                        join_text,
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
                    declared: name,
                    stages,
                    ..
                } => {
                    let target = targets.named(name)?;
                    let signature = signature_over(&target.takes, &target.answers, call_conv)?;
                    let id = reachable.of_behavior_named(name)?;
                    context.clear();
                    context.func = Function::with_name_signature(UserFuncName::default(), signature);
                    let lowering = Lowering {
                        declared: &declared,
                        reachable: &reachable,
                        carrier: &written.name,
                        allocate,
                        compare_text,
                        join_text,
                    };
                    define_composed(
                        &mut context.func,
                        &mut shapes,
                        target.takes.len(),
                        stages,
                        frontend,
                        &lowering,
                        &mut module,
                    )?;
                    module.define_function(id, &mut context)?;
                }
            }
        }
        for example in &written.examples {
            let signature = running_a_row(&targets, &written.name, &example.behavior, call_conv)?;
            let symbol = example_symbol(&written.name, &example.behavior, example.at);
            let id = *entries
                .get(&symbol)
                .ok_or_else(|| anyhow!("no entry was declared for {symbol}"))?;
            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            let lowering = Lowering {
                declared: &declared,
                reachable: &reachable,
                carrier: &written.name,
                allocate,
                compare_text,
                join_text,
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
) -> Result<()> {
    match (target.is, local) {
        (Answers::Body, Definition::Body { .. }) => Ok(()),
        (Answers::Composed, Definition::Composed { answers, stages, .. }) => {
            if answers != &target.answers {
                bail!(
                    "{name} answers {} as a composition and {} at the target that reaches it: \
                     the two halves disagree about what it answers",
                    answers.spelt(),
                    target.answers.spelt()
                );
            }
            let first = stages.first().ok_or_else(|| {
                anyhow!("{name} is a composition composing nothing")
            })?;
            if !matches!(first.routing, Routing::Always) {
                bail!(
                    "{name}'s first stage is routed rather than always applied: the first stage \
                     of a composition takes the composition's own arguments, and nothing is \
                     routed into it"
                );
            }
            let leads = targets.named(&first.behavior)?;
            if leads.takes != target.takes {
                bail!(
                    "{name} takes {} and its first stage {} takes {}: a composition takes \
                     whatever its first stage takes, and the two halves disagree about what \
                     that is",
                    spelt(&target.takes),
                    first.behavior,
                    spelt(&leads.takes)
                );
            }
            for stage in stages {
                let reached = targets.named(&stage.behavior)?;
                if stage.answers != reached.answers {
                    bail!(
                        "{}'s stage naming {} answers {} on the wire and {} at the target it \
                         reaches: the two halves disagree",
                        name,
                        stage.behavior,
                        stage.answers.spelt(),
                        reached.answers.spelt()
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
    signature_over(&[], &target.answers, call_conv)
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
#[derive(Default)]
struct Reachable {
    held: HashMap<(String, String), FuncId>,
    behaviors: HashMap<String, FuncId>,
}

impl Reachable {
    fn held(&mut self, carrier: &str, declared: &str, id: FuncId) -> Result<()> {
        let key = (carrier.to_string(), declared.to_string());
        if self.held.insert(key, id).is_some() {
            bail!("{carrier} holds two definitions both called {declared}");
        }
        Ok(())
    }

    fn behavior(&mut self, declared: &str, id: FuncId) -> Result<()> {
        if self.behaviors.insert(declared.to_string(), id).is_some() {
            bail!("two behaviors are both written {declared}");
        }
        Ok(())
    }

    fn of_held(&self, carrier: &str, declared: &str) -> Result<FuncId> {
        self.held
            .get(&(carrier.to_string(), declared.to_string()))
            .copied()
            .ok_or_else(|| anyhow!("{carrier} reaches {declared}, which it holds no copy of"))
    }

    fn of_behavior_named(&self, declared: &str) -> Result<FuncId> {
        self.behaviors
            .get(declared)
            .copied()
            .ok_or_else(|| anyhow!("a call reaching {declared}, which the program does not name"))
    }
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
        Ok(Declared { shapes })
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

/// What a value of this type is on the machine.
///
/// A number, a truth, or the address of what a value is made of. Every other primitive is a value
/// with a representation to design — how it is held, who owns it, what frees it — and none of that
/// is decided by giving it a width here.
fn machine_type(ty: &Ty) -> Result<types::Type> {
    match ty {
        Ty::Declared { .. } | Ty::Union { .. } | Ty::Option { .. } | Ty::Tuple { .. } => {
            Ok(POINTER)
        }
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
    for ty in target.takes.iter().chain([&target.answers]) {
        if !means_the_same_elsewhere(ty) {
            return Err(not_lowered(format!(
                "{}.{} takes or answers {}, which has no representation an object built from \
                 another document reads the same way, and it is reached across objects",
                target.module,
                target.name,
                ty.spelt()
            )));
        }
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
        Ty::Union { .. } => true,
        Ty::Option { option } => means_the_same_elsewhere(option),
        Ty::Tuple { tuple } => tuple.iter().all(means_the_same_elsewhere),
    }
}

/// What holds the address of a value made of fields.
///
/// One width, and the host's. Nothing here is written for a machine this is not running on, and a
/// pointer narrower or wider than a slot would make a field's offset a question about the host
/// rather than a multiplication.
const POINTER: types::Type = types::I64;

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
                running =
                    call_reached(&mut builder, module, abort, reached, answers, &[running])?;
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
                running =
                    call_reached(&mut builder, module, abort, reached, answers, &[running])?;
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
    let slot = builder.create_sized_stack_slot(ir::StackSlotData::new(
        ir::StackSlotKind::ExplicitSlot,
        SLOT as u32,
        0,
    ));
    let out = builder.ins().stack_addr(POINTER, slot, 0);
    let mut given = arguments.to_vec();
    given.push(out);
    let called = builder.ins().call(reaching, &given);
    let status = builder.inst_results(called)[0];

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
    Ok(builder.ins().load(answers, TRUSTED, out, 0))
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
        Node::Neg { operand, aborts, .. } => {
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
        Node::Str { value, .. } => text_in_the_object(builder, module, value)?,
        Node::Binary {
            op, left, right, aborts, ..
        } => binary(
            builder, lowering, module, bindings, abort, *op,
            Operands { left, right, aborts },
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
                lower(builder, lowering, module, bindings, abort, if taken { then } else { els })
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
        Node::Field { target, field, ty, .. } => {
            let of = target.ty();
            let Ty::Declared { declared } = of else {
                bail!("a field of {}, which holds no fields", of.spelt());
            };
            let shape = lowering.declared.shape(declared)?;
            let at = shape
                .position_of(field)
                .ok_or_else(|| anyhow!("{declared} declares no field {field}"))?;
            let value = lower(builder, lowering, module, bindings, abort, target)?;
            let flags = TRUSTED;
            let held = builder
                .ins()
                .load(types::I64, flags, value, field_at(at) as i32);
            out_of_slot(builder, held, machine_type(ty)?)
        }
        Node::Match { subject, arms, ty, .. } => {
            let value = lower(builder, lowering, module, bindings, abort, subject)?;
            fork_on_what_it_is(
                builder, lowering, module, bindings, abort, value,
                ForkArms { arms, answers: machine_type(ty)? },
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
                builder.ins().store(flags, member, value, member_at(at) as i32);
            }
            value
        }
        Node::Call {
            reaches,
            arguments,
            ty,
            aborts,
        } => match reaches {
            // The copy this module holds, and not another module's copy of the same declaration:
            // a module carries every definition it reaches.
            Reaches::Helper { declared } | Reaches::Behavior { declared } => {
                let reached = match reaches {
                    Reaches::Helper { .. } => {
                        lowering.reachable.of_held(lowering.carrier, declared)?
                    }
                    Reaches::Behavior { .. } => {
                        lowering.reachable.of_behavior_named(declared)?
                    }
                    Reaches::Value { .. } | Reaches::Kernel { .. } => unreachable!(),
                };
                let mut given = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    given.push(lower(builder, lowering, module, bindings, abort, argument)?);
                }
                call_reached(builder, module, abort, reached, machine_type(ty)?, &given)?
            }
            Reaches::Value { .. } => {
                return Err(not_lowered(
                    "a call to a value, which runs in the module that declares it",
                ));
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
            let read_as = arm
                .binds
                .as_ref()
                .ok_or_else(|| anyhow!("an arm binds a value and does not say what it reads it as"))?;
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
    cases: &[String],
) -> Result<ir::Value> {
    let flags = TRUSTED;
    let which = builder.ins().load(POINTER, flags, value, WHICH as i32);
    let mut any: Option<ir::Value> = None;
    for case in cases {
        let expected = tag_of(builder, lowering, module, case)?;
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
    let Operands { left, right, aborts } = operands;
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
            let a = Held::of(left, lower(builder, lowering, module, bindings, abort, left)?);
            let b = Held::of(right, lower(builder, lowering, module, bindings, abort, right)?);
            match op {
                Op::Add | Op::Sub | Op::Mul => arithmetic(builder, abort, op, a, b, aborts),
                Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                    compare(builder, lowering, module, op, a, b)
                }
                // `/` answers the exact quotient, which is not a whole number and has no
                // representation here yet.
                Op::Div => Err(not_lowered(format!("the operator {}", op.spelt()))),
                Op::Concat => join(builder, lowering, module, a, b),
                Op::And | Op::Or => unreachable!("answered above, where the right side may not run"),
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
                Ok(builder
                    .ins()
                    .icmp_imm_s(as_a_whole_number(op), answered, 0))
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
        (Ty::Declared { .. } | Ty::Union { .. }, _) | (_, Ty::Declared { .. } | Ty::Union { .. }) => {
            Err(not_lowered(format!(
                "a comparison of {} against {}, which is what they are made of compared rather \
                 than where they are",
                left.ty.spelt(),
                right.ty.spelt()
            )))
        }
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
                    abort_where_the_sign_bit_is_set(builder, abort, overflow_status(aborts)?, past, also);
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

/// A string the object carries, and the address of it.
///
/// A literal says the same text every run, so it is written into the object rather than worked out
/// into the arena. What comes back is the address of a string like any other: a comparison and a
/// join read it the way they read one a run made, and nothing in the value says which of the two
/// it is. That is what keeps where a string is kept out of what a string means.
///
/// One data object per literal, anonymous because nothing outside this object reaches one and two
/// spellings of one text are not a thing anything has to agree about.
fn text_in_the_object(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    value: &str,
) -> Result<ir::Value> {
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

    let named = module.declare_data_in_func(id, builder.func);
    Ok(builder.ins().symbol_value(POINTER, named))
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
