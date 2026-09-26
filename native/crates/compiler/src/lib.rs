//! Lowers what crossed to Cranelift IR, and lets Cranelift write the object.
//!
//! The object is for the machine this runs on. Choosing a target for another machine is a question
//! about linkers and a runtime built for it, and answering that before the code generation works
//! would be answering the easier question first.
//!
//! A module whose private items state the reasons of a design the code alone does not show denies
//! a missing doc comment itself, with `#![deny(clippy::missing_docs_in_private_items)]`. Where that
//! is enforced follows what a module is responsible for, not which of its items are private: a doc
//! comment that code inserted above it carries onto another item leaves the first undocumented,
//! and the build says so.

mod boundary;
mod closures;
mod codec;
mod coherent;
mod equality;
mod growing;
mod host;
mod index;
mod interface;
mod kernels;
mod link;
mod literals;
mod manifest;
mod replaced;
mod restating;
mod specialize;
pub mod transport;
mod unrun;
mod versioned;

use anyhow::{Result, anyhow, bail};
use closures::{ClosureSites, Site};
use coherent::{Coherent, Defined};
use cranelift::codegen::ir::condcodes::IntCC;
use cranelift::codegen::ir::{
    AbiParam, Function, InstBuilder, MemFlagsData, TrapCode, UserFuncName, types,
};
use cranelift::codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift::codegen::settings::{self, Configurable};
use cranelift::codegen::{Context, ir};
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext, Switch, Variable};
use cranelift::module::{DataDescription, DataId, FuncId, Linkage, Module, default_libcall_names};
use cranelift::object::{ObjectBuilder, ObjectModule};
use interface::Surface;
use kernels::LoweredKernel;
use literals::Literals;
use restating::restate;
use souther_native_abi::{
    ALLOCATE, ANSWERED, CAPABILITY_ENVIRONMENT, CAPABILITY_INVOKE, CARRIED, EXAMPLE_STATUSES,
    FAKE_NO_OUTPUT, HELD, HOST_STATUSES, INJECTION_UNBOUND, LIST_LENGTH, NO_FAILED_CLAUSE, NOTHING,
    Parameter, SLOT, STRING_CODE_POINTS, STRING_COMPARE, STRING_CONCAT, Status, TOKEN, WHICH, Word,
    behavior_symbol, boundary_symbol, built_in_case_symbol, checked_constructor_symbol,
    constructor_symbol, example_symbol, field_at, generated_call, held_symbol, home_symbol,
    list_at, member_at, requirement_at, room_for_capability, room_for_carried, room_for_fields,
    room_for_held, room_for_list, room_for_members, room_for_requirements, spells_a_module,
    spells_a_name, type_symbol, value_symbol,
};
use specialize::{Instance, InstanceId, Specializations};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use transport::{
    AbortKind, AlternativesForm, Arm, Carrier, Case, Declaration, DeclaredBy, Definition,
    Departures, Emitted, Ensures, Guard, LanguageCase, Node, Op, Owner, Prim, Program, Publication,
    Reaches, Reading, Requirement, Routing, Selects, Target, Ty,
};

/// A fork that ran out of arms, which is this compiler having emitted the wrong test rather than
/// anything a program can be written as: the checker settles that a fork always answers. Still a
/// trap and not a status: this is this compiler's own invariant failing, not a Souther computation
/// ending without a value, and the two are told apart by which channel answers for them.
const NO_ARM: u8 = 2;

/// A host handing a list's constructor a count no list holds: below nought, or past what room can
/// be counted for. A trap for the reason [`NO_ARM`] is one: no Souther computation came to this,
/// and a status would say one had.
const COUNT_NO_LIST_HOLDS: u8 = 3;

/// A walk that writes a value finding its result other than its work expects: a form where work
/// is about to leave one, or none where work is about to take one. A trap for the reason
/// [`NO_ARM`] is one: this compiler added the work out of order, and nothing a program does
/// comes to this.
const A_WALK_OUT_OF_ORDER: u8 = 4;

/// Every reason a Souther computation ends without a value, mapped to the wire number a generated
/// function's status answers with. `souther_native_abi` reserves `ANSWERED`, the `HOST_STATUSES`
/// and the `EXAMPLE_STATUSES`, so every member here gets one of what is left, which is held below at compile
/// time rather than by a reading of both tables.
///
/// No default arm, for the reason `KernelContracts::abortsOf` on the Java side has none: a member
/// `AbortKind` adds and this does not answer for is a mapping nobody wrote rather than one that
/// silently agrees with the last one written for something else. What number a member gets is a
/// decision of this crate's alone — the `abi` crate states the wire's width and the values it
/// reserves, and nothing about what any other value of it means.
///
/// `pub`, and not only for `lower`'s own sake: `Running`'s Java test harness reads a status this
/// answers back off a compiled run and has to turn it back into the `AbortKind` it came from to
/// assert anything about it, which means it holds a second, hand-written copy of this same table.
/// `tests/abort_status.rs` is what keeps the two from drifting apart unnoticed — the same role
/// `vocabularies.rs` plays for `Op`, `Prim` and the rest, and the same reason: a member spelt
/// (here, numbered) differently on the two sides reads without complaint and means something
/// other than what either side thinks it does.
pub const fn native_status(kind: AbortKind) -> Status {
    match kind {
        AbortKind::InvariantNotHeld => 1,
        AbortKind::EnsuresNotHeld => 2,
        AbortKind::UnreachableReached => 3,
        AbortKind::DivisionByZero => 4,
        AbortKind::RequiredFormHasNoPlace => 5,
        AbortKind::InvalidBounds => 6,
    }
}

/// That no reason a computation ends is answered with a number the `abi` crate reserves, or with
/// the number of another reason. Both halves are constants, so a number given twice is a build
/// that stops, and not a status a host reads as one thing when it meant the other.
const _: () = {
    let mut at = 0;
    while at < AbortKind::ALL.len() {
        let number = native_status(AbortKind::ALL[at]);
        assert!(number != ANSWERED);
        let mut reserved = 0;
        while reserved < HOST_STATUSES.len() {
            assert!(number != HOST_STATUSES[reserved].1);
            reserved += 1;
        }
        let mut reserved = 0;
        while reserved < EXAMPLE_STATUSES.len() {
            assert!(number != EXAMPLE_STATUSES[reserved].1);
            reserved += 1;
        }
        let mut other = 0;
        while other < at {
            assert!(number != native_status(AbortKind::ALL[other]));
            other += 1;
        }
        at += 1;
    }
};

/// The one status an arithmetic site that may leave the range its type holds jumps to the abort
/// block with.
///
/// Read off the site's own `aborts` — `program.abortsAt(site)`'s answer, carried on the `Node` —
/// rather than assumed from which operator or which kernel this is: what a machine condition here
/// means is a fact `CheckedProgram` already settled, and asking the transport for it instead of
/// deciding it again here is the one thing issue #9 exists to change.
///
/// An arithmetic site names exactly one reason, which [`Coherent`] held every such site to: zero
/// or more than one would be the two halves disagreeing about what kind of site this is, and
/// answering a wrong value because the checker said `NONE` would be worse than refusing the
/// program.
fn overflow_status(aborts: &[AbortKind]) -> Status {
    match aborts {
        [only] => native_status(*only),
        _ => unreachable!("`Coherent` held every site that can leave its range to one reason"),
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
    /// The object is on stdout, or what was asked for is in the directory it was asked into.
    pub const WITH_AN_OBJECT: u8 = 0;

    /// Something went wrong here: a document this driver could not read, or a machine it could not
    /// write for. Not the program's author's to fix.
    pub const BADLY: u8 = 1;

    /// The program is one the language admits and this backend does not write yet. What it was is
    /// on stderr.
    pub const NOT_LOWERED: u8 = 2;
}

fn not_lowered(what: impl Into<String>) -> NotLowered {
    NotLowered(what.into())
}

/// The object holding every behavior the document carries.
///
/// Three steps, and what each may answer instead of an object is part of what it is. Reading the
/// document (`Coherent::of`) refuses whatever the two halves disagree about, and nothing after it
/// can: `emit` answers only [`NotLowered`], so a disagreement found while lowering would not
/// compile, and a name `Coherent` held to be there and is not is this compiler's own mistake.
/// Between them the host is asked for a code generator, which is neither.
pub fn object_for(document: &str) -> Result<Vec<u8>> {
    let program = Program::read(document)?;
    let coherent = Coherent::of(&program)?;
    let module = for_this_host()?;
    Ok(emit(&program, coherent, module)?)
}

/// What a build for a host writes into a directory of its own.
pub struct Library {
    /// The object, which is what another Souther build's object is linked with.
    pub object: PathBuf,
    /// The header a C or C++ compiler includes: the declarations below, with what a compiler wants
    /// around them and a reader of declarations cannot read.
    pub header: PathBuf,
    /// Every function a host calls, declared in C and in nothing a preprocessor has to run over,
    /// for an FFI that reads C declarations.
    pub declarations: PathBuf,
    /// The same functions described in the model's terms, for a binding to be written from.
    pub manifest: PathBuf,
    /// The object and the runtime linked into one shared library, exporting what the header
    /// declares and nothing else.
    pub library: PathBuf,
}

/// What the declarations are called beside the header, which includes them by this name.
const DECLARATIONS: &str = "souther.ffi.h";

/// The header a C or C++ compiler includes. The same text for every library: what is declared is
/// in [`DECLARATIONS`], written from the surface once, and this is only what a compiler needs
/// around it — a guard, the header `int64_t` and the rest come from, and C linkage for C++ — none
/// of which a reader with no preprocessor could read.
fn header() -> String {
    format!(
        "/* What a host calls in a Souther program built by souther-native-compiler: the\n \
         * declarations in {DECLARATIONS}, for a C or C++ compiler. */\n\
         #ifndef SOUTHER_H\n\
         #define SOUTHER_H\n\
         \n\
         #include <stdint.h>\n\
         \n\
         #ifdef __cplusplus\n\
         extern \"C\" {{\n\
         #endif\n\
         \n\
         #include \"{DECLARATIONS}\"\n\
         \n\
         #ifdef __cplusplus\n\
         }}\n\
         #endif\n\
         \n\
         #endif\n"
    )
}

/// What a library is linked from besides the program's own object: what other Souther builds
/// wrote, and the runtime.
///
/// Nothing else. What the program names and does not define is defined by the object of the build
/// that declares it, a behavior with no body included: that object makes the capability a host's
/// implementation of one is handed over as, so nothing is left for whoever links the library to
/// supply.
pub struct Linking {
    /// Every object another Souther build wrote that the program reaches: whose behaviors it calls,
    /// whether they have a body or a host answers them, or whose types it builds and reads. Each
    /// carries its own surface, and one that does not is refused.
    pub builds: Vec<PathBuf>,
    /// The runtime's static archive.
    pub runtime: PathBuf,
}

/// The object for a document, and beside it what a host needs to call it: a header and the
/// declarations it includes, a manifest, and a shared library of the object and what `linking`
/// names.
///
/// A library is one program, so it holds every build the program reaches. What it offers a host is
/// everything each Souther object in it carries. Everything a host reads is written from what those objects carry, which is what
/// their emission put there, so none of it can name a function the rest does not.
pub fn library_for(document: &str, linking: &Linking, into: &Path) -> Result<Library> {
    let linker = link::Linker::of_this_host()?;
    let object = object_for(document)?;
    let library = linker.library();
    let replacing = replaced::Replacing::beside(
        into,
        &[
            "souther.o",
            "souther.h",
            DECLARATIONS,
            "souther.json",
            library,
        ],
    )?;
    let staged = replacing.staging();
    let written = Library {
        object: staged.join("souther.o"),
        header: staged.join("souther.h"),
        declarations: staged.join(DECLARATIONS),
        manifest: staged.join("souther.json"),
        library: staged.join(library),
    };
    match build(&written, &object, linking, &linker) {
        Ok(()) => {
            let placed = replacing.commit(&[
                &written.object,
                &written.header,
                &written.declarations,
                &written.manifest,
                &written.library,
            ])?;
            let [object, header, declarations, manifest, library] =
                <[PathBuf; 5]>::try_from(placed).expect("five were handed over");
            Ok(Library {
                object,
                header,
                declarations,
                manifest,
                library,
            })
        }
        Err(problem) => {
            replacing.abandon();
            Err(problem)
        }
    }
}

/// Writes a build into where `written` names, all of it or an error.
fn build(written: &Library, object: &[u8], linking: &Linking, linker: &link::Linker) -> Result<()> {
    fs::write(&written.object, object)?;

    // Every module once: a module is declared by one build, so one in two objects is two builds
    // of it, or one object handed over twice, and either would be a link of two definitions.
    let mut modules: BTreeMap<String, manifest::Module> = BTreeMap::new();
    let mut objects = vec![written.object.as_path()];
    let mut carried = vec![(written.object.display().to_string(), object.to_vec())];
    for path in &linking.builds {
        carried.push((path.display().to_string(), fs::read(path)?));
        objects.push(path);
    }
    for (named, bytes) in &carried {
        for module in interface::carried_by(bytes, named)? {
            let name = module.name.clone();
            index::once(&mut modules, name.clone(), module, || {
                format!(
                    "the module {name} is carried by two of the objects linked, the second {named}"
                )
            })?;
        }
    }
    let manifest = interface::manifest_of(modules.into_values().collect())?;

    fs::write(&written.header, header())?;
    fs::write(&written.declarations, interface::declarations(&manifest))?;
    fs::write(&written.manifest, interface::written(&manifest))?;
    linker.shared_library(
        &objects,
        &linking.runtime,
        &interface::exported(&manifest),
        &written.library,
    )
}

/// What is left once a document has been read whole: a program this backend does not write yet,
/// and nothing else.
type Lowered<T> = std::result::Result<T, NotLowered>;

/// What Cranelift answered for what this compiler asked of it. Every document that reaches it was
/// read whole first, so a refusal is this compiler having emitted what it should not have.
fn accepted<T, E: fmt::Display>(answer: std::result::Result<T, E>) -> T {
    answer
        .unwrap_or_else(|refused| panic!("Cranelift refused what this compiler emitted: {refused}"))
}

/// An object to write for the machine this runs on.
fn for_this_host() -> Result<ObjectModule> {
    let mut flags = settings::builder();
    // A call out of this object reaches its callee the way the platform's linker expects, which on
    // both of the hosts this runs on means position-independent. Said here rather than left to the
    // host, so that what is emitted is the same wherever it is built.
    flags.set("is_pic", "true")?;
    let isa = cranelift::native::builder()
        .map_err(|it| anyhow!("no code generator for this host: {it}"))?
        .finish(settings::Flags::new(flags))?;

    let builder = ObjectBuilder::new(isa, "souther", default_libcall_names())?;
    Ok(ObjectModule::new(builder))
}

/// The object for a document [`Coherent`] read whole.
fn emit(program: &Program, coherent: Coherent, mut module: ObjectModule) -> Lowered<Vec<u8>> {
    let Coherent {
        declared,
        targets,
        locals,
        runs,
        closures,
        defined,
    } = coherent;
    // Every copy of a helper this object defines, and which of them each call reaches, settled
    // before anything is declared: a helper that leaves type variables open is a function only once
    // a call has said what each variable is.
    let specializations = Specializations::of(&runs)?;

    let mut context = Context::new();
    let mut shapes = FunctionBuilderContext::new();
    let frontend = module.isa().frontend_config();
    let call_conv = module.isa().default_call_conv();

    let allocate = import_runtime(&mut module, ALLOCATE, call_conv);

    // What two strings are compared and joined through. Neither is emitted here: a comparison of
    // text is a walk over two runs of bytes, and one written into every site that says `==` would
    // be the same walk written as many times as the program says it.
    let compare_text = import_runtime(&mut module, STRING_COMPARE, call_conv);
    let join_text = import_runtime(&mut module, STRING_CONCAT, call_conv);
    // And what a string's length is counted through, for the same reason: it is a walk over the
    // bytes, counting what starts a code point.
    let count_text = import_runtime(&mut module, STRING_CODE_POINTS, call_conv);

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
        accepted(module.define_data(id, &token));
    }

    // The lifted function for every closure site the document holds — declared before any body is
    // defined, the same two-phase shape every other declaration here keeps. A site nested inside
    // one body may be referenced from another (a closure returned from one function and applied by
    // another), so nothing about defining a body may assume every site it itself needs was already
    // declared by the time it runs; all of them are, because this runs before any of them does.
    let mut lifted: BTreeMap<usize, FuncId> = BTreeMap::new();
    for (&site, plan) in closures.iter() {
        let fn_ = plan.signature;
        let signature = lifted_signature(&fn_.takes, &fn_.answers, call_conv)?;
        let symbol = format!("$closure${site}");
        let id = accepted(module.declare_function(&symbol, Linkage::Local, &signature));
        index::unique(&mut lifted, site, id);
    }

    // The constructor of every declaration a value is built of through one, declared before any
    // body is, since a construction may call one (`Construction`). A declaration is built by the
    // object of the build that declared it, the way its token is defined there: this object
    // defines the constructor of each declaration it builds (`Runs`), and reaches the constructor
    // of one a module on the path declares.
    let mut constructors = Constructors::default();
    for key in runs.built() {
        let declaration = declared.laid(key);
        let signature = constructor_signature(declaration, call_conv)?;
        // Reached from another build where the module publishes the type, which is what lets
        // another build name it, and where a value of each field means the same there, as a
        // behavior taking the fields would be. Otherwise it is this object's own.
        let linkage = if runs.publishes(key)
            && declaration
                .fields()
                .iter()
                .all(|field| means_the_same_elsewhere(&field.codec.ty()))
        {
            Linkage::Export
        } else {
            Linkage::Local
        };
        let symbol = constructor_symbol(declaration.module(), declaration.name());
        let id = accepted(module.declare_function(&symbol, linkage, &signature));
        index::unique(&mut constructors.by_key, key.to_string(), id);
        // Reached from another build exactly where the constructor is: an attempted construction
        // there asks this which clause the constructor would have ended the run for.
        let symbol = checked_constructor_symbol(declaration.module(), declaration.name());
        let checked = accepted(module.declare_function(
            &symbol,
            linkage,
            &checked_signature(declaration, call_conv)?,
        ));
        index::unique(&mut constructors.checked, key.to_string(), checked);
    }
    for key in runs.calls() {
        let declaration = declared.laid(key);
        match declaration.by() {
            // Built here, or with no representation here for what it holds, which the
            // construction's own fields are refused over.
            DeclaredBy::AModule => continue,
            DeclaredBy::OnThePath => {}
            DeclaredBy::TheLanguage => unreachable!(
                "`Declared` held that the language declares only sums and units, and neither is \
                 built by a call"
            ),
        }
        for field in declaration.fields() {
            crosses_object(
                &format!("{key}, built by the build that declares it, takes"),
                &field.codec.ty(),
            )?;
        }
        let signature = constructor_signature(declaration, call_conv)?;
        let symbol = constructor_symbol(declaration.module(), declaration.name());
        let id = accepted(module.declare_function(&symbol, Linkage::Import, &signature));
        index::unique(&mut constructors.by_key, key.to_string(), id);
    }
    // What decides a construction of each declaration a body here attempts, reached the way its
    // constructor is: defined above where this object builds the declaration, and named here where
    // a module on the path declares it.
    for key in runs.attempts() {
        let declaration = declared.laid(key);
        match declaration.by() {
            DeclaredBy::AModule => continue,
            DeclaredBy::OnThePath => {}
            DeclaredBy::TheLanguage => unreachable!(
                "`Declared` held that the language declares only sums and units, and neither is \
                 attempted"
            ),
        }
        for field in declaration.fields() {
            crosses_object(
                &format!("{key}, attempted through the build that declares it, takes"),
                &field.codec.ty(),
            )?;
        }
        let signature = checked_signature(declaration, call_conv)?;
        let symbol = checked_constructor_symbol(declaration.module(), declaration.name());
        let id = accepted(module.declare_function(&symbol, Linkage::Import, &signature));
        index::unique(&mut constructors.checked, key.to_string(), id);
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
        let signature = behavior_signature(&target.takes(), &target.answers(), call_conv)?;
        let linkage = match defined[&target.declared()] {
            // Defined here, so what the table carries for it is this object's answer about a name
            // the declaring module has already decided. A body or a composition with no such
            // answer is a local definition of a module this document does not carry, which is the
            // two halves disagreeing rather than something to fall back from. That the definition
            // found, if any, is the kind of definition this target says was already checked above.
            Defined::Here => {
                let declared = target.declared();
                let local = locals.get(declared.as_str()).copied().expect(
                    "`Coherent` held every target answering with a local definition to have one",
                );
                Some(linkage_of(local.publication()))
            }
            // Reached only through a capability something was constructed with, so it has no
            // symbol: what answers it is what the capability holds. What a host makes one of is
            // this object's (`host::define_injections`).
            Defined::ByTheHost => {
                crosses_objects(target)?;
                None
            }
            // Named and not defined: another build's object defines it. One a host implements is
            // reached through a capability, as it is in the build that declares it.
            Defined::Elsewhere if target.is == transport::Answers::Injected => None,
            Defined::Elsewhere => {
                crosses_objects(target)?;
                Some(Linkage::Import)
            }
            Defined::Nowhere => {
                return Err(not_lowered(format!(
                    "the unwritten behavior {}",
                    target.declared()
                )));
            }
        };
        if let Some(linkage) = linkage {
            let id = accepted(module.declare_function(&symbol, linkage, &signature));
            reachable.behavior(&target.declared(), id);
        }

        // What holds the answer to what the behavior declares of it, one for each behavior that
        // declares something here: private to this object, since the rules are this object's
        // modules' and a caller elsewhere is held by its own build. Where it is held at the
        // callee, the body moves under a name of its own and the behavior's symbol is what runs it
        // and then holds its answer, so that a call, a stage, a row, a host and a boundary all go
        // through the one place the answer is held.
        if target.ensures.contract().is_some() {
            let name = target.declared();
            let holding = holding_signature(&target.takes(), &target.answers(), call_conv)?;
            let rules = accepted(module.declare_function(
                &format!("$ensures${name}"),
                Linkage::Local,
                &holding,
            ));
            reachable.rules(&name, rules);
            if let Ensures::Callee { .. } = target.ensures {
                let unheld = accepted(module.declare_function(
                    &format!("$unheld${name}"),
                    Linkage::Local,
                    &signature,
                ));
                reachable.unheld(&name, unheld);
            }
        }
    }
    // Every copy of a helper, each under where the helper stands. Held and not exported: a
    // definition a module holds is that module's copy, and nothing outside the object reaches one.
    // A helper leaving nothing open is one function under its own name, and one over variables is
    // a function for each set of types a call needs, told apart by which of its copies each is.
    for (id, instance) in specializations.iter() {
        let symbol = held_symbol(instance.carrier.module(), &instance.held.reached.rendered());
        let symbol = if instance.types.is_empty() {
            symbol
        } else {
            format!("{symbol}.{}", instance.ordinal)
        };
        let signature = signature_over(&instance.takes(), instance.answers(), call_conv)?;
        let defined = accepted(module.declare_function(&symbol, Linkage::Local, &signature));
        reachable.instance(id, defined);
    }
    // A value's home, under where its body stands: the same statement a call from a body is
    // resolved by, so the key a home is put under and the key a call asks for are read off one
    // thing.
    for body in runs.bodies() {
        match body.owner {
            // Declared above, as its copies.
            Owner::Helper(_) => {}
            Owner::Value(value) => {
                // As private as a helper's method, and named the same way: a value's home is this
                // module's own business (ADR-0074) — nothing outside this object reaches it
                // directly, whether outside this object's other modules or another object
                // altogether. A caller elsewhere goes through the entry declared below instead.
                let takes = handover_types(value);
                let symbol = home_symbol(&value.module, &value.name);
                let signature = signature_over(&takes, value.answers(), call_conv)?;
                let id = accepted(module.declare_function(&symbol, Linkage::Local, &signature));
                reachable.value(body.carrier(), &value.declared(), id);
            }
            Owner::Entry(_)
            | Owner::Definition(_)
            | Owner::Example(_)
            | Owner::StoodIn { .. }
            | Owner::Invariant { .. }
            | Owner::Ensures { .. } => {}
        }
    }
    // What each row stands in with: a function answering for each dependency as the row states it,
    // a capability of it, and the capabilities laid out in the order the behavior requires them,
    // which is what the row's entry calls the behavior with. All of it in the object, so a row is run
    // by its entry alone, handed nothing.
    let mut stood = Stood::default();
    for written in &program.modules {
        // Named out in full, and not `..`'d away, so a field `transport::Module` starts carrying
        // tomorrow is a compile error at this one destructure until it is given a home below.
        // `Module::entries` (a module's own published-value entries) is bound to `value_entries`
        // rather than `entries`, which stays free for this loop's own row-entry table below.
        let transport::Module {
            name,
            publishes: _,
            helpers: _,
            values: _,
            entries: value_entries,
            definitions: _,
            examples,
        } = written;
        for entry in value_entries {
            // Exported under value_symbol, which is the one thing about a value ever addressed
            // from outside the module that declares it: the entry takes nothing at the language
            // level (ADR-0074), and its answer is read off its own body rather than the value's —
            // the two agree by the invariant CheckedModule already holds, and re-deriving one from
            // the other here would be the checker's decision read a second time.
            let signature = signature_over(&[], entry.body.ty(), call_conv)?;
            let symbol = value_symbol(&entry.value.module, &entry.value.name);
            let id = accepted(module.declare_function(&symbol, Linkage::Export, &signature));
            reachable.published_value(&entry.value.module, &entry.value.name, id);
        }
        for example in examples {
            let signature = running_a_row(&targets, name, &example.behavior, call_conv)?;
            let symbol = example_symbol(name, &example.behavior, example.at);
            // Reached from outside whatever the module says about the behavior's own name: what
            // this runs is a row, and a row of a kept name is as much a row as any other.
            let id = accepted(module.declare_function(&symbol, Linkage::Export, &signature));
            stood.row(&mut module, &targets, &symbol, example, call_conv)?;
            index::unique(&mut entries, symbol, id);
        }
    }
    // Every published value a call anywhere this object runs reaches, and the answer type its
    // call sites carry (`Runs` gathered them) — read here rather than at the call site during lowering, because a value
    // this program does not itself declare an entry for still needs a symbol declared before any
    // function that calls it is defined, the same two-phase shape every other declaration in this
    // file keeps. A value this program does declare an entry for was just given one above, so
    // only a genuinely foreign one reaches this loop.
    for ((module_name, value_name), ty) in runs.published_values() {
        if reachable.is_published(module_name, value_name) {
            continue;
        }
        // The same boundary a behavior answering from another object crosses (`crosses_objects`),
        // and held to the same condition: an entry is an ordinary call across an object boundary,
        // and a value this backend would refuse a behavior for answering does not become
        // reachable just because a value happened to answer it instead.
        crosses_object(
            &format!("`{module_name}`'s published value {value_name} answers"),
            ty,
        )?;
        let signature = signature_over(&[], ty, call_conv)?;
        let symbol = value_symbol(module_name, value_name);
        let id = accepted(module.declare_function(&symbol, Linkage::Import, &signature));
        reachable.published_value(module_name, value_name, id);
    }

    let comparators = equality::Comparators::default();
    let restaters = restating::Restaters::default();
    let lowerings = Lowerings {
        declared: &declared,
        comparators: &comparators,
        restaters: &restaters,
        reachable: &reachable,
        specializations: &specializations,
        allocate,
        compare_text,
        join_text,
        count_text,
        closures: &closures,
        lifted: &lifted,
        targets: &targets,
        literals: &literals,
        constructors: &constructors,
    };

    // Every body this object runs, each lowered where it stands: what a call from it reaches is
    // what `Coherent` held reachable from there, because both read it off the one `Body`.
    for (id, instance) in specializations.iter() {
        let lowering = lowerings.at(instance.carrier);
        let signature = signature_over(&instance.takes(), instance.answers(), call_conv)?;
        context.clear();
        context.func = Function::with_name_signature(UserFuncName::default(), signature);
        define_helper(
            &mut context.func,
            &mut shapes,
            (id, instance),
            frontend,
            &lowering,
            &mut module,
        )?;
        accepted(module.define_function(reachable.of_instance(id), &mut context));
    }
    for body in runs.bodies() {
        let lowering = lowerings.at(body.carrier());
        match body.owner {
            // Defined above, as its copies.
            Owner::Helper(_) => {}
            Owner::Value(value) => {
                let takes = handover_types(value);
                let signature = signature_over(&takes, value.answers(), call_conv)?;
                let id = reachable.of_value(body.carrier(), &value.declared());
                context.clear();
                context.func = Function::with_name_signature(UserFuncName::default(), signature);
                define(
                    &mut context.func,
                    &mut shapes,
                    Handed::Nothing,
                    &takes,
                    body.node,
                    frontend,
                    &lowering,
                    &mut module,
                )?;
                accepted(module.define_function(id, &mut context));
            }
            Owner::Entry(entry) => {
                let signature = signature_over(&[], entry.body.ty(), call_conv)?;
                let id = reachable.of_published_value(&entry.value.module, &entry.value.name);
                context.clear();
                context.func = Function::with_name_signature(UserFuncName::default(), signature);
                // Taking nothing, the same as a row's entry: what a value needs is handed over
                // inside its own body (`Reaches::Value`, threading each handover), never by a
                // caller of this entry.
                define(
                    &mut context.func,
                    &mut shapes,
                    Handed::Nothing,
                    &[],
                    body.node,
                    frontend,
                    &lowering,
                    &mut module,
                )?;
                accepted(module.define_function(id, &mut context));
            }
            Owner::Definition(behavior_name) => {
                let target = targets.reached(behavior_name);
                let takes = &target.takes();
                let signature = behavior_signature(&target.takes(), &target.answers(), call_conv)?;
                let id = reachable.of_body(behavior_name);
                context.clear();
                context.func =
                    Function::with_name_signature(UserFuncName::default(), signature.clone());
                define(
                    &mut context.func,
                    &mut shapes,
                    Handed::Requirements(body.environment()),
                    takes,
                    body.node,
                    frontend,
                    &lowering,
                    &mut module,
                )?;
                accepted(module.define_function(id, &mut context));
                if let Ensures::Callee { .. } = target.ensures {
                    context.clear();
                    context.func =
                        Function::with_name_signature(UserFuncName::default(), signature);
                    define_held(
                        &mut context.func,
                        &mut shapes,
                        target,
                        id,
                        reachable.of_rules(behavior_name),
                        frontend,
                        &mut module,
                    )?;
                    let held = reachable.of_behavior_named(behavior_name);
                    accepted(module.define_function(held, &mut context));
                }
            }
            Owner::Example(example) => {
                let signature = running_a_row(
                    &targets,
                    body.carrier().module(),
                    &example.behavior,
                    call_conv,
                )?;
                let symbol = example_symbol(body.carrier().module(), &example.behavior, example.at);
                let id = *entries
                    .get(&symbol)
                    .expect("every row was declared an entry before any was defined");
                context.clear();
                context.func = Function::with_name_signature(UserFuncName::default(), signature);
                // Taking nothing: what the row states is written into the body, so an entry with
                // parameters would be a row whose values came from whoever ran it. What it stands in
                // with is in the object too.
                let constructs = format!("{}.{}", body.carrier().module(), example.behavior);
                define(
                    &mut context.func,
                    &mut shapes,
                    Handed::Row {
                        constructs: &constructs,
                        requirements: stood.requirements(&symbol),
                    },
                    &[],
                    body.node,
                    frontend,
                    &lowering,
                    &mut module,
                )?;
                accepted(module.define_function(id, &mut context));
            }
            // Each is one of several bodies lowered into one function, the declaration's and the
            // behavior's, below.
            Owner::Invariant { .. } | Owner::Ensures { .. } => {}
            // Lowered into the function answering for the dependency, below.
            Owner::StoodIn { .. } => {}
        }
    }

    // What answers for each dependency a row stands in for, where the row's body stands.
    for body in runs.bodies() {
        let Owner::Example(example) = body.owner else {
            continue;
        };
        let row = example_symbol(body.carrier().module(), &example.behavior, example.at);
        for (at, stand_in) in example.stands_in.iter().enumerate() {
            let dependency = targets.reached(&stand_in.declared());
            context.clear();
            context.func = Function::with_name_signature(
                UserFuncName::default(),
                behavior_signature(&dependency.takes(), &dependency.answers(), call_conv)?,
            );
            define_stand_in(
                &mut context.func,
                &mut shapes,
                stand_in,
                dependency,
                frontend,
                &lowerings.at(body.carrier()),
                &mut module,
            )?;
            accepted(module.define_function(stood.answering(&row, at), &mut context));
        }
    }

    // A composition has no body: it applies behaviors by name, and nothing it does is resolved
    // where it stands.
    for written in &program.modules {
        for local in &written.definitions {
            let Definition::Composed {
                declared: behavior_name,
                ..
            } = local
            else {
                continue;
            };
            let target = targets.reached(behavior_name);
            let signature = behavior_signature(&target.takes(), &target.answers(), call_conv)?;
            let id = reachable.of_behavior_named(behavior_name);
            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), signature);
            define_composed(
                &mut context.func,
                &mut shapes,
                target,
                local,
                frontend,
                &lowerings,
                &mut module,
            )?;
            accepted(module.define_function(id, &mut context));
        }
    }

    // What holds each answer to what its behavior declares, one function of the behavior's rules.
    // Each rule is lowered where it stands, which is the module that declares the behavior
    // whichever place the check is run from. A caller holding an answer as it crosses in calls
    // this and restates none of it.
    for target in &program.behaviors {
        if target.ensures.contract().is_none() {
            continue;
        }
        let rules: Vec<transport::Body> = runs
            .bodies()
            .filter(|body| matches!(body.owner, Owner::Ensures { target: of, .. } if std::ptr::eq(of, target)))
            .collect();
        context.clear();
        context.func = Function::with_name_signature(
            UserFuncName::default(),
            holding_signature(&target.takes(), &target.answers(), call_conv)?,
        );
        define_rules(
            &mut context.func,
            &mut shapes,
            target,
            &rules,
            frontend,
            &lowerings,
            &mut module,
        )?;
        accepted(module.define_function(reachable.of_rules(&target.declared()), &mut context));
    }

    // Every lifted function, defined after every ordinary body: a site's own body may itself hold
    // a nested site, or reach one returned from elsewhere, and every one of them was declared
    // above regardless of which body it is nested under. A site stands where the body holding it
    // does.
    for (&site, plan) in closures.iter() {
        let fn_ = plan.signature;
        let signature = lifted_signature(&fn_.takes, &fn_.answers, call_conv)?;
        let id = *lifted
            .get(&site)
            .expect("every closure site was declared a lifted function before any was defined");
        context.clear();
        context.func = Function::with_name_signature(UserFuncName::default(), signature);
        define_closure(
            &mut context.func,
            &mut shapes,
            plan,
            frontend,
            &lowerings.at(plan.carrier),
            &mut module,
        )?;
        accepted(module.define_function(id, &mut context));
    }

    for key in runs.built() {
        let declaration = declared.laid(key);
        let checked = constructors.checked(key)?;
        context.clear();
        context.func = Function::with_name_signature(
            UserFuncName::default(),
            constructor_signature(declaration, call_conv)?,
        );
        define_constructor(
            &mut context.func,
            &mut shapes,
            declaration.field_count(),
            checked,
            frontend,
            &mut module,
        );
        accepted(module.define_function(constructors.of(key)?, &mut context));

        let clauses: Vec<transport::Body> = runs
            .bodies()
            .filter(|body| matches!(body.owner, Owner::Invariant { declaration: of, .. } if std::ptr::eq(of, declaration)))
            .collect();
        let signature = checked_signature(declaration, call_conv)?;
        context.clear();
        context.func = Function::with_name_signature(UserFuncName::default(), signature);
        define_checked(
            &mut context.func,
            &mut shapes,
            declaration,
            &clauses,
            frontend,
            &lowerings,
            &mut module,
        )?;
        accepted(module.define_function(checked, &mut context));
    }

    // Every comparator and every restating function a body above asked for, and every one those
    // ask for in turn. Written last because a comparison or a restatement anywhere may be the first
    // to reach a pair of types, and each reaches the types its values are made of only as it is
    // written.
    loop {
        if let Some(owed) = comparators.owed() {
            context.clear();
            context.func = Function::with_name_signature(UserFuncName::default(), owed.signature);
            equality::define_comparator(
                &mut context.func,
                &mut shapes,
                &owed.ty,
                frontend,
                &lowerings,
                &mut module,
            )?;
            accepted(module.define_function(owed.id, &mut context));
        } else if let Some(owed) = restaters.owed() {
            context.clear();
            context.func =
                Function::with_name_signature(UserFuncName::default(), owed.signature.clone());
            restating::define_restater(
                &mut context.func,
                &mut shapes,
                &owed,
                frontend,
                &lowerings,
                &mut module,
            )?;
            accepted(module.define_function(owed.id, &mut context));
        } else {
            break;
        }
    }

    // Every behavior this object defines and publishes, which a host calls and whose answer a
    // boundary writes as the language writes it, and every row, which has a boundary too. A
    // behavior another build implements is that build's to give either to, so one object never
    // answers for a second entry under the same name.
    let mut boundaries = Vec::new();
    let mut published = Vec::new();
    for target in &program.behaviors {
        if defined[&target.declared()] != Defined::Here {
            continue;
        }
        let declared = target.declared();
        let local = locals
            .get(declared.as_str())
            .copied()
            .expect("`Coherent` held every target answering with a local definition to have one");
        if local.publication() != Publication::Published {
            continue;
        }
        boundaries.push(boundary::Boundary {
            symbol: boundary_symbol(&behavior_symbol(&target.module, &target.name)),
            runs: reachable.of_behavior_named(&declared),
            constructed: true,
            takes: target.takes(),
            output: &target.output,
        });
        published.push(host::Entry {
            module: &target.module,
            name: &target.name,
            runs: reachable.of_behavior_named(&declared),
            constructed: true,
            inputs: &target.inputs,
            names: target.names(),
            answers: target.answers(),
            cases: match &target.output {
                transport::BoundaryOutput::Cases { cases, .. } => Some(cases),
                _ => None,
            },
        });
    }
    let constructions = constructions(program, &targets, &locals, &defined, &reachable);
    // Every value a module of this object publishes, through the entry another object reaches it
    // by.
    let values: Vec<host::Entry> = program
        .modules
        .iter()
        .flat_map(|written| &written.entries)
        .map(|entry| host::Entry {
            module: &entry.value.module,
            name: &entry.value.name,
            runs: reachable.of_published_value(&entry.value.module, &entry.value.name),
            constructed: false,
            inputs: &[],
            names: Some(&[]),
            answers: entry.body.ty().clone(),
            cases: None,
        })
        .collect();
    for written in &program.modules {
        for example in &written.examples {
            let target = targets.reached(&format!("{}.{}", written.name, example.behavior));
            let entry = example_symbol(&written.name, &example.behavior, example.at);
            let runs = *entries
                .get(&entry)
                .expect("every row was declared an entry before any was defined");
            boundaries.push(boundary::Boundary {
                symbol: boundary_symbol(&entry),
                runs,
                constructed: false,
                takes: Vec::new(),
                output: &target.output,
            });
        }
    }
    let mut emitting = Emitting {
        module: &mut module,
        context: &mut context,
        shapes: &mut shapes,
        frontend,
        call_conv,
        declared: &declared,
        literals: &literals,
        constructors: &constructors,
        allocate,
    };
    let mut codecs = codec::Codecs::new(call_conv);
    // What a host builds, reads, decodes and encodes a value of a published type through, each
    // running on the constructors just defined and the layout they write.
    let mut surface = Surface::default();
    let mut lists = host::Lists::default();
    host::define(
        &mut emitting,
        &mut codecs,
        &mut surface,
        &mut lists,
        program,
        &runs,
    )?;
    // What a host calls a behavior and reads a value through, beside what another object built by
    // this compiler calls: the two are different parties and are told different things.
    host::define_behaviors(&mut emitting, &mut surface, &mut lists, &published)?;
    // What a host builds the capabilities a behavior is called with out of, apart from what it
    // calls by name.
    host::define_constructions(&mut emitting, &mut surface, &constructions)?;
    host::define_values(&mut emitting, &mut surface, &mut lists, &values)?;
    // What a host implements, and makes a capability of an implementation of its own through.
    let injections: Vec<host::Injected> = program
        .behaviors
        .iter()
        .filter(|target| defined[&target.declared()] == Defined::ByTheHost)
        .map(|target| host::Injected {
            module: &target.module,
            name: &target.name,
            inputs: &target.inputs,
            names: target
                .names()
                .expect("a behavior a host implements is declared, which a target is held to"),
            output: &target.output,
        })
        .collect();
    host::define_injections(&mut emitting, &mut surface, &mut lists, &injections)?;
    // What a host builds and reads every list above through.
    host::define_lists(&mut emitting, &mut surface, &lists)?;
    boundary::define(&mut emitting, &mut codecs, &boundaries)?;
    // Every writer and reader the entries above reached.
    codecs.define(&mut emitting)?;

    surface.carry(&mut module);

    Ok(accepted(module.finish().emit()))
}

/// What a host constructs, apart from what it calls: every behavior this object defines that a
/// published one requires, at any depth, whether or not its module publishes it, and each
/// published one that requires something.
///
/// Two questions, and not one answered by publication. Whether a host may name a behavior is what
/// its module publishes; whether a host has to build a capability of one is whether something a
/// host calls reaches it through `depends on`. A behavior kept by its module that a published one
/// depends on is the second and not the first (upstream ADR-0068: a callee arrives bound), so it has
/// no call and no class a host names, and what a host builds a capability of it out of all the
/// same. One another build defines is on that build's own surface, closed the same way.
///
/// In the order a walk from the published behaviors meets them, each once.
fn constructions<'p>(
    program: &'p Program,
    targets: &Targets<'p>,
    locals: &HashMap<&'p str, &'p Definition>,
    defined: &HashMap<String, Defined>,
    reachable: &Reachable,
) -> Vec<host::Construction<'p>> {
    let mut met: Vec<&'p Target> = Vec::new();
    let mut owed: Vec<&'p Target> = program
        .behaviors
        .iter()
        .filter(|target| {
            defined[&target.declared()] == Defined::Here
                && locals[target.declared().as_str()].publication() == Publication::Published
        })
        .collect();
    owed.reverse();
    while let Some(target) = owed.pop() {
        if met.iter().any(|it| std::ptr::eq(*it, target)) || target.requirements.is_empty() {
            continue;
        }
        met.push(target);
        for required in target.requirements.iter().rev() {
            let declared = required.declared();
            if defined.get(&declared) == Some(&Defined::Here) {
                owed.push(targets.reached(&declared));
            }
        }
    }
    met.into_iter()
        .map(|target| {
            let declared = target.declared();
            host::Construction {
                module: &target.module,
                name: &target.name,
                runs: reachable.of_behavior_named(&declared),
                requires: &target.requirements,
                // What something may `depend on`: a behavior with a body that requires something
                // (spec §depends-on). A composition requires what its stages do and nothing depends
                // on one, so a host is told what it requires and makes no capability of it.
                binds: matches!(locals[declared.as_str()], Definition::Body { .. }),
            }
        })
        .collect()
}

/// Where a definition that is not a body is emitted from: a host's entries, a boundary, a writer
/// or a reader. The same handful of things for each, and one way of putting a function under an id.
pub(crate) struct Emitting<'a> {
    pub module: &'a mut ObjectModule,
    pub context: &'a mut Context,
    pub shapes: &'a mut FunctionBuilderContext,
    pub frontend: TargetFrontendConfig,
    pub call_conv: CallConv,
    pub declared: &'a Declared<'a>,
    pub literals: &'a Literals,
    pub constructors: &'a Constructors,
    pub allocate: FuncId,
}

impl Emitting<'_> {
    /// Defines `id` as what `body` emits, handed the function's parameters from its entry block.
    /// Every block is sealed once the body has emitted all of them, so a body jumps to a block it
    /// has not written yet without saying so.
    pub(crate) fn function(
        &mut self,
        id: FuncId,
        signature: ir::Signature,
        body: impl FnOnce(&mut FunctionBuilder, &mut ObjectModule, &[ir::Value]) -> Lowered<()>,
    ) -> Lowered<()> {
        self.context.clear();
        self.context.func = Function::with_name_signature(UserFuncName::default(), signature);
        let mut builder = FunctionBuilder::new(&mut self.context.func, self.shapes);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let given = builder.block_params(entry).to_vec();
        body(&mut builder, self.module, &given)?;
        builder.seal_all_blocks();
        builder.finalize(self.frontend);
        accepted(self.module.define_function(id, self.context));
        Ok(())
    }
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
    fn of(behaviors: &'a [Target]) -> Result<Self> {
        let mut by_name = HashMap::with_capacity(behaviors.len());
        for target in behaviors {
            let declared = target.declared();
            index::once(&mut by_name, declared.clone(), target, || {
                format!("two behaviors are both written {declared}")
            })?;
        }
        Ok(Targets { by_name })
    }

    /// The behavior a name reaches, where [`Coherent`] already held the name to be one a target
    /// names.
    fn reached(&self, declared: &str) -> &'a Target {
        self.by_name
            .get(declared)
            .copied()
            .expect("`Coherent` held every name reached to be one a target names")
    }

    /// The behavior a name in this document reaches, as the table of targets says it.
    fn named(&self, declared: &str) -> Result<&'a Target> {
        self.by_name
            .get(declared)
            .copied()
            .ok_or_else(|| anyhow!("{declared}, which no target names"))
    }
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
) -> Lowered<ir::Signature> {
    let target = targets.reached(&format!("{module}.{behavior}"));
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
/// A definition a module holds is keyed by which copy of it a call reaches, which
/// [`Specializations`] answers: two modules holding one declaration hold a copy each, a call
/// reaches the copy its own module holds, and a helper leaving type variables open is a function
/// for each set of types a call needs.
///
/// A value's own home is kept apart from a helper's copy (`values`, not folded into `instances`),
/// because the two are different identities even where a document never confuses them: a helper
/// is carried, a value is declared, and souther's own `CheckedModule` already refuses to hold one
/// declaration as both. A published entry is kept apart again (`published_values`), keyed by the
/// declaring module and not by a carrier: unlike a helper or a local value home, an entry is one
/// symbol the whole program shares, addressed by every object that calls it, this one included
/// where it happens to be the declaring module's own.
#[derive(Default)]
struct Reachable {
    /// Each copy of a helper, by which copy it is ([`Specializations`]).
    instances: HashMap<InstanceId, FuncId>,
    values: HashMap<(String, String), FuncId>,
    behaviors: HashMap<String, FuncId>,
    published_values: HashMap<(String, String), FuncId>,
    /// What holds a behavior's answer to what the behavior declares of it, by the behavior, where
    /// something here holds it.
    rules: HashMap<String, FuncId>,
    /// What a behavior held at the callee answers before it is held, by the behavior. The
    /// behavior's own symbol is where it is held, so every way in is held and none of them reaches
    /// this.
    unheld: HashMap<String, FuncId>,
}

/// Built from names [`Coherent`] already held to be named once each, so a name written twice here
/// is this compiler's mistake and not the document's.
impl Reachable {
    fn instance(&mut self, instance: InstanceId, id: FuncId) {
        index::unique(&mut self.instances, instance, id);
    }

    fn value(&mut self, carrier: Carrier, declared: &str, id: FuncId) {
        let key = (carrier.module().to_string(), declared.to_string());
        index::unique(&mut self.values, key, id);
    }

    fn behavior(&mut self, declared: &str, id: FuncId) {
        index::unique(&mut self.behaviors, declared.to_string(), id);
    }

    fn published_value(&mut self, module: &str, name: &str, id: FuncId) {
        let key = (module.to_string(), name.to_string());
        index::unique(&mut self.published_values, key, id);
    }

    fn rules(&mut self, declared: &str, id: FuncId) {
        index::unique(&mut self.rules, declared.to_string(), id);
    }

    fn unheld(&mut self, declared: &str, id: FuncId) {
        index::unique(&mut self.unheld, declared.to_string(), id);
    }

    /// Whether an entry for `module`'s value `name` has already been declared — asked before
    /// declaring one as an import, so a value this program's own modules publish is never given a
    /// second, importing declaration of the same symbol.
    fn is_published(&self, module: &str, name: &str) -> bool {
        self.published_values
            .contains_key(&(module.to_string(), name.to_string()))
    }

    fn of_instance(&self, instance: InstanceId) -> FuncId {
        *self
            .instances
            .get(&instance)
            .expect("every copy of a helper was declared before any body was defined")
    }

    fn of_value(&self, carrier: Carrier, declared: &str) -> FuncId {
        *self
            .values
            .get(&(carrier.module().to_string(), declared.to_string()))
            .expect("`Coherent` held every value a call reaches to have a home in its module")
    }

    fn of_behavior_named(&self, declared: &str) -> FuncId {
        *self
            .behaviors
            .get(declared)
            .expect("`Coherent` held every behavior a call reaches to be one a target names")
    }

    fn of_published_value(&self, module: &str, name: &str) -> FuncId {
        *self
            .published_values
            .get(&(module.to_string(), name.to_string()))
            .expect("every published value a call reaches was declared an entry or an import")
    }

    fn of_rules(&self, declared: &str) -> FuncId {
        *self
            .rules
            .get(declared)
            .expect("every behavior whose answer is held here was declared what holds it")
    }

    /// What `declared`'s body is defined under: the function it answers through before it is
    /// held, where it is held at the callee, and otherwise the behavior's own.
    fn of_body(&self, declared: &str) -> FuncId {
        self.unheld
            .get(declared)
            .copied()
            .unwrap_or_else(|| self.of_behavior_named(declared))
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
            index::once(&mut shapes, key.clone(), declaration, || {
                format!("two declarations are both written {key}")
            })?;
        }
        let declared = Declared { shapes };
        for declaration in declarations {
            let key = declaration.key();
            if !spells_a_module(declaration.module()) || !spells_a_name(declaration.name()) {
                bail!("a declaration is written {key}, which no symbol can carry");
            }
            // What the language itself declares is a set of alternatives or a single value
            // (`CheckedProgramAssembler`), and nothing it declares is built from fields.
            if declaration.by() == DeclaredBy::TheLanguage
                && !matches!(
                    declaration,
                    Declaration::Sum { .. } | Declaration::Unit { .. }
                )
            {
                bail!("{key} is declared by the language and has fields, which none of its has");
            }
            // A field's binding is the number a clause reads it under and the constructor holds it
            // under, so two fields under one is one name for two values, whether the declaration
            // states a clause or not.
            let mut bound = HashMap::new();
            for field in declaration.fields() {
                // A host reads a field by a symbol its name is the last segment of.
                if !spells_a_name(&field.name) {
                    bail!(
                        "{key} declares a field written {}, which no symbol can carry",
                        field.name
                    );
                }
                declared.resolves(&format!("{key}'s field {}", field.name), &field.codec.ty())?;
                index::once(&mut bound, field.binding, (), || {
                    format!(
                        "{key} binds two fields under {}, which a clause reads as one value",
                        field.binding
                    )
                })?;
            }
            // A declaration's clauses cross exactly where this build is the one that runs them, and
            // what they are answered under exactly where another build is.
            if let Declaration::Product {
                invariants,
                headers,
                ..
            }
            | Declaration::Newtype {
                invariants,
                headers,
                ..
            } = declaration
            {
                let are = |carried: bool| if carried { "are" } else { "are not" };
                if invariants.is_some() != (declaration.by() == DeclaredBy::AModule) {
                    bail!(
                        "{key} is declared by {:?} and its clauses {} carried: they cross for a \
                         declaration this build builds and for no other",
                        declaration.by(),
                        are(invariants.is_some())
                    );
                }
                if headers.is_some() != (declaration.by() == DeclaredBy::OnThePath) {
                    bail!(
                        "{key} is declared by {:?} and what its clauses are answered under {} \
                         carried apart from them: that crosses for a declaration another build \
                         builds and for no other",
                        declaration.by(),
                        are(headers.is_some())
                    );
                }
            }
            // A failure is answered by the name of the clause that did not hold, so two clauses
            // under one name would be one arm for two rules, which the checker refuses
            // (`checkClauseNames`) and an arm matched by the name would take for either.
            let mut named = HashMap::new();
            for name in declaration.clause_names().into_iter().flatten() {
                index::once(&mut named, name, (), || {
                    format!("{key} states two clauses both named {name}")
                })?;
            }
            if let Declaration::Sum { cases, form, .. } = declaration {
                declared.settled(&key, cases, form)?;
            }
        }
        Ok(declared)
    }

    /// Refuses a type naming a declaration no declaration of the document is, at any depth: every
    /// key a type names is one the checker declared, and the document carries every declaration
    /// anything in it names. And a type variable, which is a type nobody settled wherever it stands
    /// outside a helper's body ([`Declared::resolves_open`]).
    fn resolves(&self, owner: &str, ty: &Ty) -> Result<()> {
        self.resolving(owner, ty, false)
    }

    /// The same inside a helper's body, where a type may be a variable the body leaves open: what
    /// it comes to is what a call of the helper settles, and the variable names nothing here.
    fn resolves_open(&self, owner: &str, ty: &Ty) -> Result<()> {
        self.resolving(owner, ty, true)
    }

    fn resolving(&self, owner: &str, ty: &Ty, open: bool) -> Result<()> {
        let named = |declared: &str| {
            self.shape(declared)
                .map(|_| ())
                .map_err(|missing| anyhow!("{owner}: {missing}"))
        };
        match ty {
            Ty::Prim { .. } => Ok(()),
            Ty::Declared { declared } => named(declared),
            Ty::Union { union } => {
                for case in union {
                    if let Case::Declared { declared } = case {
                        named(declared)?;
                    }
                }
                Ok(())
            }
            Ty::Option { option } => self.resolving(owner, option, open),
            Ty::List { list } => self.resolving(owner, list, open),
            Ty::Set { set } => self.resolving(owner, set, open),
            Ty::Map { map } => {
                self.resolving(owner, &map.key, open)?;
                self.resolving(owner, &map.value, open)
            }
            Ty::Tuple { tuple } => tuple
                .iter()
                .try_for_each(|it| self.resolving(owner, it, open)),
            Ty::Fn { fn_ } => {
                for taken in &fn_.takes {
                    self.resolving(owner, taken, open)?;
                }
                self.resolving(owner, &fn_.answers, open)
            }
            Ty::Nothing { .. } => Ok(()),
            Ty::Var { .. } if open => Ok(()),
            Ty::Var { var } => bail!(
                "{owner}: the type variable {var} stands outside a helper's body, which is the one \
                 place the checker leaves a type open: the two halves disagree"
            ),
        }
    }

    /// Refuses a set of alternatives the checker could not have settled, as the two halves
    /// disagreeing. The relation is the one `Boundary` decides upstream, stated whole and not
    /// only the direction that has bitten:
    ///
    /// - the set travels as an enumeration exactly when it has cases and every one of them is a
    ///   declared unit — so an enumeration over a case with fields, and a discriminated form over
    ///   nothing but units, are both refused;
    /// - a discriminated form's tag is a key no product case lays a field under, since the case's
    ///   fields and the tag stand in one object and one of the two would be lost;
    /// - no case is a sum, since the checker answers the cases a sum descends to.
    ///
    /// Asked of every sum when the document is read, and of every answer union
    /// ([`Coherent::of`](coherent::Coherent::of)), so nothing downstream is handed a form and cases
    /// that disagree.
    fn settled(&self, owner: &str, cases: &[Case], form: &AlternativesForm) -> Result<()> {
        let mut not_a_unit = None;
        for case in cases {
            let unit = match case {
                Case::Declared { declared } => match self.shape(declared)? {
                    // The checker answers the cases a sum descends to, so a case that is a sum
                    // again is a set of cases this side would have to descend itself.
                    Declaration::Sum { .. } => bail!(
                        "{owner} stands {declared} as a case, and {declared} is a sum, where the \
                         checker answers the cases a sum descends to: the two halves disagree"
                    ),
                    declaration => matches!(declaration, Declaration::Unit { .. }),
                },
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

    /// Whether every value of `actual` is a value of `expected`, as the checker lets one stand as
    /// the other for the types this backend lays out: the same type; for declared types, unions and
    /// primitives, every case the one descends to being among the other's; for an optional, a list
    /// or a tuple, the same asked of what it holds; for a function, one taking at least what the
    /// other takes and answering no more than it answers.
    ///
    /// `None` where either side is a `Set` or a `Map`. The checker lets one stand where a wider one
    /// is asked for, and neither is laid out here, so this side has no reason to know the rule yet
    /// and does not answer it: the question is left to be refused as not lowered.
    ///
    /// Nothing about a value's layout is asked here, so a refusal from this is always the two
    /// halves disagreeing.
    ///
    /// Where a value stands as a wider type is the checker's decision, and the document says so
    /// with a `Widen`; this is asked of that decision, and of the relations a document states
    /// outside `Core` (a composition's stages, what an arm binds), and nowhere else. It is a copy of
    /// part of the checker's rule all the same, and each type laid out here later would copy more
    /// of it.
    fn fits(&self, actual: &Ty, expected: &Ty) -> Result<Option<bool>> {
        if actual == expected {
            return Ok(Some(true));
        }
        Ok(match (actual, expected) {
            // No value of it is made, so what it stands as is never handed one that does not fit.
            (Ty::Nothing { .. }, _) => Some(true),
            (Ty::Set { .. } | Ty::Map { .. }, _) | (_, Ty::Set { .. } | Ty::Map { .. }) => None,
            (Ty::Option { option: actual }, Ty::Option { option: expected })
            | (Ty::List { list: actual }, Ty::List { list: expected }) => {
                self.fits(actual, expected)?
            }
            (Ty::Tuple { tuple: actual }, Ty::Tuple { tuple: expected }) => {
                if actual.len() != expected.len() {
                    return Ok(Some(false));
                }
                self.all_fit(actual.iter().zip(expected))?
            }
            // What the position takes it as is handed only what that type takes, so the function
            // standing there has to take at least that; and what it answers stands where the
            // position's answer does.
            (Ty::Fn { fn_: actual }, Ty::Fn { fn_: expected }) => {
                if actual.takes.len() != expected.takes.len() {
                    return Ok(Some(false));
                }
                let takes = expected.takes.iter().zip(&actual.takes);
                let answers = std::iter::once((actual.answers.as_ref(), expected.answers.as_ref()));
                self.all_fit(takes.chain(answers))?
            }
            _ => match (self.cases_of(actual)?, self.cases_of(expected)?) {
                (Some(actual), Some(expected)) => {
                    Some(actual.iter().all(|case| expected.contains(case)))
                }
                _ => Some(false),
            },
        })
    }

    /// Whether every pair fits, `None` where one is not answered and none is refused.
    fn all_fit<'t>(&self, pairs: impl Iterator<Item = (&'t Ty, &'t Ty)>) -> Result<Option<bool>> {
        let mut all = Some(true);
        for (actual, expected) in pairs {
            match self.fits(actual, expected)? {
                Some(false) => return Ok(Some(false)),
                None => all = None,
                Some(true) => {}
            }
        }
        Ok(all)
    }

    /// Whether a value of `ty` is one a test of which case it is can stand over: a union, or a
    /// declared sum. The checker's own answer (`CaseSpace.of`) and no wider — a product, a newtype
    /// or a unit is a value with one case, and a primitive, an optional or a tuple has none — so a
    /// document testing anything else is the two halves disagreeing.
    ///
    /// Every such type says its case ([`says_its_case`]), which is what the test reads.
    fn has_cases(&self, ty: &Ty) -> Result<bool> {
        Ok(match ty {
            Ty::Union { .. } => true,
            Ty::Declared { declared } => matches!(self.shape(declared)?, Declaration::Sum { .. }),
            Ty::Prim { .. }
            | Ty::Option { .. }
            | Ty::List { .. }
            | Ty::Set { .. }
            | Ty::Map { .. }
            | Ty::Tuple { .. }
            | Ty::Fn { .. }
            | Ty::Var { .. }
            | Ty::Nothing { .. } => false,
        })
    }

    /// The cases a value of `ty` can be, where it is a type made of cases: a declared type, a
    /// union, or a primitive, which is a case of a union that names it.
    fn cases_of(&self, ty: &Ty) -> Result<Option<Vec<Case>>> {
        Ok(match ty {
            Ty::Declared { declared } => Some(self.leaves_of(&[Case::Declared {
                declared: declared.clone(),
            }])?),
            Ty::Union { union } => Some(self.leaves_of(union)?),
            Ty::Prim { prim } => Some(vec![Case::Primitive { prim: *prim }]),
            _ => None,
        })
    }

    /// What a declaration a document was read whole with says, where [`Coherent`] held the key to
    /// be one a declaration crossed for.
    fn laid(&self, declared: &str) -> &'a Declaration {
        self.shapes
            .get(declared)
            .copied()
            .expect("`Coherent` held every declaration named to be one that crossed")
    }

    /// What a value of `case` holds of its own, where it stands as that case.
    ///
    /// Which case a value is and what it holds are two questions. The first is its token, whatever
    /// the case; this answers the second, once, for every place that reads or writes a case's
    /// contents. A value of a declared type is its own contents. A primitive is carried and holds
    /// itself at [`CARRIED`]; a case the language gives holds nothing.
    fn body_of<'c>(&self, case: &'c Case) -> CaseBody<'c>
    where
        'a: 'c,
    {
        match case {
            Case::Declared { declared } => CaseBody::Declared {
                key: declared,
                declaration: self.laid(declared),
            },
            Case::Primitive { prim } => CaseBody::Primitive(*prim),
            Case::Language { case } => CaseBody::Empty(*case),
        }
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
    fn tag(&self, module: &mut ObjectModule, declared: &str) -> Lowered<DataId> {
        let declaration = self.laid(declared);
        if let Declaration::Sum { .. } = declaration {
            unreachable!(
                "a tag for {declared}, which is a sum: nothing is ever tagged with one, since an \
                 arm tests the leaves a case resolved to"
            );
        }
        let linkage = match declaration.by() {
            // At home here, and exported whether the module publishes the type or keeps it: a type
            // the module keeps may still be a case of a sum it publishes, and another build
            // writing or forking on a value of the sum compares against this token. Which of its
            // cases another build can reach is the checker's to say, and not asked here.
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
        Ok(accepted(
            module.declare_data(&symbol, linkage, false, false),
        ))
    }
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
/// other than the value did. That the node's type is the type of what it lowered to is not this
/// struct's doing: the value was made from a binder, a callee or a declaration, and
/// [`coherent`] held the node's type to that before anything was lowered.
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

/// What the lowering of every function in this object shares.
///
/// What it does not hold is where a body stands, which is the one thing that differs between two
/// bodies lowered here and is a body's own to say ([`Lowering`]). A function with no body, a
/// composition, is lowered with this alone, and so cannot resolve a call the way a body would.
struct Lowerings<'a> {
    declared: &'a Declared<'a>,
    /// The function comparing two values of each type a comparison here asked about.
    comparators: &'a equality::Comparators,
    /// The function restating a value of each pair of types a site here asked to have one held as
    /// the other, where that rebuilds it.
    restaters: &'a restating::Restaters,
    reachable: &'a Reachable,
    /// Which copy of a helper each call reaching one reaches.
    specializations: &'a Specializations<'a>,
    allocate: FuncId,
    compare_text: FuncId,
    join_text: FuncId,
    count_text: FuncId,
    /// Every closure site the whole document holds, and what each one reaches — read here rather
    /// than re-walked per body, since a `Node::Block` nested under one top-level body may be
    /// referenced (its captures restored) while defining a different site's own lifted function.
    closures: &'a ClosureSites<'a>,
    /// The lifted function declared for each site, by the site's own number — declared before any
    /// body is defined, the same two-phase shape every other declaration in this object keeps.
    lifted: &'a BTreeMap<usize, FuncId>,
    /// What string literals this object already holds.
    literals: &'a Literals,
    /// The constructor of every declaration a body here builds a value of.
    constructors: &'a Constructors,
    /// Every behavior the document names, which is where what a composition's stage answers is
    /// read.
    targets: &'a Targets<'a>,
}

impl<'a> Lowerings<'a> {
    /// What lowering a body that stands where `carrier` says needs.
    fn at(&'a self, carrier: Carrier<'a>) -> Lowering<'a> {
        Lowering {
            shared: self,
            carrier,
        }
    }
}

/// What the lowering of one body needs besides the function itself: what every lowering here
/// shares, and where the body stands.
///
/// Where it stands is a [`Carrier`], which only a [`transport::Body`] answers and only
/// `Program::bodies` makes, so a body is lowered where `Coherent` read it as standing and not
/// where the code defining its function happened to name.
struct Lowering<'a> {
    shared: &'a Lowerings<'a>,
    /// The module whose copy of a definition a call from here reaches.
    carrier: Carrier<'a>,
}

impl<'a> std::ops::Deref for Lowering<'a> {
    type Target = Lowerings<'a>;

    fn deref(&self) -> &Lowerings<'a> {
        self.shared
    }
}

impl Lowerings<'_> {
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
        let size = builder.ins().iconst(types::I64, bytes);
        self.room_of(builder, module, size)
    }

    /// Room for as many bytes as `bytes` comes to when the run gets there: what a value whose size
    /// is not known until then, a list two others are joined into, is made in.
    fn room_of(
        &self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        bytes: ir::Value,
    ) -> ir::Value {
        let taking = module.declare_func_in_func(self.allocate, builder.func);
        let taken = builder.ins().call(taking, &[bytes]);
        builder.inst_results(taken)[0]
    }
}

/// A generated function's signature, in the `status + out` shape every one of them shares.
///
/// The value crosses through one more parameter than a caller reading only `takes` and `answers`
/// would expect — a pointer the answer is written through — and the return says whether it is
/// there to read: `ANSWERED` if so, and if not why it is not, a language abort's wire number or one
/// of the `abi` crate's `HOST_STATUSES` a host's implementation brought about. A plain return of the
/// answer can only ever say the first of those, which is exactly the gap issue #9 closes; see
/// `define`'s own doc for the rest of the shape this signature is half of.
fn signature_over(takes: &[Ty], answers: &Ty, call_conv: CallConv) -> Lowered<ir::Signature> {
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

/// A behavior's signature: the shape [`signature_over`] gives every body, with what the behavior was
/// constructed with first — the address of the capabilities of what it requires, in order, or
/// null where it requires nothing.
///
/// First because that is what the code of a capability is handed first
/// ([`souther_native_abi::CAPABILITY_ENVIRONMENT`]): a behavior's symbol is the code of its
/// capability as it stands, so a call of the symbol and a call through a capability hand over the
/// same words, and a body is one thing whichever way it is reached.
fn behavior_signature(takes: &[Ty], answers: &Ty, call_conv: CallConv) -> Lowered<ir::Signature> {
    let mut signature = signature_over(takes, answers, call_conv)?;
    signature.params.insert(0, AbiParam::new(POINTER));
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
fn lifted_signature(takes: &[Ty], answers: &Ty, call_conv: CallConv) -> Lowered<ir::Signature> {
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

/// A function of the runtime's, named in the object as `souther_native_abi` says generated code
/// calls it. The signature is lowered from what that crate says it takes and answers, which the
/// runtime's own tests hold to the function, so nothing here writes a width down a second time.
fn import_runtime(module: &mut ObjectModule, name: &str, call_conv: CallConv) -> FuncId {
    let call = generated_call(name);
    let mut signature = ir::Signature::new(call_conv);
    for taken in call.takes {
        signature.params.push(AbiParam::new(match taken {
            Parameter::Given(word) => word_on_the_machine(*word),
            Parameter::Room(_) | Parameter::Slice(_) => POINTER,
        }));
    }
    if let Some(word) = call.answers {
        signature
            .returns
            .push(AbiParam::new(word_on_the_machine(word)));
    }
    accepted(module.declare_function(name, Linkage::Import, &signature))
}

/// What a word generated code hands the runtime is on the machine.
fn word_on_the_machine(word: Word) -> types::Type {
    match word {
        Word::Host(word) => interface::machine(word),
        Word::Comparison => types::I64,
        Word::Memory | Word::Form | Word::Node | Word::Path => POINTER,
    }
}

/// What a value of this type is on the machine.
///
/// A number, a truth, or the address of what a value is made of. Every other primitive is a value
/// with a representation to design — how it is held, who owns it, what frees it — and none of that
/// is decided by giving it a width here.
fn machine_type(ty: &Ty) -> Lowered<types::Type> {
    match ty {
        Ty::Declared { .. } | Ty::Option { .. } | Ty::Tuple { .. } => Ok(POINTER),
        // The address of its length and its elements, as `souther-native-abi` lays one out. What
        // the elements are is the static type's and is not asked here: every element is a slot.
        Ty::List { .. } => Ok(POINTER),
        // What holds a union holds one of its members, and says which by the token at the front of
        // it: a value of a declared type as it is, and a primitive or a case the language gives
        // carried with the runtime's token for it (`carry`). A member with no token is one no value
        // of the union could say it is.
        Ty::Union { union } => {
            for case in union {
                if !matches!(case, Case::Declared { .. }) {
                    built_in_case(case)?;
                }
            }
            Ok(POINTER)
        }
        // A collection other than a list is a value with a layout to design, and none is designed
        // yet. Read whole off the wire all the same: whether a type crosses and whether it can be
        // laid out here are two questions, and only this one is this backend's.
        Ty::Set { .. } | Ty::Map { .. } => {
            Err(not_lowered(format!("a value of type {}", ty.spelt())))
        }
        // A flat closure: one pointer, the same as every other compound value. Slot 0 holds the
        // lifted function's code address and every slot after it a capture — see `closures` — but
        // none of that is a second machine type; a function value is a pointer here exactly as a
        // tuple or a declared value is.
        Ty::Fn { .. } => Ok(POINTER),
        Ty::Var { var } => laid_out_nowhere(*var),
        // No value of it is ever made, so there is nothing to hold. Not a width chosen to stand in
        // for one: a list of it is laid out as any list is (above), and a walk whose step would be
        // handed one never runs that step (`growing`), so what asks this is code that would hold a
        // value no run can make, and it is refused rather than given a place to hold it in.
        Ty::Nothing { .. } => Err(not_lowered(format!("a value of type {}", ty.spelt()))),
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

/// The name the runtime's token for a case no declaration names is defined under, where there is
/// one.
///
/// Every primitive and every case the language gives is named, for the reason `machine_type` names
/// them. A primitive has a token where it has a representation to carry, and every case the
/// language gives has one, since it carries nothing.
fn built_in_case(case: &Case) -> Lowered<&'static str> {
    let name = match case {
        Case::Declared { declared } => unreachable!(
            "{declared} is tagged by its declaration's token, and asked for through `tag`"
        ),
        Case::Primitive { prim } => match prim {
            Prim::Int => "Int",
            Prim::Bool => "Bool",
            Prim::String => "String",
            Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => {
                return Err(not_lowered(format!(
                    "a value of the case {}, which has no representation to carry",
                    prim.spelt()
                )));
            }
        },
        Case::Language { case } => match case {
            LanguageCase::DivisionByZero => "DivisionByZero",
            LanguageCase::NotANumber => "NotANumber",
            LanguageCase::NotADate => "NotADate",
            LanguageCase::NotATime => "NotATime",
            LanguageCase::NotWhole => "NotWhole",
            LanguageCase::NotAFiniteDecimal => "NotAFiniteDecimal",
            // A union naming one of an optional's two cases carries its token like any other the
            // language gives. An optional itself never does: it says which by a null pointer.
            LanguageCase::Some => "Some",
            LanguageCase::None => "None",
        },
    };
    Ok(name)
}

/// What a value standing as a case holds of its own ([`Declared::body_of`]).
pub(crate) enum CaseBody<'c> {
    /// A value of a declared type, which is what it holds.
    Declared {
        key: &'c str,
        declaration: &'c Declaration,
    },
    /// A primitive, carried, holding itself at [`CARRIED`].
    Primitive(Prim),
    /// A case the language gives, holding nothing.
    Empty(LanguageCase),
}

impl CaseBody<'_> {
    /// What the case is called where it is named: the name a set of alternatives is told apart by.
    pub(crate) fn name(&self) -> &str {
        match self {
            CaseBody::Declared { declaration, .. } => declaration.name(),
            CaseBody::Primitive(prim) => prim.spelt(),
            CaseBody::Empty(case) => case.spelt(),
        }
    }
}

/// Whether a value of this type says which case it is, by the token at the front of it.
///
/// A declared type and a union do; nothing else does. A primitive standing as one of their cases
/// is carried so that it says so too (`carry`), which is what makes this a fact about the type and
/// not about which case a value happens to be.
pub(crate) fn says_its_case(ty: &Ty) -> bool {
    matches!(ty, Ty::Declared { .. } | Ty::Union { .. })
}

/// A value whose type says which case it is ([`says_its_case`]), so a token stands at [`WHICH`].
///
/// The only way [`WHICH`] is read. A test of which case a value is loads through the value's
/// address, and a value of any other type is a number, a truth or an address laid out some other
/// way: a load through it is a read of memory the value does not own, and nothing at run time
/// would say so. So the type is asked where the value is made into one of these, and a caller
/// holding one has already been answered.
///
/// What makes asking it here enough is [`Coherent`]: a test of which case a value is stands only
/// over a union or a sum, and a composition routes on cases only where what runs is a declared type
/// or a union, as the checker decides both. A document saying otherwise is refused as the two halves
/// disagreeing before anything is lowered, so reaching [`Tagged::of`] with another type is this
/// compiler's own mistake.
#[derive(Clone, Copy)]
pub(crate) struct Tagged(ir::Value);

impl Tagged {
    /// `value`, of type `ty`, as one whose token can be read.
    ///
    /// # Panics
    ///
    /// Where `ty` does not say its case, which [`Coherent`] held no test or routing to reach.
    pub(crate) fn of(value: ir::Value, ty: &Ty) -> Tagged {
        assert!(
            says_its_case(ty),
            "`Coherent` held every test of which case a value is to stand over a type that says \
             it, and {} does not",
            ty.spelt()
        );
        Tagged(value)
    }

    /// The value itself, to be read as the case a test found it to be.
    pub(crate) fn value(self) -> ir::Value {
        self.0
    }

    /// The token the value carries.
    pub(crate) fn which(self, builder: &mut FunctionBuilder) -> ir::Value {
        builder.ins().load(POINTER, TRUSTED, self.0, WHICH as i32)
    }
}

/// A value of a case no declaration names, made to say which case it is: room with the runtime's
/// token for the case at [`WHICH`], and what the case holds, if it holds anything, at [`CARRIED`].
///
/// A primitive holds itself; a case the language gives holds nothing.
fn carry(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    case: &Case,
    holds: Option<ir::Value>,
) -> Lowered<ir::Value> {
    let value = lowering.room(builder, module, room_to_carry(holds.is_some()));
    carry_into(builder, lowering.declared, module, value, case, holds)
}

/// How much room [`carry_into`] is handed, by whether the case holds something.
const fn room_to_carry(holds: bool) -> i64 {
    if holds {
        room_for_carried()
    } else {
        room_for_fields(0)
    }
}

/// [`carry`], into room of [`room_to_carry`] bytes a caller with no [`Lowerings`] took itself.
pub(crate) fn carry_into(
    builder: &mut FunctionBuilder,
    declared: &Declared,
    module: &mut ObjectModule,
    value: ir::Value,
    case: &Case,
    holds: Option<ir::Value>,
) -> Lowered<ir::Value> {
    let token = token_of(builder, declared, module, case)?;
    builder.ins().store(TRUSTED, token, value, WHICH as i32);
    if let Some(held) = holds {
        let held = into_slot(builder, held);
        builder.ins().store(TRUSTED, held, value, CARRIED as i32);
    }
    Ok(value)
}

/// The address a value of this case carries at [`WHICH`]: its declaration's token, or the
/// runtime's for a case no declaration names.
pub(crate) fn token_of(
    builder: &mut FunctionBuilder,
    declared: &Declared,
    module: &mut ObjectModule,
    case: &Case,
) -> Lowered<ir::Value> {
    let token = match case {
        Case::Declared { declared: key } => declared.tag(module, key)?,
        Case::Primitive { .. } | Case::Language { .. } => accepted(module.declare_data(
            &built_in_case_symbol(built_in_case(case)?),
            Linkage::Import,
            false,
            false,
        )),
    };
    let named = module.declare_data_in_func(token, builder.func);
    Ok(builder.ins().symbol_value(POINTER, named))
}

/// Holds the signature of a behavior the object does not define to what a value still means in
/// another object.
///
/// Said here, where the signature is declared, because that is the one place the two scopes meet.
fn crosses_objects(target: &Target) -> Lowered<()> {
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
fn crosses_object(clause: &str, ty: &Ty) -> Lowered<()> {
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
        // those says which case it is by a token the linker resolves — a declaration's, which the
        // object of the build declaring it defines, or the runtime's for a primitive or a case the
        // language gives, which every object in a library links. So a union means what its cases
        // do: a declared case is its declared type, a primitive what that primitive means, and a
        // case the language gives holds nothing but its token.
        Ty::Union { union } => union.iter().all(|case| match case {
            Case::Declared { declared } => means_the_same_elsewhere(&Ty::Declared {
                declared: declared.clone(),
            }),
            Case::Primitive { prim } => means_the_same_elsewhere(&Ty::Prim { prim: *prim }),
            Case::Language { .. } => true,
        }),
        Ty::Option { option } => means_the_same_elsewhere(option),
        // A length and slots, laid out in the crate both halves read, so a list means what its
        // elements mean.
        Ty::List { list } => means_the_same_elsewhere(list),
        // No layout, so nothing another object could read the same way.
        Ty::Set { .. } | Ty::Map { .. } => false,
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
        Ty::Var { var } => laid_out_nowhere(*var),
        // No value of it crosses, so none can mean something else once it has.
        Ty::Nothing { .. } => true,
    }
}

/// A type variable met where a value's layout is asked for, which is nowhere: `Coherent` refuses one
/// outside a helper's body, and a helper that leaves variables open is lowered only as its copies,
/// each with every variable replaced ([`specialize`]).
fn laid_out_nowhere(var: usize) -> ! {
    unreachable!(
        "the type variable {var} reached a lowering, which is handed only copies of a helper with \
         every variable replaced"
    )
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
#[allow(clippy::too_many_arguments)]
fn define(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    handed: Handed,
    takes: &[Ty],
    body: &Node,
    frontend: TargetFrontendConfig,
    lowering: &Lowering,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut bindings = Bindings::default();
    let first = match handed {
        Handed::Nothing => 0,
        Handed::Row {
            constructs,
            requirements,
        } => {
            let requirements = match requirements {
                Some(laid) => {
                    let laid = module.declare_data_in_func(laid, builder.func);
                    builder.ins().symbol_value(POINTER, laid)
                }
                None => builder.ins().iconst(POINTER, 0),
            };
            bindings.row = Some((constructs.to_string(), requirements));
            0
        }
        Handed::Requirements(requires) => {
            if !requires.is_empty() {
                let variable = builder.declare_var(POINTER);
                let given = builder.block_params(entry)[0];
                builder.def_var(variable, given);
                bindings.environment = Some(Environment::of(variable, requires));
            }
            1
        }
    };
    for (at, taken) in takes.iter().enumerate() {
        let variable = builder.declare_var(machine_type(taken)?);
        let given = builder.block_params(entry)[first + at];
        builder.def_var(variable, given);
        bindings.at(at, variable);
    }
    let out = builder.block_params(entry)[first + takes.len()];

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

/// What a function [`define`] lowers a body as is handed besides what the body takes.
#[derive(Clone, Copy)]
enum Handed<'a> {
    /// Nothing: a value's home and entry, and a row's entry.
    Nothing,
    /// What a behavior was constructed with, first, whatever it requires: a behavior's symbol takes
    /// it ([`behavior_signature`]). The capabilities of `requires`, in order.
    Requirements(&'a [Requirement]),
    /// Nothing, as a row's entry: the behavior it `constructs` is called with `requirements`, what
    /// the row stands in with, where it stands in with anything.
    Row {
        constructs: &'a str,
        requirements: Option<DataId>,
    },
}

/// What each row stands in with, by the row's entry: the function answering for each dependency,
/// and the capabilities of them laid out in the order the behavior requires them.
#[derive(Default)]
struct Stood {
    answering: HashMap<(String, usize), FuncId>,
    requirements: HashMap<String, DataId>,
}

impl Stood {
    /// Declares what `example`, whose entry is `row`, stands in with, and lays out its capabilities.
    /// Read-only data: a capability of a row's stand-in is what it is for as long as the object is.
    fn row(
        &mut self,
        module: &mut ObjectModule,
        targets: &Targets,
        row: &str,
        example: &transport::Example,
        call_conv: CallConv,
    ) -> Lowered<()> {
        if example.stands_in.is_empty() {
            return Ok(());
        }
        let mut capabilities = Vec::with_capacity(example.stands_in.len());
        for (at, stand_in) in example.stands_in.iter().enumerate() {
            let dependency = targets.reached(&stand_in.declared());
            let signature =
                behavior_signature(&dependency.takes(), &dependency.answers(), call_conv)?;
            let answering = accepted(module.declare_function(
                &format!("{row}$standsIn${at}"),
                Linkage::Local,
                &signature,
            ));
            index::unique(&mut self.answering, (row.to_string(), at), answering);
            let capability = accepted(module.declare_data(
                &format!("{row}$standsIn${at}$capability"),
                Linkage::Local,
                false,
                false,
            ));
            let mut laid = DataDescription::new();
            laid.define(vec![0; room_for_capability() as usize].into_boxed_slice());
            let code = module.declare_func_in_data(answering, &mut laid);
            laid.write_function_addr(CAPABILITY_INVOKE as u32, code);
            accepted(module.define_data(capability, &laid));
            capabilities.push(capability);
        }
        let requirements = accepted(module.declare_data(
            &format!("{row}$requirements"),
            Linkage::Local,
            false,
            false,
        ));
        let mut laid = DataDescription::new();
        laid.define(vec![0; room_for_requirements(capabilities.len()) as usize].into_boxed_slice());
        for (at, capability) in capabilities.into_iter().enumerate() {
            let capability = module.declare_data_in_data(capability, &mut laid);
            laid.write_data_addr(requirement_at(at) as u32, capability, 0);
        }
        accepted(module.define_data(requirements, &laid));
        index::unique(&mut self.requirements, row.to_string(), requirements);
        Ok(())
    }

    /// What the row whose entry is `row` stands in with, where it stands in with anything.
    fn requirements(&self, row: &str) -> Option<DataId> {
        self.requirements.get(row).copied()
    }

    /// What answers for the dependency the row whose entry is `row` stands in for at `at`.
    fn answering(&self, row: &str, at: usize) -> FuncId {
        *self
            .answering
            .get(&(row.to_string(), at))
            .expect("every stand-in was declared what answers for it before any was defined")
    }
}

/// What answers for a dependency as a row states it (upstream `StandsIn.answering`): the first entry
/// whose arguments are each equal to what the call arrived with, compared as `==` compares two values
/// of the type the dependency takes, answers what it states; where none does, what the row states
/// for the rest, and where it states nothing for the rest, [`FAKE_NO_OUTPUT`].
///
/// The code of a capability, like any behavior's: what it is handed first is null and never read.
/// What it answers is handed on by whatever called through it, [`FAKE_NO_OUTPUT`] too, since a
/// status is held to what may be answered only where a host's implementation answers it.
fn define_stand_in(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    stand_in: &transport::StandIn,
    dependency: &Target,
    frontend: TargetFrontendConfig,
    lowering: &Lowering,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);
    let takes = dependency.takes();
    let given = builder.block_params(entry).to_vec();
    let (asked, out) = (&given[1..=takes.len()], given[takes.len() + 1]);

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);
    let mut bindings = Bindings::default();

    let answered = |builder: &mut FunctionBuilder, answer: ir::Value| {
        builder.ins().store(TRUSTED, answer, out, 0);
        let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
        builder.ins().return_(&[ok]);
    };
    for stated in &stand_in.entries {
        let next = builder.create_block();
        for ((argument, taken), asked) in stated.arguments.iter().zip(&takes).zip(asked) {
            let argument = lower(
                &mut builder,
                lowering,
                module,
                &mut bindings,
                abort,
                argument,
            )?;
            let same = equality::equal(&mut builder, lowering, module, taken, *asked, argument)?;
            let holds = builder.create_block();
            builder.ins().brif(same, holds, &[], next, &[]);
            builder.seal_block(holds);
            builder.switch_to_block(holds);
        }
        let answer = lower(
            &mut builder,
            lowering,
            module,
            &mut bindings,
            abort,
            &stated.answer,
        )?;
        answered(&mut builder, answer);
        builder.seal_block(next);
        builder.switch_to_block(next);
    }
    match &stand_in.otherwise {
        Some(otherwise) => {
            let answer = lower(
                &mut builder,
                lowering,
                module,
                &mut bindings,
                abort,
                otherwise,
            )?;
            answered(&mut builder, answer);
        }
        None => {
            let missed = builder.ins().iconst(types::I32, i64::from(FAKE_NO_OUTPUT));
            builder.ins().return_(&[missed]);
        }
    }

    builder.seal_block(abort);
    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.finalize(frontend);
    Ok(())
}

/// A copy of a helper, in the shape [`define`] gives every body, with each call to itself in tail
/// position a jump back to the start and not a call.
///
/// The entry hands what it was called with to a loop header whose block parameters are the
/// helper's parameters, and the body is lowered from there. A call reaching this same copy
/// ([`Specializations::callee`]) where the body answers what it answers ([`lower_tail`]) works out
/// every argument and then jumps to the header with them, so the recursion runs in the one frame. A
/// call reaching it anywhere else is a call, since what it answers is still to be used.
///
/// The same copy and not the same helper: a helper called at other types from inside itself would
/// be another copy, and a jump to this one's header would run it at the wrong types. What the
/// answer is written through is the entry's, which the header never changes, so it is not one of
/// the header's parameters.
fn define_helper(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    (id, instance): (InstanceId, &Instance),
    frontend: TargetFrontendConfig,
    lowering: &Lowering,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let takes = instance.takes();
    let header = builder.create_block();
    for taken in &takes {
        builder.append_block_param(header, machine_type(taken)?);
    }
    let given: Vec<ir::BlockArg> = builder.block_params(entry)[..takes.len()]
        .iter()
        .map(|&it| it.into())
        .collect();
    let out = builder.block_params(entry)[takes.len()];
    builder.ins().jump(header, &given);

    builder.switch_to_block(header);
    let mut bindings = Bindings::default();
    for (at, taken) in takes.iter().enumerate() {
        let variable = builder.declare_var(machine_type(taken)?);
        let given = builder.block_params(header)[at];
        builder.def_var(variable, given);
        bindings.at(at, variable);
    }

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);

    let tail = Tail {
        header,
        out,
        instance: id,
    };
    lower_tail(
        &mut builder,
        lowering,
        module,
        &mut bindings,
        abort,
        instance.body(),
        &tail,
    )?;
    // Every jump back to the header is written now.
    builder.seal_block(header);

    builder.seal_block(abort);
    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.finalize(frontend);
    Ok(())
}

/// Where a copy of a helper answers from: the header a call to itself jumps back to, what its
/// answer is written through, and which copy it is.
struct Tail {
    header: ir::Block,
    out: ir::Value,
    instance: InstanceId,
}

/// `node`, standing where the copy of a helper being defined answers what it answers, lowered to
/// that answer: written through `out`, or, for a call reaching this same copy, a jump back to the
/// header. Every block this leaves is ended, by a return or by a jump.
///
/// Where the body answers is the body itself and every branch of a fork that answers there, which
/// is [`branched`]'s to say, the same table [`lower`] reads a fork's branches from. Anywhere else a
/// node is lowered for its value ([`lower`]), and a call there stays a call. The arguments of a
/// call that becomes a jump are all worked out before the jump hands them over, so a call handing
/// the parameters round (`f(b, a)`) reads each before any is replaced.
fn lower_tail(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    abort: ir::Block,
    node: &Node,
    tail: &Tail,
) -> Lowered<()> {
    if let Node::Call {
        reaches: Reaches::Helper { reached: _ },
        arguments,
        ..
    } = node
        && lowering.specializations.callee(node) == tail.instance
    {
        let mut given: Vec<ir::BlockArg> = Vec::with_capacity(arguments.len());
        for argument in arguments {
            given.push(lower(builder, lowering, module, bindings, abort, argument)?.into());
        }
        builder.ins().jump(tail.header, &given);
        return Ok(());
    }
    let forked = branched(
        builder,
        lowering,
        module,
        bindings,
        abort,
        node,
        &mut |builder, module, bindings, branch| {
            lower_tail(builder, lowering, module, bindings, abort, branch, tail)
        },
    )?;
    if !forked {
        let answer = lower(builder, lowering, module, bindings, abort, node)?;
        builder.ins().store(TRUSTED, answer, tail.out, 0);
        let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
        builder.ins().return_(&[ok]);
    }
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
) -> Lowered<()> {
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
    if let Some(requires) = site.environment {
        let held = builder.ins().load(
            POINTER,
            TRUSTED,
            closure,
            capture_at(site.captures.len()) as i32,
        );
        let variable = builder.declare_var(POINTER);
        builder.def_var(variable, held);
        bindings.environment = Some(Environment::of(variable, requires));
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

/// The one function a value of a declaration is built by: the one this object defines for a
/// declaration it builds, and the one it reaches for a declaration a module on the path declares.
#[derive(Default)]
struct Constructors {
    by_key: BTreeMap<String, FuncId>,
    /// What decides a construction of each declaration this object builds ([`define_checked`]),
    /// which its constructor, its reader and an attempted construction call, and of each one a
    /// module on the path declares that a body here attempts.
    checked: BTreeMap<String, FuncId>,
}

impl Constructors {
    /// The constructor a construction of `declared` calls. None where the declaration is this
    /// build's and holds something with no representation here, which is not lowered.
    fn of(&self, declared: &str) -> Lowered<FuncId> {
        self.by_key.get(declared).copied().ok_or_else(|| {
            not_lowered(format!(
                "a value of {declared}, whose fields have no representation here"
            ))
        })
    }

    /// What decides a construction of `declared`. None where the declaration is this build's and
    /// holds something with no representation here, which is not lowered.
    fn checked(&self, declared: &str) -> Lowered<FuncId> {
        self.checked.get(declared).copied().ok_or_else(|| {
            not_lowered(format!(
                "a value of {declared}, whose fields have no representation here"
            ))
        })
    }
}

/// What this object runs, which is narrower than what the document says, and what running it
/// needs from outside the bodies themselves.
///
/// Every body of a module, every rule a behavior's answer is held to here, and the clauses of each
/// declaration this object builds a constructor for. A rule runs whether or not anything here calls
/// the behavior: what holds an answer is the declaring module's, defined once where the module is,
/// the way the constructor of a type it publishes is. It builds one for a declaration a module of this compile declares, whose fields all have a
/// representation here, wherever something may build a value of it through this object: another
/// build or a host, where the module publishes it; a body here, through a call
/// ([`Construction::Called`]) or an attempted construction ([`Node::attempts`]); and a reader,
/// where a value of a published type is read through it
/// ([`codec::reached_from`]). A declaration none of those reaches is read, and its clauses held to
/// what the checker held them to, and nothing of it is run here: no value of it is built here to
/// run a clause over, and the program is not refused for what nothing runs.
///
/// Settled once, when this is made, by walking each body it runs once: which bodies run, which
/// constructors a body calls and which published values it reaches. Every pass that asks what this
/// object will emit reads those answers, and walks [`Runs::bodies`] only for what it asks of a
/// body's own shape, never [`Program::bodies`], which is what the document says and is read whole.
/// A fact another pass comes to need about what runs is gathered in [`Reach`] beside these,
/// rather than by a walk of its own.
pub(crate) struct Runs<'p> {
    bodies: Vec<transport::Body<'p>>,
    built: BTreeSet<String>,
    published: BTreeSet<String>,
    /// Every declaration with an external form this backend reads and writes ([`codec::carried`]).
    carried: BTreeSet<String>,
    reach: Reach<'p>,
}

/// What the bodies an object runs reach besides one another, gathered in the one walk
/// [`Runs::of`] makes of each.
#[derive(Default)]
struct Reach<'p> {
    /// Every declaration a body here builds a value of by calling its constructor.
    calls: BTreeSet<&'p str>,
    /// Every declaration a body here attempts to build a value of, through what decides a
    /// construction of it and never its constructor.
    attempts: BTreeSet<&'p str>,
    /// Every published value a call here reaches, by the module and the name the call names, with
    /// the answer type one of its call sites carries.
    ///
    /// A `BTreeMap`, because this is walked to declare symbols in whatever order it hands them
    /// back, and nothing in this file lets an unordered map decide an order that ends up in the
    /// object. One type per value and not one per call: every call to one value answers with the
    /// same type, since it is one declaration, and [`Coherent`] refused a document where two calls
    /// disagree.
    published_values: BTreeMap<(String, String), Ty>,
}

impl<'p> Reach<'p> {
    /// What `body` reaches where it runs, added to what is already here: nothing in the step of a
    /// walk that never runs.
    fn of(&mut self, body: &transport::Body<'p>, declared: &Declared) -> Result<()> {
        let mut named = Ok(());
        unrun::each_lowered(body.node, &mut |node| {
            if let Some(key) = node.builds() {
                match declared.shape(key) {
                    Ok(declaration) => {
                        if construction(declaration) == Construction::Called {
                            self.calls.insert(key);
                        }
                    }
                    Err(missing) => {
                        if named.is_ok() {
                            named = Err(missing);
                        }
                    }
                }
            }
            if let Some(key) = node.attempts() {
                match declared.shape(key) {
                    Ok(_) => {
                        self.attempts.insert(key);
                    }
                    Err(missing) => {
                        if named.is_ok() {
                            named = Err(missing);
                        }
                    }
                }
            }
            if let Node::Call {
                reaches: Reaches::PublishedValue { module, name },
                ty,
                ..
            } = node
            {
                self.published_values
                    .entry((module.clone(), name.clone()))
                    .or_insert_with(|| ty.clone());
            }
        });
        named
    }
}

impl<'p> Runs<'p> {
    pub(crate) fn of(program: &'p Program, declared: &Declared) -> Result<Self> {
        let published: BTreeSet<String> = program
            .modules
            .iter()
            .flat_map(|module| module.publishes.iter().cloned())
            .collect();
        // Every body of a module runs. A clause runs where its declaration is built, and what is
        // built is settled by the modules' bodies alone: a clause builds nothing from fields, as
        // `Coherent` holds, and the unit it may name is laid out where it stands, so no clause
        // calls a constructor and none makes another declaration built.
        let (clauses, bodies): (Vec<_>, Vec<_>) = program
            .bodies()
            .partition(|body| matches!(body.owner, transport::Owner::Invariant { .. }));
        let mut reach = Reach::default();
        for body in &bodies {
            reach.of(body, declared)?;
        }
        // A host reads a value of every published type of this build that has an external form,
        // and reading one builds a value of every type it holds, published or kept: so each of
        // those is built here, by a construction a reader calls.
        let carried = codec::carried(&program.declarations, declared);
        let read = codec::reached_from(
            program
                .declarations
                .iter()
                .map(Declaration::key)
                .filter(|key| published.contains(key) && carried.contains(key))
                .filter(|key| declared.laid(key).by() == DeclaredBy::AModule),
            declared,
        );
        let built: BTreeSet<String> = program
            .declarations
            .iter()
            .filter(|declaration| {
                let key = declaration.key();
                declaration.by() == DeclaredBy::AModule
                    && !matches!(declaration, Declaration::Sum { .. })
                    && (published.contains(&key)
                        || reach.calls.contains(key.as_str())
                        || reach.attempts.contains(key.as_str())
                        || read.contains(&key))
                    && declaration
                        .fields()
                        .iter()
                        .all(|field| machine_type(&field.codec.ty()).is_ok())
            })
            .map(Declaration::key)
            .collect();
        let mut runs = Runs {
            bodies,
            built,
            published,
            carried,
            reach: Reach::default(),
        };
        for clause in clauses {
            if runs.runs(&clause) {
                reach.of(&clause, declared)?;
                runs.bodies.push(clause);
            }
        }
        runs.reach = reach;
        Ok(runs)
    }

    /// Whether another build may build a value of `declared` through this object: the module
    /// publishes it, so another build can name it.
    fn publishes(&self, declared: &str) -> bool {
        self.published.contains(declared)
    }

    /// Whether a value of `declared` has an external form this backend reads and writes.
    fn carries(&self, declared: &str) -> bool {
        self.carried.contains(declared)
    }

    /// Every declaration this object defines a constructor for, by the key a reference to it says.
    fn built(&self) -> impl Iterator<Item = &str> {
        self.built.iter().map(String::as_str)
    }

    /// Every declaration a body here builds a value of by calling its constructor.
    fn calls(&self) -> impl Iterator<Item = &'p str> + '_ {
        self.reach.calls.iter().copied()
    }

    /// Every declaration a body here attempts to build a value of.
    fn attempts(&self) -> impl Iterator<Item = &'p str> + '_ {
        self.reach.attempts.iter().copied()
    }

    /// Every published value a call here reaches, with the type it answers.
    fn published_values(&self) -> &BTreeMap<(String, String), Ty> {
        &self.reach.published_values
    }

    /// Every body this object runs.
    pub(crate) fn bodies(&self) -> impl Iterator<Item = transport::Body<'p>> + '_ {
        self.bodies.iter().copied()
    }

    /// Whether this object runs `body`: every body of a module and every rule over a behavior's
    /// answer does, and a clause does where its declaration is one this object builds.
    pub(crate) fn runs(&self, body: &transport::Body) -> bool {
        match body.owner {
            transport::Owner::Invariant { declaration, .. } => {
                self.built.contains(&declaration.key())
            }
            _ => true,
        }
    }
}

/// How a construction of a declaration is made where it stands: laid out right there, or by a call
/// to the declaration's constructor.
///
/// Laid out where this object holds the declaration's clauses and there are none — a unit, which
/// never has one, or a type of this compile's that states none — since there is then nothing for a
/// constructor to run and nothing a construction can end for. Called where there are clauses to
/// run, and where the clauses are another build's and not carried, whether that build's type
/// states any or not. Either way the value is laid out by [`lay_out`], so how one is laid out is
/// written once whichever way a construction is made.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Construction {
    Laid,
    Called,
}

fn construction(declaration: &Declaration) -> Construction {
    match declaration.clauses() {
        Some([]) => Construction::Laid,
        Some(_) | None => Construction::Called,
    }
}

/// What a declaration's constructor takes and answers: its fields, in the order they are laid out,
/// and a value of it, in the `status + out` shape every generated function shares.
fn constructor_signature(declaration: &Declaration, call_conv: CallConv) -> Lowered<ir::Signature> {
    let takes: Vec<Ty> = declaration
        .fields()
        .iter()
        .map(|field| field.codec.ty())
        .collect();
    signature_over(
        &takes,
        &Ty::Declared {
            declared: declaration.key(),
        },
        call_conv,
    )
}

/// What [`define_checked`] takes and answers: what the constructor does, with room for which
/// clause did not hold after the room for the value.
fn checked_signature(declaration: &Declaration, call_conv: CallConv) -> Lowered<ir::Signature> {
    let mut signature = constructor_signature(declaration, call_conv)?;
    let answered = signature.params.len();
    signature.params.insert(answered, AbiParam::new(POINTER));
    Ok(signature)
}

/// What a construction of a declaration is decided by: every clause run over the fields it was
/// handed, in the order the declaration states them, and the value laid out only once all of them
/// hold.
///
/// One function for every way a value of the declaration is made, so there is one place what a
/// value of the type is gets decided. The constructor a body or another build calls is this with
/// its answer read as a status ([`define_constructor`]); a reader calls it directly, since a reader
/// reports which clause did not hold and a status says only that one did not; and an attempted
/// construction calls it directly too, since it takes the arm that clause names. None of them runs
/// a clause of its own, and each asks this through [`decide`].
///
/// It takes the fields, room for the value and room for which clause did not hold, and answers a
/// status, as [`checked_constructor_symbol`] states. `ANSWERED` with the clause's room holding
/// [`NO_FAILED_CLAUSE`] is a value, written through its room. `ANSWERED` with the clause's room
/// holding a clause's place among the declaration's, counted from nought, is that clause not
/// holding, and nothing after it runs and nothing is laid out. A clause that itself ends without a
/// value, dividing by nought or leaving an `Int`'s range, ends the construction with that status
/// instead: the clause did not answer false, it did not answer.
///
/// A field is put under the binding its clauses read it through and not under where it sits, which
/// is what lets a clause a spread took in read the field the declaration that wrote it named.
///
/// Nothing is laid out before every clause has held, so a value that is not one of the type never
/// exists, even in the arena.
///
/// Each clause is one of `clauses`, the bodies the program says the declaration's clauses are, and
/// is lowered where that body stands.
fn define_checked(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    declaration: &Declaration,
    clauses: &[transport::Body],
    frontend: TargetFrontendConfig,
    lowerings: &Lowerings,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let fields = declaration.fields();
    let mut bindings = Bindings::default();
    let mut given = Vec::with_capacity(fields.len());
    for (at, field) in fields.iter().enumerate() {
        let value = builder.block_params(entry)[at];
        let variable = builder.declare_var(machine_type(&field.codec.ty())?);
        builder.def_var(variable, value);
        bindings.at(field.binding, variable);
        given.push(value);
    }
    let out = builder.block_params(entry)[fields.len()];
    let which_clause = builder.block_params(entry)[fields.len() + 1];

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);
    let broken = builder.create_block();
    builder.append_block_param(broken, types::I64);

    let stated = declaration
        .clauses()
        .expect("a constructor is defined only for a declaration this build runs the clauses of");
    assert_eq!(
        clauses.len(),
        stated.len(),
        "every clause of a declaration this object builds is a body it runs"
    );
    for (at, clause) in clauses.iter().enumerate() {
        assert!(
            matches!(clause.owner, Owner::Invariant { at: place, .. } if place == at),
            "the program lists a declaration's clauses in the order they run"
        );
        let holds = lower(
            &mut builder,
            &lowerings.at(clause.carrier()),
            module,
            &mut bindings,
            abort,
            clause.node,
        )?;
        let held = builder.create_block();
        let place = builder.ins().iconst(
            types::I64,
            i64::try_from(at).expect("fewer clauses than an Int counts"),
        );
        builder
            .ins()
            .brif(holds, held, &[], broken, &[place.into()]);
        builder.seal_block(held);
        builder.switch_to_block(held);
    }

    let value = lay_out(
        &mut builder,
        module,
        lowerings.declared,
        lowerings.allocate,
        declaration,
        &given,
    )?;
    builder.ins().store(TRUSTED, value, out, 0);
    let none = builder.ins().iconst(types::I64, NO_FAILED_CLAUSE);
    builder.ins().store(TRUSTED, none, which_clause, 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);

    builder.seal_block(broken);
    builder.switch_to_block(broken);
    let place = builder.block_params(broken)[0];
    builder.ins().store(TRUSTED, place, which_clause, 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);

    builder.seal_block(abort);
    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.finalize(frontend);
    Ok(())
}

/// A declaration's constructor: [`define_checked`]'s decision, with a clause that does not hold
/// answered as `InvariantNotHeld`, which is what it is inside a body — a computation among values
/// that ends without one — and what another build calling this is told.
fn define_constructor(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    fields: usize,
    checked: FuncId,
    frontend: TargetFrontendConfig,
    module: &mut ObjectModule,
) {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    let given = builder.block_params(entry).to_vec();
    let (fields, out) = given.split_at(fields);

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);
    let broken = builder.create_block();
    builder.append_block_param(broken, types::I64);

    let value = decide(&mut builder, module, checked, fields, abort, broken);
    builder.ins().store(TRUSTED, value, out[0], 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);

    builder.switch_to_block(broken);
    let not_held = builder.ins().iconst(
        types::I32,
        i64::from(native_status(AbortKind::InvariantNotHeld)),
    );
    builder.ins().return_(&[not_held]);

    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.seal_all_blocks();
    builder.finalize(frontend);
}

/// A construction from `fields` decided by `checked`, and the value where every clause held.
///
/// The one way [`define_checked`] is asked and its answer read, by the constructor, a reader and an
/// attempted construction alike, so the order its answer is read in is written once: a status other
/// than `ANSWERED` goes to `abort` as it came, since a clause that did not answer wrote nothing; a
/// clause that did not hold goes to `broken`, with its place among the declaration's as that
/// block's one parameter; and only then is the value read, in the block this leaves the builder in.
/// A caller is never handed a room to read before the answer says what is in it.
///
/// `broken` is the caller's to fill and to seal.
fn decide(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    checked: FuncId,
    fields: &[ir::Value],
    abort: ir::Block,
    broken: ir::Block,
) -> ir::Value {
    let value_room = out_slot(builder);
    let clause_room = out_slot(builder);
    let mut given = fields.to_vec();
    given.push(value_room);
    given.push(clause_room);
    let reaching = module.declare_func_in_func(checked, builder.func);
    let called = builder.ins().call(reaching, &given);
    let status = builder.inst_results(called)[0];
    forward_unless_answered(builder, abort, status);

    let clause = builder.ins().load(types::I64, TRUSTED, clause_room, 0);
    let every_clause_held = builder
        .ins()
        .icmp_imm_s(IntCC::Equal, clause, NO_FAILED_CLAUSE);
    let held = builder.create_block();
    builder
        .ins()
        .brif(every_clause_held, held, &[], broken, &[clause.into()]);
    builder.seal_block(held);
    builder.switch_to_block(held);
    builder.ins().load(POINTER, TRUSTED, value_room, 0)
}

/// Which of an attempted construction's departures answers each clause of the declaration it
/// attempts, by the clause's place, as a place among [`Departures::bodies`].
///
/// The checker's rule, as `checkArmsAnswerClauses` states it, and the one statement of it here:
/// [`Coherent`] holds a document to it and the lowering reads which way to go from it, so the two
/// cannot come apart. One value for any failure answers every clause. Otherwise each clause with a
/// name is answered by the arm naming it and by no other, and the clauses with no name by the arm
/// naming none; an arm naming a clause the declaration does not state, two naming one clause, a
/// clause no arm answers, and an arm naming none where every clause has a name are each refused.
/// So the arm naming none never answers a clause with a name.
///
/// By the place and not the name, because the place is what the object running the clauses
/// answers; the names are only what an author wrote an arm against, and nothing at run time reads
/// them.
pub(crate) fn departures_taken(
    clauses: &[Option<&str>],
    departures: &Departures,
) -> Result<Vec<usize>> {
    let (named, unnamed) = match departures {
        Departures::Any(_) => return Ok(vec![0; clauses.len()]),
        Departures::ByClause { named, unnamed } => (named, unnamed),
    };
    let mut arms: HashMap<&str, usize> = HashMap::new();
    for (at, (name, _)) in named.iter().enumerate() {
        if !clauses.contains(&Some(name.as_str())) {
            bail!("a departure answers the clause {name}, which it does not state");
        }
        index::once(&mut arms, name.as_str(), at, || {
            format!("two departures answer the clause {name}")
        })?;
    }
    // Where it stands among the bodies: after every arm naming a clause.
    let unnamed = unnamed.as_ref().map(|_| named.len());
    if unnamed.is_some() && !clauses.contains(&None) {
        bail!("a departure answers the clauses that have no name, and every clause has one");
    }
    clauses
        .iter()
        .enumerate()
        .map(|(place, clause)| match clause {
            Some(name) => arms
                .get(name)
                .copied()
                .ok_or_else(|| anyhow!("its clause {name} is answered by no departure")),
            None => unnamed.ok_or_else(|| {
                anyhow!("its clause {place}, which has no name, is answered by no departure")
            }),
        })
        .collect()
}

/// A value of `declared` built from `fields`, the way [`construction`] says one is made where it
/// stands.
fn construct(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    abort: ir::Block,
    declared: &str,
    fields: &[ir::Value],
) -> Lowered<ir::Value> {
    let declaration = lowering.declared.laid(declared);
    match construction(declaration) {
        Construction::Laid => lay_out(
            builder,
            module,
            lowering.declared,
            lowering.allocate,
            declaration,
            fields,
        ),
        Construction::Called => {
            let constructor = lowering.constructors.of(declared)?;
            Ok(call_reached(
                builder,
                module,
                abort,
                constructor,
                POINTER,
                fields,
            ))
        }
    }
}

/// A value of `declaration` laid out from its fields, which are already ones the type admits: room
/// for them, the token that says which type it is, and each field in its slot.
///
/// The one place a value of a declared type is laid out, whether a constructor lays it out once its
/// clauses hold or a construction with none to run lays it out where it stands.
fn lay_out(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    declared: &Declared,
    allocate: FuncId,
    declaration: &Declaration,
    fields: &[ir::Value],
) -> Lowered<ir::Value> {
    let taking = module.declare_func_in_func(allocate, builder.func);
    let size = builder
        .ins()
        .iconst(types::I64, room_for_fields(fields.len()));
    let taken = builder.ins().call(taking, &[size]);
    let value = builder.inst_results(taken)[0];
    let token = declared.tag(module, &declaration.key())?;
    let named = module.declare_data_in_func(token, builder.func);
    let which = builder.ins().symbol_value(POINTER, named);
    builder.ins().store(TRUSTED, which, value, WHICH as i32);
    for (at, &field) in fields.iter().enumerate() {
        let held = into_slot(builder, field);
        builder
            .ins()
            .store(TRUSTED, held, value, field_at(at) as i32);
    }
    Ok(value)
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
    composed: &Target,
    written: &Definition,
    frontend: TargetFrontendConfig,
    lowering: &Lowerings,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let Definition::Composed {
        requirements: requires,
        stages,
        ..
    } = written
    else {
        unreachable!("a composition is lowered from what composes it");
    };
    let takes = composed.takes().len();
    let answers = composed.answers();
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);
    let handed = builder.block_params(entry)[0];
    let out = builder.block_params(entry)[1 + takes];

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);

    let (first, rest) = stages
        .split_first()
        .expect("`Coherent` held every composition to compose something");
    let arguments: Vec<ir::Value> = builder.block_params(entry)[1..=takes].to_vec();
    let through = stage_through(
        &mut builder,
        lowering,
        module,
        abort,
        requires,
        handed,
        &first.behavior,
    );
    let mut running = call_behavior(
        &mut builder,
        lowering,
        module,
        abort,
        &first.behavior,
        through,
        &arguments,
    )?;
    // What the value running is, which is what a stage is handed it from and what the composition
    // answers it from: a stage takes it, and the composition answers it, as a type of their own,
    // and `restate` holds it the way that type is held.
    let mut running_is = lowering.targets.reached(&first.behavior).answers();

    for stage in rest {
        let reached = lowering.targets.reached(&stage.behavior);
        let [taken] = reached.takes().try_into().unwrap_or_else(|_: Vec<Ty>| {
            unreachable!("`Coherent` held every stage after the first to take one value")
        });
        match &stage.routing {
            Routing::Always => {
                let offered =
                    restate(&mut builder, lowering, module, running, &running_is, &taken)?;
                let through = stage_through(
                    &mut builder,
                    lowering,
                    module,
                    abort,
                    requires,
                    handed,
                    &stage.behavior,
                );
                running = call_behavior(
                    &mut builder,
                    lowering,
                    module,
                    abort,
                    &stage.behavior,
                    through,
                    &[offered],
                )?;
            }
            Routing::OnCases { accepted } => {
                let tagged = Tagged::of(running, &running_is);
                let accepts = is_one_of_cases(&mut builder, lowering, module, tagged, accepted)?;
                let offer = builder.create_block();
                let leave = builder.create_block();
                builder.ins().brif(accepts, offer, &[], leave, &[]);
                builder.seal_block(offer);
                builder.seal_block(leave);

                // What left the main line is answered here, at the stage that did not accept it,
                // rather than carried along to be tested against a stage further on.
                builder.switch_to_block(leave);
                let left = restate(
                    &mut builder,
                    lowering,
                    module,
                    running,
                    &running_is,
                    &answers,
                )?;
                builder.ins().store(TRUSTED, left, out, 0);
                let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
                builder.ins().return_(&[ok]);

                builder.switch_to_block(offer);
                let offered =
                    restate(&mut builder, lowering, module, running, &running_is, &taken)?;
                let through = stage_through(
                    &mut builder,
                    lowering,
                    module,
                    abort,
                    requires,
                    handed,
                    &stage.behavior,
                );
                running = call_behavior(
                    &mut builder,
                    lowering,
                    module,
                    abort,
                    &stage.behavior,
                    through,
                    &[offered],
                )?;
            }
        }
        running_is = reached.answers();
    }

    let answered = restate(
        &mut builder,
        lowering,
        module,
        running,
        &running_is,
        &answers,
    )?;
    builder.ins().store(TRUSTED, answered, out, 0);
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);

    builder.seal_block(abort);
    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.finalize(frontend);
    Ok(())
}

/// How a composition constructed with `requirements`, handed as `handed`, applies its stage
/// `stage` (spec §composition-with-requirements).
///
/// A stage a host implements is one of what the composition was handed, and is called through that
/// capability. Any other is built by the composition, whichever build implements it: its symbol,
/// handed the capabilities of what its target says it requires, picked out of the composition's own
/// in the stage's order. Those are laid out in room taken from the arena and not on this function's
/// stack, since the stage may make a function value that carries them past this call's end, the
/// way the JVM's composition holds the stage it built for as long as the stage is held. One
/// requiring nothing is handed nothing.
fn stage_through(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    abort: ir::Block,
    requirements: &[Requirement],
    handed: ir::Value,
    stage: &str,
) -> Through {
    let at = |behavior: &str| {
        requirements
            .iter()
            .position(|it| it.declared() == behavior)
            .expect("`Coherent` held every stage to be handed what it requires")
    };
    let reached = lowering.targets.reached(stage);
    if reached.is == transport::Answers::Injected {
        return Through::Capability(capability_at(builder, abort, handed, at(stage)));
    }
    let stage_requires = &reached.requirements;
    if stage_requires.is_empty() {
        return Through::Symbol(builder.ins().iconst(POINTER, 0));
    }
    handed_or_unbound(builder, abort, handed);
    let picked = lowering.room(builder, module, room_for_requirements(stage_requires.len()));
    for (position, required) in stage_requires.iter().enumerate() {
        let capability = builder.ins().load(
            POINTER,
            TRUSTED,
            handed,
            requirement_at(at(&required.declared())) as i32,
        );
        builder
            .ins()
            .store(TRUSTED, capability, picked, requirement_at(position) as i32);
    }
    Through::Symbol(picked)
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
/// status other than `ANSWERED` is not this call's to interpret: it was decided once where it
/// arose, through `native_status` at whichever site first left its range or ran out of
/// representation, or at the object answering a behavior a host implements, which is where a
/// host's own status is held to what a host may answer. Asking what it means a second time here
/// would be the reclassification issue #9 exists to rule out. So it is not read; it is forwarded, to `abort`, exactly as it arrived — which is what makes
/// a callee's abort cross a call boundary the same way an answer does, transparently, all the way
/// out to whichever caller first receives a status that is not zero.
fn call_reached(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    abort: ir::Block,
    reached: FuncId,
    answers: types::Type,
    arguments: &[ir::Value],
) -> ir::Value {
    let reaching = module.declare_func_in_func(reached, builder.func);
    let out = out_slot(builder);
    let mut given = arguments.to_vec();
    given.push(out);
    let called = builder.ins().call(reaching, &given);
    let status = builder.inst_results(called)[0];
    status_or_answer(builder, abort, status, out, answers)
}

/// How a call reaches the behavior it applies: its symbol, handed an environment, or a capability
/// the caller holds for it.
///
/// Decided by the caller and not by the behavior: whether the behavior is one the caller was
/// constructed with is the caller's, and a behavior reached through a capability is answered by
/// whatever the capability holds — a body, a host's implementation, a row's stand-in — and never by
/// the symbol a call would recover from its name.
#[derive(Clone, Copy)]
enum Through {
    /// The behavior's symbol, handed this as what it was constructed with.
    Symbol(ir::Value),
    /// The address of a capability for the behavior.
    Capability(ir::Value),
}

/// A behavior applied to `arguments` the way `through` says, and its answer where it keeps what
/// the behavior declares.
///
/// The one way a behavior is applied, whether a body calls it, a row runs it or a composition's
/// stage applies it, so what is done about the answer is decided here for all of them. Where the
/// answer arrives from outside it is held here, as it crosses in; where the callee holds its own
/// answer, or nothing does, there is nothing for a caller to do.
///
/// The rules are not written here. They are the declaring module's, lowered once where it holds
/// them ([`define_rules`]), and this calls that.
fn call_behavior(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    abort: ir::Block,
    declared: &str,
    through: Through,
    arguments: &[ir::Value],
) -> Lowered<ir::Value> {
    let target = lowering.targets.reached(declared);
    let answers = machine_type(&target.answers())?;
    let answer = match through {
        // A row of a behavior a host implements: nothing in the object answers it, and a row states
        // no capability of the behavior it is a row of, so the run is handed nothing for it.
        Through::Symbol(_) if target.is == transport::Answers::Injected => {
            let status = builder
                .ins()
                .iconst(types::I32, i64::from(INJECTION_UNBOUND));
            builder.ins().jump(abort, &[status.into()]);
            let unreached = builder.create_block();
            builder.seal_block(unreached);
            builder.switch_to_block(unreached);
            builder.ins().iconst(answers, 0)
        }
        Through::Symbol(environment) => {
            let reached = lowering.reachable.of_behavior_named(declared);
            let mut given = Vec::with_capacity(arguments.len() + 1);
            given.push(environment);
            given.extend_from_slice(arguments);
            call_reached(builder, module, abort, reached, answers, &given)
        }
        Through::Capability(capability) => {
            let signature = behavior_signature(
                &target.takes(),
                &target.answers(),
                module.isa().default_call_conv(),
            )?;
            let code = builder
                .ins()
                .load(POINTER, TRUSTED, capability, CAPABILITY_INVOKE as i32);
            let environment =
                builder
                    .ins()
                    .load(POINTER, TRUSTED, capability, CAPABILITY_ENVIRONMENT as i32);
            let signature = builder.import_signature(signature);
            let out = out_slot(builder);
            let mut given = Vec::with_capacity(arguments.len() + 2);
            given.push(environment);
            given.extend_from_slice(arguments);
            given.push(out);
            let called = builder.ins().call_indirect(signature, code, &given);
            let status = builder.inst_results(called)[0];
            status_or_answer(builder, abort, status, out, answers)
        }
    };
    match target.ensures {
        Ensures::Crossing { .. } => {
            let rules = lowering.reachable.of_rules(declared);
            hold(builder, module, abort, rules, arguments, answer);
        }
        // Held by the callee, on its way out: every way in reaches its symbol, which holds it.
        Ensures::Callee { .. } => {}
        Ensures::None => {}
        // Another build's behavior, whose clause nothing in this compile runs: what is done about
        // it is not decided here, and a check made up here would be this compile deciding it.
        Ensures::Undecided => {}
    }
    Ok(answer)
}

/// The address of the capability at `at` among `requirements`, where there is one: a caller handed
/// null requirements, or a null in the place of one, is handed nothing for the behavior, and the
/// call answers [`INJECTION_UNBOUND`] rather than reading behind a null.
fn capability_at(
    builder: &mut FunctionBuilder,
    abort: ir::Block,
    requirements: ir::Value,
    at: usize,
) -> ir::Value {
    handed_or_unbound(builder, abort, requirements);
    let capability = builder
        .ins()
        .load(POINTER, TRUSTED, requirements, requirement_at(at) as i32);
    handed_or_unbound(builder, abort, capability);
    capability
}

/// The run carried on past `address` where it is not null, and ended with [`INJECTION_UNBOUND`]
/// where it is.
fn handed_or_unbound(builder: &mut FunctionBuilder, abort: ir::Block, address: ir::Value) {
    let handed = builder.create_block();
    let unbound = builder.create_block();
    builder.ins().brif(address, handed, &[], unbound, &[]);
    builder.seal_block(handed);
    builder.seal_block(unbound);

    builder.switch_to_block(unbound);
    let status = builder
        .ins()
        .iconst(types::I32, i64::from(INJECTION_UNBOUND));
    builder.ins().jump(abort, &[status.into()]);

    builder.switch_to_block(handed);
}

fn hold(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    abort: ir::Block,
    rules: FuncId,
    arguments: &[ir::Value],
    answer: ir::Value,
) {
    let reaching = module.declare_func_in_func(rules, builder.func);
    let mut given = arguments.to_vec();
    given.push(answer);
    let called = builder.ins().call(reaching, &given);
    let status = builder.inst_results(called)[0];
    forward_unless_answered(builder, abort, status);
}

/// What holds an answer to what its behavior declares takes and answers: what the behavior takes,
/// then the answer, and a status. Nothing is written back, since there is nothing to answer beyond
/// whether the answer is kept.
fn holding_signature(takes: &[Ty], answers: &Ty, call_conv: CallConv) -> Lowered<ir::Signature> {
    let mut signature = ir::Signature::new(call_conv);
    for taken in takes.iter().chain([answers]) {
        signature.params.push(AbiParam::new(machine_type(taken)?));
    }
    signature.returns.push(AbiParam::new(types::I32));
    Ok(signature)
}

/// A behavior held at the callee, under its own symbol: its body run under the name it moved to,
/// and the answer held to what the behavior declares before it is answered.
///
/// Held at the symbol and not at the end of the body. A body answers from one place today, but
/// what holds the answer is then a property of every way the body can come to answer, where here it
/// is a property of the one way anything reaches the behavior at all.
fn define_held(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    target: &Target,
    unheld: FuncId,
    rules: FuncId,
    frontend: TargetFrontendConfig,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);
    let given = builder.block_params(entry).to_vec();
    // What it was constructed with, handed on as it came, then what it takes.
    let (constructed, arguments, out) = (
        &given[..1],
        &given[1..=target.inputs.len()],
        given[target.inputs.len() + 1],
    );

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);

    let answers = machine_type(&target.answers())?;
    let handed = [constructed, arguments].concat();
    let answer = call_reached(&mut builder, module, abort, unheld, answers, &handed);
    hold(&mut builder, module, abort, rules, arguments, answer);
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

/// What holds an answer to what its behavior declares: every rule whose guard the answer meets,
/// in the order the checker keeps them, until one does not hold.
///
/// Every rule and not the first whose guard holds, because a declaration states a conjunction and
/// one answer may be a case two rules name. A rule that does not hold ends the check with
/// `EnsuresNotHeld`; a rule that itself ends without an answer, dividing by nought or leaving an
/// `Int`'s range, ends it with that status instead, since it did not answer false.
///
/// The parameters are bound under where each stands, as a body's are, and the answer under the
/// number each rule reads it by, as what the rule's guard says it is read as.
fn define_rules(
    function: &mut Function,
    shapes: &mut FunctionBuilderContext,
    target: &Target,
    rules: &[transport::Body],
    frontend: TargetFrontendConfig,
    lowerings: &Lowerings,
    module: &mut ObjectModule,
) -> Lowered<()> {
    let mut builder = FunctionBuilder::new(function, shapes);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let contract = target
        .ensures
        .contract()
        .expect("what holds an answer is defined only for a behavior that declares something");
    let takes = target.takes();
    let mut bindings = Bindings::default();
    for (at, taken) in takes.iter().enumerate() {
        let variable = builder.declare_var(machine_type(taken)?);
        let given = builder.block_params(entry)[at];
        builder.def_var(variable, given);
        bindings.at(at, variable);
    }
    let answer = builder.block_params(entry)[takes.len()];
    let answers = target.answers();

    let abort = builder.create_block();
    builder.append_block_param(abort, types::I32);
    let broken = builder.create_block();

    assert_eq!(
        rules.len(),
        contract.rules.len(),
        "every rule of a contract is a body this object runs"
    );
    for (at, body) in rules.iter().enumerate() {
        assert!(
            matches!(body.owner, Owner::Ensures { at: place, .. } if place == at),
            "the program lists a contract's rules in the order they run"
        );
        let rule = &contract.rules[at];
        let lowering = lowerings.at(body.carrier());
        // Where the rule applies, and past it where it does not.
        let next = builder.create_block();
        let read = match &rule.guard {
            Guard::Always => answer,
            Guard::Case { selects, binds } => {
                let selects = std::slice::from_ref(selects);
                let applies = builder.create_block();
                let asked = tests(&mut builder, &lowering, module, answer, &answers, selects)?;
                builder.ins().brif(asked, applies, &[], next, &[]);
                builder.seal_block(applies);
                builder.switch_to_block(applies);
                self::binds(
                    &mut builder,
                    &lowering,
                    module,
                    answer,
                    &answers,
                    selects,
                    binds,
                )?
            }
        };
        let variable = builder.declare_var(machine_type(rule.guard.reads_as(&answers))?);
        builder.def_var(variable, read);
        bindings.at(rule.value, variable);
        let holds = lower(
            &mut builder,
            &lowering,
            module,
            &mut bindings,
            abort,
            body.node,
        );
        bindings.leave(rule.value);
        builder.ins().brif(holds?, next, &[], broken, &[]);
        builder.seal_block(next);
        builder.switch_to_block(next);
    }
    let ok = builder.ins().iconst(types::I32, i64::from(ANSWERED));
    builder.ins().return_(&[ok]);

    builder.seal_block(broken);
    builder.switch_to_block(broken);
    let not_held = builder.ins().iconst(
        types::I32,
        i64::from(native_status(AbortKind::EnsuresNotHeld)),
    );
    builder.ins().return_(&[not_held]);

    builder.seal_block(abort);
    builder.switch_to_block(abort);
    let status = builder.block_params(abort)[0];
    builder.ins().return_(&[status]);

    builder.finalize(frontend);
    Ok(())
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
) -> Lowered<ir::Value> {
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
    forward_unless_answered(builder, abort, status);
    builder.ins().load(answers, TRUSTED, out, 0)
}

/// A status other than `ANSWERED` forwarded to `abort` exactly as it arrived, and the run carried
/// on past it otherwise.
///
/// Shared by every call whose status this function does not interpret: a callee's, and what holds
/// an answer to what its behavior declares.
fn forward_unless_answered(builder: &mut FunctionBuilder, abort: ir::Block, status: ir::Value) {
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
}

/// What the document's numbers for a behavior's bindings stand for here.
///
/// Held by the number rather than pushed in the order they are met: the writer numbers a binder
/// where it writes it and this lowers a binding's value before the binder exists, so an order
/// either side happened to have would only agree until a binding's value held a binding of its own.
///
/// A number is an identity and not a position, so it is a key and never an index: a table sized
/// by the number would take as much room as the largest one a document happens to write, which the
/// writer keeps small by counting and nothing in what is read does.
///
/// A number names one binder in force at a time, which [`Coherent`] held. So binding one that is
/// already in force is this compiler's own mistake and stops it, and is never a scope quietly
/// replaced: a lowering that kept the inner binder after its scope closed would read it for what the
/// outer one meant.
#[derive(Default)]
struct Bindings {
    held: HashMap<usize, Variable>,
    /// The bindings holding what a walk grows a list in, which is laid out as nothing else is
    /// ([`Growing`]): the step's accumulator, and every name a `let` gives it.
    growing: HashSet<usize>,
    /// What the function was handed capabilities for, where it was handed any.
    environment: Option<Environment>,
    /// The behavior a row's entry constructs, and what it constructs it with.
    row: Option<(String, ir::Value)>,
}

/// The capabilities a function was handed: the variable holding their address, and the behavior
/// each is for, in the order they stand ([`transport::Body::environment`]).
struct Environment {
    held: Variable,
    requires: Vec<String>,
}

impl Environment {
    fn of(held: Variable, requires: &[Requirement]) -> Environment {
        Environment {
            held,
            requires: requires.iter().map(Requirement::declared).collect(),
        }
    }
}

impl Bindings {
    fn at(&mut self, number: usize, variable: Variable) {
        let before = index::Index::put(&mut self.held, number, variable);
        assert!(
            before.is_none(),
            "`Coherent` held every binder's number to name one binder in force"
        );
    }

    /// `number` out of force, at the end of the scope that bound it.
    fn leave(&mut self, number: usize) {
        self.held.remove(&number);
        self.growing.remove(&number);
    }

    /// `number`, already in force, as a name for what a walk grows a list in.
    fn grows(&mut self, number: usize) {
        assert!(
            self.held.contains_key(&number),
            "a binding is in force before it is said to grow"
        );
        self.growing.insert(number);
    }

    /// What a walk grows a list in, where `node` reads a name for it, standing as whatever it
    /// stands as.
    fn grown(&self, node: &Node) -> Option<usize> {
        let read = match node {
            Node::Widen { value, .. } => value.as_ref(),
            other => other,
        };
        match read {
            Node::Read { binding, .. } if self.growing.contains(binding) => Some(*binding),
            _ => None,
        }
    }

    fn of(&self, number: usize) -> Variable {
        *self
            .held
            .get(&number)
            .expect("`Coherent` held every read to be of a binding in scope")
    }

    /// How a call from here reaches `declared`: through the capability this function was handed
    /// for it, where it was handed one, and by its symbol, handed nothing, otherwise — which
    /// `Coherent` held to be a behavior that requires nothing.
    fn through(&self, builder: &mut FunctionBuilder, abort: ir::Block, declared: &str) -> Through {
        if let Some((constructs, requirements)) = &self.row
            && constructs == declared
        {
            return Through::Symbol(*requirements);
        }
        if let Some(environment) = &self.environment
            && let Some(at) = environment.requires.iter().position(|it| it == declared)
        {
            let requirements = builder.use_var(environment.held);
            return Through::Capability(capability_at(builder, abort, requirements, at));
        }
        Through::Symbol(builder.ins().iconst(POINTER, 0))
    }
}

fn lower(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    abort: ir::Block,
    node: &Node,
) -> Lowered<ir::Value> {
    Ok(match node {
        Node::Int { value, ty, .. } => builder.ins().iconst(machine_type(ty)?, *value),
        Node::Read { binding, .. } => {
            let variable = bindings.of(*binding);
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
            // The width first: a `Decimal` or a `Rational` has none here, and is refused before its
            // negation is asked what it can end for. An `Int` names the one reason `Coherent` held
            // it to.
            let width = machine_type(operand.ty())?;
            let held = lower(builder, lowering, module, bindings, abort, operand)?;
            let nought = builder.ins().iconst(width, 0);
            difference(builder, abort, overflow_status(aborts), nought, held)
        }
        // A fork answers what the branch it takes answers, and each branch hands that to the block
        // after the fork. Which nodes are forks, and how each chooses a branch, is `branched`'s.
        Node::Let { ty, .. }
        | Node::If { ty, .. }
        | Node::Match { ty, .. }
        | Node::Attempt { ty, .. } => {
            let after = builder.create_block();
            builder.append_block_param(after, machine_type(ty)?);
            let forked = branched(
                builder,
                lowering,
                module,
                bindings,
                abort,
                node,
                &mut |builder, module, bindings, branch| {
                    let answered = lower(builder, lowering, module, bindings, abort, branch)?;
                    builder.ins().jump(after, &[answered.into()]);
                    Ok(())
                },
            )?;
            assert!(forked, "a let, an if, a match and an attempt are forks");
            builder.seal_block(after);
            builder.switch_to_block(after);
            builder.block_params(after)[0]
        }
        Node::Bool { value, ty, .. } => builder.ins().iconst(machine_type(ty)?, i64::from(*value)),
        Node::Str { value, .. } => lowering.literals.address(builder, module, value),
        Node::Binary {
            op,
            reading,
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
                reading,
                left,
                right,
                aborts,
            },
        )?,
        Node::Unit { declared, .. } => construct(builder, lowering, module, abort, declared, &[])?,
        // The fields are worked out here, in the order they are written; whether the value is one
        // the type admits, and how one is laid out, is not this site's to say (`construct`).
        Node::Construct {
            declared, values, ..
        } => {
            let mut given = Vec::with_capacity(values.len());
            for value in values {
                given.push(lower(builder, lowering, module, bindings, abort, value)?);
            }
            construct(builder, lowering, module, abort, declared, &given)?
        }
        Node::Field {
            target, field, ty, ..
        } => {
            let of = target.ty();
            let Ty::Declared { declared } = of else {
                unreachable!("`Coherent` held every field read to be of a declared type");
            };
            let value = lower(builder, lowering, module, bindings, abort, target)?;
            match lowering.declared.laid(declared) {
                Declaration::Sum { .. } => {
                    shared_field(builder, lowering, module, value, of, field, ty)?
                }
                shape => {
                    let at = shape.position_of(field).expect(
                        "`Coherent` held every field read to be one its declaration declares",
                    );
                    let held = builder
                        .ins()
                        .load(types::I64, TRUSTED, value, field_at(at) as i32);
                    out_of_slot(builder, held, machine_type(ty)?)
                }
            }
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
        // Its length and then its elements, as `souther-native-abi` lays a list out. The empty
        // list is room for the length alone, and never the null an absent value is.
        Node::List { elements, .. } => {
            let mut held = Vec::with_capacity(elements.len());
            for element in elements {
                let answered = lower(builder, lowering, module, bindings, abort, element)?;
                held.push(into_slot(builder, answered));
            }
            let count = elements.len() as i64;
            let flags = TRUSTED;
            let value = lowering.room(builder, module, room_for_list(count));
            let length = builder.ins().iconst(types::I64, count);
            builder
                .ins()
                .store(flags, length, value, LIST_LENGTH as i32);
            for (at, element) in held.into_iter().enumerate() {
                builder
                    .ins()
                    .store(flags, element, value, list_at(at as i64) as i32);
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
            // A behavior is applied the one way every behavior is, which is where what is done
            // about its answer is decided (`call_behavior`).
            Reaches::Behavior { declared } => {
                let mut given = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    given.push(lower(builder, lowering, module, bindings, abort, argument)?);
                }
                let through = bindings.through(builder, abort, declared);
                call_behavior(builder, lowering, module, abort, declared, through, &given)?
            }
            Reaches::Emitted { operation } => match operation {
                Emitted::BuildList => build_list(builder, lowering, module, bindings, abort, node)?,
                Emitted::GrowList => {
                    let [grown, added] = arguments.as_slice() else {
                        unreachable!(
                            "`Coherent` held {} to the two arguments it takes",
                            operation.spelt()
                        );
                    };
                    assert!(
                        bindings.grown(grown).is_some(),
                        "`growing` held every growth to add to what its own walk grows"
                    );
                    let growing =
                        Growing(lower(builder, lowering, module, bindings, abort, grown)?);
                    // A list written out where it is added is added element by element, with no
                    // list made of them first: `acc ++ [y]` is what `map` and `filter` grow by.
                    if let Node::List { elements, .. } = added {
                        for element in elements {
                            let held = lower(builder, lowering, module, bindings, abort, element)?;
                            growing.add(builder, lowering, module, held);
                        }
                    } else {
                        let held = lower(builder, lowering, module, bindings, abort, added)?;
                        growing.add_all(builder, lowering, module, held);
                    }
                    growing.0
                }
                Emitted::BuildMap | Emitted::PutMap => unreachable!(
                    "`Coherent` refused {} as not lowered wherever it runs",
                    operation.spelt()
                ),
            },
            Reaches::Helper { .. } | Reaches::Value { .. } | Reaches::PublishedValue { .. } => {
                let reached = match reaches {
                    // The copy `Specializations` resolved this call to, and not one worked out here
                    // again from the name and the types.
                    Reaches::Helper { reached: _ } => lowering
                        .reachable
                        .of_instance(lowering.specializations.callee(node)),
                    Reaches::Value { module, name } => lowering
                        .reachable
                        .of_value(lowering.carrier, &format!("{module}.{name}")),
                    Reaches::PublishedValue { module, name } => {
                        lowering.reachable.of_published_value(module, name)
                    }
                    Reaches::Behavior { .. } | Reaches::Kernel { .. } | Reaches::Emitted { .. } => {
                        unreachable!()
                    }
                };
                let mut given = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    given.push(lower(builder, lowering, module, bindings, abort, argument)?);
                }
                call_reached(builder, module, abort, reached, machine_type(ty)?, &given)
            }
            // The kernels this backend lowers are `kernels::Lowered`'s and nowhere else's, so one it
            // has not met falls to NotLowered rather than a list here claiming to know. What one
            // takes is that table's contract and not the document's word: `Coherent` held the
            // settlement to it, so the arguments are exactly as many as the kernel takes.
            Reaches::Kernel { kernel, .. } => match LoweredKernel::of(kernel) {
                Some(LoweredKernel::IntAdd) => {
                    let [left, right] = arguments.as_slice() else {
                        unreachable!("`Coherent` held int.add to the two arguments it takes");
                    };
                    let a = Held::of(
                        left,
                        lower(builder, lowering, module, bindings, abort, left)?,
                    );
                    let b = Held::of(
                        right,
                        lower(builder, lowering, module, bindings, abort, right)?,
                    );
                    arithmetic(builder, abort, Op::Add, a, b, aborts)?
                }
                Some(LoweredKernel::ListLength) => {
                    let [list] = arguments.as_slice() else {
                        unreachable!("`Coherent` held list.length to the one argument it takes");
                    };
                    let list = lower(builder, lowering, module, bindings, abort, list)?;
                    builder
                        .ins()
                        .load(types::I64, TRUSTED, list, LIST_LENGTH as i32)
                }
                // The element's own slot, which is what an `Option` holding it points at: nothing
                // is copied and nothing taken from the arena. An index is in the list where it is
                // below the length read without a sign, so a negative one, read as a very large
                // one, is outside it as well. The address is worked out either way and only
                // answered where the index is inside.
                Some(LoweredKernel::ListGet) => {
                    let [index, list] = arguments.as_slice() else {
                        unreachable!("`Coherent` held list.get to the two arguments it takes");
                    };
                    let index = lower(builder, lowering, module, bindings, abort, index)?;
                    let list = lower(builder, lowering, module, bindings, abort, list)?;
                    let length = builder
                        .ins()
                        .load(types::I64, TRUSTED, list, LIST_LENGTH as i32);
                    let inside = builder.ins().icmp(IntCC::UnsignedLessThan, index, length);
                    let along = builder.ins().imul_imm_s(index, SLOT);
                    let at = builder.ins().iadd(list, along);
                    let slot = builder.ins().iadd_imm_s(at, list_at(0));
                    let nothing = builder.ins().iconst(POINTER, NOTHING);
                    builder.ins().select(inside, slot, nothing)
                }
                Some(
                    divided @ (LoweredKernel::IntTruncatingDivide
                    | LoweredKernel::IntTruncatingRemainder),
                ) => {
                    let [dividend, divisor] = arguments.as_slice() else {
                        unreachable!("`Coherent` held {kernel} to the two arguments it takes");
                    };
                    let dividend = lower(builder, lowering, module, bindings, abort, dividend)?;
                    let divisor = lower(builder, lowering, module, bindings, abort, divisor)?;
                    let answering = match divided {
                        LoweredKernel::IntTruncatingDivide => Division::Quotient,
                        _ => Division::Remainder,
                    };
                    let division = Dividing {
                        answering,
                        dividend,
                        divisor,
                        aborts,
                    };
                    truncating_division(builder, lowering, module, abort, division)?
                }
                Some(LoweredKernel::StringLength) => {
                    let [text] = arguments.as_slice() else {
                        unreachable!("`Coherent` held string.length to the one argument it takes");
                    };
                    let text = lower(builder, lowering, module, bindings, abort, text)?;
                    let counting = module.declare_func_in_func(lowering.count_text, builder.func);
                    let counted = builder.ins().call(counting, &[text]);
                    builder.inst_results(counted)[0]
                }
                None => return Err(not_lowered(format!("a call to the kernel {kernel}"))),
            },
        },
        // Standing as a wider type is no operation in the language, and here it costs nothing
        // almost everywhere: a case and the sum or union it is a case of are both the address of
        // a value carrying its token, and an optional, a tuple or a function over them is laid out
        // alike. A primitive is the exception, since it carries no token and a union's value has
        // to say which case it is; `restate` carries it. The type it stands as is asked for its
        // representation all the same, so one this backend has none for is refused here as it is
        // anywhere else.
        Node::Widen {
            value: narrower,
            ty,
            ..
        } => {
            machine_type(ty)?;
            let held = lower(builder, lowering, module, bindings, abort, narrower)?;
            restate(builder, lowering, module, held, narrower.ty(), ty)?
        }
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
                .expect("every closure site was planned before any body was lowered");
            let code_id = *lowering
                .lifted
                .get(site)
                .expect("every closure site was declared a lifted function before any was defined");

            let flags = TRUSTED;
            let carried = plan.captures.len() + usize::from(plan.environment.is_some());
            let value = lowering.room(builder, module, room_for_closure(carried));

            let code_ref = module.declare_func_in_func(code_id, builder.func);
            let code = builder.ins().func_addr(POINTER, code_ref);
            builder.ins().store(flags, code, value, CLOSURE_CODE as i32);

            for (position, capture) in plan.captures.iter().enumerate() {
                let variable = bindings.of(capture.binding);
                let held = builder.use_var(variable);
                let held = into_slot(builder, held);
                builder
                    .ins()
                    .store(flags, held, value, capture_at(position) as i32);
            }
            // What the function making it was handed, after the captures, where the site calls
            // through it.
            if plan.environment.is_some() {
                let environment = bindings
                    .environment
                    .as_ref()
                    .expect("a site reaching the environment stands where there is one");
                let held = builder.use_var(environment.held);
                builder
                    .ins()
                    .store(flags, held, value, capture_at(plan.captures.len()) as i32);
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
                unreachable!("`Coherent` held every application to be of a function type");
            };
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

/// A field read off a value of a sum, `of`, which is the field of whichever case the value is.
///
/// Each case lays its own fields out, and one it takes in by spread need not stand where it stands
/// in another case, so the value is told apart by its token first and the field read where that
/// case lays it, then held as what the read answers. The last case is the one a value tagged by
/// none of the others is, which the checker settles every value of the sum to be one of.
fn shared_field(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    value: ir::Value,
    of: &Ty,
    field: &str,
    ty: &Ty,
) -> Lowered<ir::Value> {
    let Ty::Declared { declared } = of else {
        unreachable!("a field is read off a sum only where the sum is its target's type");
    };
    let cases = lowering
        .declared
        .leaves_of(&[Case::Declared {
            declared: declared.clone(),
        }])
        .expect("`Coherent` held every case of the sum to be one a declaration crossed for");
    let which = Tagged::of(value, of).which(builder);
    let read = builder.create_block();
    builder.append_block_param(read, machine_type(ty)?);
    for (at, case) in cases.iter().enumerate() {
        let Case::Declared { declared: key } = case else {
            unreachable!("`Coherent` held every case a field is read off to be declared");
        };
        let next = if at + 1 < cases.len() {
            let this = builder.create_block();
            let next = builder.create_block();
            let token = token_of(builder, lowering.declared, module, case)?;
            let is_it = builder.ins().icmp(IntCC::Equal, which, token);
            builder.ins().brif(is_it, this, &[], next, &[]);
            builder.seal_block(this);
            builder.switch_to_block(this);
            Some(next)
        } else {
            None
        };
        let laid = lowering.declared.laid(key);
        let position = laid
            .position_of(field)
            .expect("`Coherent` held every case of the sum to lay the field out");
        let laid_as = laid.fields()[position].codec.ty();
        let held = builder
            .ins()
            .load(types::I64, TRUSTED, value, field_at(position) as i32);
        let held = out_of_slot(builder, held, machine_type(&laid_as)?);
        let held = restate(builder, lowering, module, held, &laid_as, ty)?;
        builder.ins().jump(read, &[held.into()]);
        if let Some(next) = next {
            builder.seal_block(next);
            builder.switch_to_block(next);
        }
    }
    builder.seal_block(read);
    builder.switch_to_block(read);
    Ok(builder.block_params(read)[0])
}

/// Where `node` is a fork, what chooses its branch, with each branch ended by `branch`; and
/// whether it is one.
///
/// A fork answers what the branch it takes answers: the body of a `let`, a branch of an `if`, an
/// arm of a `match`, and what an attempted construction goes on to where its clauses hold or
/// where one does not. How a branch ends is the caller's: [`lower`] hands its value to the block
/// after the fork, and [`lower_tail`] lowers it where a copy of a helper answers, so a call to
/// itself in a branch of a fork in tail position is in tail position too. Every block `branch`
/// is handed is one it has to end.
///
/// The one place a kind of node is said to fork or not. Every kind is named here and no arm
/// stands for the rest, so a kind added to the document that forks is lowered as a fork by both
/// callers, and one that does not says so, rather than falling to whichever a default picked.
///
/// The arms of a `match` are tried in the order they are written, because that is the order the
/// language reads them in. Running out of them, or out of the clauses an attempt answers, is this
/// compiler having emitted the wrong test: the checker settles that a fork always answers.
fn branched(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    abort: ir::Block,
    node: &Node,
    branch: &mut dyn FnMut(
        &mut FunctionBuilder,
        &mut ObjectModule,
        &mut Bindings,
        &Node,
    ) -> Lowered<()>,
) -> Lowered<bool> {
    match node {
        Node::Let {
            binding,
            binds,
            value,
            body,
            ..
        } => {
            let held = lower(builder, lowering, module, bindings, abort, value)?;
            let variable = builder.declare_var(machine_type(binds)?);
            builder.def_var(variable, held);
            bindings.at(*binding, variable);
            if bindings.grown(value).is_some() {
                bindings.grows(*binding);
            }
            let answered = branch(builder, module, bindings, body);
            bindings.leave(*binding);
            answered?;
        }
        Node::If {
            cond, then, els, ..
        } => {
            let asked = lower(builder, lowering, module, bindings, abort, cond)?;
            let when_taken = builder.create_block();
            let otherwise = builder.create_block();
            builder.ins().brif(asked, when_taken, &[], otherwise, &[]);
            builder.seal_block(when_taken);
            builder.seal_block(otherwise);
            for (block, taken) in [(when_taken, then), (otherwise, els)] {
                builder.switch_to_block(block);
                branch(builder, module, bindings, taken)?;
            }
        }
        Node::Match { subject, arms, .. } => {
            let value = lower(builder, lowering, module, bindings, abort, subject)?;
            for arm in arms {
                let next = enter_arm(
                    builder,
                    lowering,
                    module,
                    bindings,
                    value,
                    subject.ty(),
                    arm,
                )?;
                let answered = branch(builder, module, bindings, &arm.body);
                if let Some(number) = arm.binding {
                    bindings.leave(number);
                }
                answered?;
                builder.switch_to_block(next);
            }
            builder
                .ins()
                .trap(TrapCode::user(NO_ARM).expect("a trap code of its own"));
        }
        // The fields as a construction works them out, and then what decides a construction of the
        // type asked, which is the declaring object's: nothing here runs a clause or knows what one
        // says. Where every clause held the value is bound for `then`; where one did not, the
        // departure answering that clause is taken, chosen by the clause's place
        // (`departures_taken`). A clause that did not answer at all ends the run as it ended the
        // decision.
        Node::Attempt {
            declared,
            values,
            binding,
            binds,
            then,
            departures,
            ..
        } => {
            let mut given = Vec::with_capacity(values.len());
            for value in values {
                given.push(lower(builder, lowering, module, bindings, abort, value)?);
            }
            let checked = lowering.constructors.checked(declared)?;
            let taken =
                departures_taken(&lowering.declared.laid(declared).clause_names(), departures)
                    .expect("`Coherent` held every clause of what is attempted to one departure");
            let departing = builder.create_block();
            builder.append_block_param(departing, types::I64);

            let built = decide(builder, module, checked, &given, abort, departing);
            let variable = builder.declare_var(machine_type(binds)?);
            builder.def_var(variable, built);
            bindings.at(*binding, variable);
            let answered = branch(builder, module, bindings, then);
            bindings.leave(*binding);
            answered?;

            builder.seal_block(departing);
            builder.switch_to_block(departing);
            let clause = builder.block_params(departing)[0];
            let bodies = departures.bodies();
            let arms: Vec<ir::Block> = bodies.iter().map(|_| builder.create_block()).collect();
            let astray = builder.create_block();
            let mut which = Switch::new();
            for (place, &departure) in taken.iter().enumerate() {
                which.set_entry(place as u128, arms[departure]);
            }
            which.emit(builder, clause, astray);

            // A place no clause of the declaration has, which the object that ran the clauses and
            // this one's copy of their names disagreeing about how many there are would reach.
            builder.seal_block(astray);
            builder.switch_to_block(astray);
            builder
                .ins()
                .trap(TrapCode::user(NO_ARM).expect("a trap code of its own"));

            for (body, arm) in bodies.into_iter().zip(arms) {
                builder.seal_block(arm);
                builder.switch_to_block(arm);
                branch(builder, module, bindings, body)?;
            }
        }
        Node::Int { .. }
        | Node::Read { .. }
        | Node::Bool { .. }
        | Node::Str { .. }
        | Node::Binary { .. }
        | Node::Neg { .. }
        | Node::Unit { .. }
        | Node::Construct { .. }
        | Node::Field { .. }
        | Node::Some { .. }
        | Node::None { .. }
        | Node::Tuple { .. }
        | Node::Member { .. }
        | Node::List { .. }
        | Node::Call { .. }
        | Node::Block { .. }
        | Node::Apply { .. }
        | Node::Widen { .. } => return Ok(false),
    }
    Ok(true)
}

/// Into `arm` of a fork on `value` where it tests true: the block its body is lowered in is the one
/// written to next, with what it binds in force, which the caller puts out of force once the body
/// is lowered. Handed back is the block the next arm is tested in, where this one tests false.
///
/// Apart from [`branched`] because which arm a value takes is not a fact about what the arm's body
/// goes on to do.
fn enter_arm(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    value: ir::Value,
    subject: &Ty,
    arm: &Arm,
) -> Lowered<ir::Block> {
    let taken = builder.create_block();
    let next = builder.create_block();
    let asked = tests(builder, lowering, module, value, subject, &arm.selects)?;
    builder.ins().brif(asked, taken, &[], next, &[]);
    builder.seal_block(taken);
    builder.seal_block(next);

    builder.switch_to_block(taken);
    if let Some(number) = arm.binding {
        let read_as = arm
            .binds
            .as_ref()
            .expect("`Coherent` held every arm that binds to say what it reads the value as");
        let held = binds(
            builder,
            lowering,
            module,
            value,
            subject,
            &arm.selects,
            read_as,
        )?;
        let variable = builder.declare_var(machine_type(read_as)?);
        builder.def_var(variable, held);
        bindings.at(number, variable);
    }
    Ok(next)
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
    lowering: &Lowerings,
    module: &mut ObjectModule,
    value: ir::Value,
    subject: &Ty,
    selects: &[Selects],
) -> Lowered<ir::Value> {
    let mut asked: Option<ir::Value> = None;
    for one in selects {
        let this = match one {
            Selects::Which { atoms } => {
                let tagged = Tagged::of(value, subject);
                is_one_of_cases(builder, lowering, module, tagged, atoms)?
            }
            Selects::Held => builder.ins().icmp_imm_s(IntCC::NotEqual, value, NOTHING),
            Selects::Nothing => builder.ins().icmp_imm_s(IntCC::Equal, value, NOTHING),
        };
        asked = Some(match asked {
            None => this,
            Some(before) => builder.ins().bor(before, this),
        });
    }
    Ok(asked.expect("`Coherent` held every arm to test at least one case"))
}

/// Whether the value is one of these cases.
///
/// What a value says it is and what a case is are both the address of a token, so this is a
/// comparison of two addresses. The one the value carries was written where it was built —
/// possibly in an object built from another document — and the one compared against is named
/// here; they are equal exactly when the linker resolved both to the one token, which is what
/// makes the answer mean the same thing on either side of an object boundary. A declared case's
/// token is its declaration's, and a case no declaration names has the runtime's, which the value
/// was carried with (`carry`).
///
/// Shared by a `match` arm testing what a value is and a composition's routing testing what a
/// stage accepts: a composition's routing is that same test at a different place, not a second
/// kind of test, and the primitive both read is the one Issue #6 settled — a token's address
/// compared as the linker resolves it.
fn is_one_of_cases(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    value: Tagged,
    cases: &[Case],
) -> Lowered<ir::Value> {
    let which = value.which(builder);
    let mut any: Option<ir::Value> = None;
    for case in cases {
        let expected = token_of(builder, lowering.declared, module, case)?;
        let same = builder.ins().icmp(IntCC::Equal, which, expected);
        any = Some(match any {
            None => same,
            Some(before) => builder.ins().bor(before, same),
        });
    }
    Ok(any.expect("`Coherent` held every test to name at least one case"))
}

/// What the arm reads the value as, once it is known to be one of its cases.
///
/// An arm over an optional's present carrier reads what it holds, narrowed to what the arm says it
/// reads the value as. Every other arm reads the value as the case it selected, which is the value
/// itself unless that case is a primitive a union carried ([`restate`]). What the arm reads it as
/// is carried on the arm, because the test it was selected by does not say it.
fn binds(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    value: ir::Value,
    subject: &Ty,
    selects: &[Selects],
    read_as: &Ty,
) -> Lowered<ir::Value> {
    if selects.iter().any(|it| matches!(it, Selects::Held)) {
        let held = builder.ins().load(types::I64, TRUSTED, value, HELD as i32);
        Ok(out_of_slot(builder, held, machine_type(read_as)?))
    } else {
        restate(builder, lowering, module, value, subject, read_as)
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
) -> Lowered<ir::Value>
where
    A: FnMut(&mut FunctionBuilder, bool) -> Lowered<ir::Value>,
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
/// The two operands of a binary operator, what it reads them as, and which reason (if any) this
/// exact site may end without a value for. Bundled rather than threaded beside each other, since a
/// caller already has all of them off one `Node::Binary`.
struct Operands<'a> {
    reading: &'a Reading,
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
) -> Lowered<ir::Value> {
    // What the operator reads its operands as decides what it does with them, so it is asked
    // before the operator is: an operator with a case of its own would otherwise be lowered as
    // the operands stand whatever the document says they are read as. Only operands read as they
    // stand are lowered, from their one type; a pair read in a type for this operator only, or at
    // their exact values, would first have to be taken as that, and nothing here does so yet.
    match operands.reading {
        Reading::AsTheyStand => {
            binary_as_they_stand(builder, lowering, module, bindings, abort, op, operands)
        }
        Reading::In { ty } => Err(not_lowered(format!(
            "{} over {} and {}, read as {}",
            op.spelt(),
            operands.left.ty().spelt(),
            operands.right.ty().spelt(),
            ty.spelt()
        ))),
        Reading::ExactNumbers => Err(not_lowered(format!(
            "{} over {} and {}, read at their exact values",
            op.spelt(),
            operands.left.ty().spelt(),
            operands.right.ty().spelt()
        ))),
    }
}

/// A binary operator over operands read as they stand.
fn binary_as_they_stand(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    abort: ir::Block,
    op: Op,
    operands: Operands,
) -> Lowered<ir::Value> {
    let Operands {
        left,
        right,
        aborts,
        ..
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
) -> Lowered<ir::Value> {
    let (a, b) = (left.value, right.value);
    match (left.ty, right.ty) {
        // Every primitive is named, for the reason `machine_type` names them: one added to the
        // language would otherwise arrive here and be compared as whatever it is held as.
        (Ty::Prim { prim }, Ty::Prim { prim: also }) if prim == also => match prim {
            Prim::Int => Ok(builder.ins().icmp(as_a_whole_number(op), a, b)),
            // Two truths are equal or they are not, and nothing orders them. `<` over a `Bool` is
            // one the checker never writes, and without its decision on the node it is refused the
            // way every other pair this has no lowering for is (`unlowered_operator`).
            Prim::Bool => match op {
                Op::Eq => Ok(builder.ins().icmp(IntCC::Equal, a, b)),
                Op::Ne => Ok(builder.ins().icmp(IntCC::NotEqual, a, b)),
                _ => Err(unlowered_operator(op, &left, &right)),
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
        // Two values of one type that is not a primitive: equal where what they are made of is,
        // which `equality` answers per type.
        (one, other) if one == other && matches!(op, Op::Eq | Op::Ne) => {
            let same = equality::equal(builder, lowering, module, one, a, b)?;
            Ok(match op {
                Op::Eq => same,
                _ => builder.ins().icmp_imm_s(IntCC::Equal, same, 0),
            })
        }
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
        // is a comparison still to be written, and `<` is one the checker never writes, refused
        // as a `Bool`'s is.
        (Ty::Option { .. }, Ty::Option { .. }) | (Ty::Tuple { .. }, Ty::Tuple { .. }) => match op {
            Op::Eq | Op::Ne => Err(not_lowered(format!(
                "a comparison of {} against {}, which is what they hold compared rather than \
                 where they are",
                left.ty.spelt(),
                right.ty.spelt()
            ))),
            _ => Err(unlowered_operator(op, &left, &right)),
        },
        _ => Err(unlowered_operator(op, &left, &right)),
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
) -> Lowered<ir::Value> {
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
                        overflow_status(aborts),
                        past,
                        also,
                    );
                    Ok(sum)
                }
                Op::Sub => Ok(difference(builder, abort, overflow_status(aborts), a, b)),
                Op::Mul => Ok(product(builder, abort, overflow_status(aborts), a, b)),
                _ => unreachable!("reached from a sum, a difference or a product and nothing else"),
            },
            Prim::Decimal
            | Prim::Rational
            | Prim::Bool
            | Prim::String
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => Err(unlowered_operator(op, &left, &right)),
        },
        _ => Err(unlowered_operator(op, &left, &right)),
    }
}

/// An operator over a pair, read as it stands, that this backend has no lowering for. A pair the
/// checker never writes as it stands was refused by [`Coherent`] as the two halves disagreeing.
fn unlowered_operator(op: Op, left: &Held, right: &Held) -> NotLowered {
    not_lowered(format!(
        "{} over {} and {}, which this backend has no lowering for",
        op.spelt(),
        left.ty.spelt(),
        right.ty.spelt()
    ))
}

/// Two values joined, which the language writes over two strings and over two lists.
///
/// Two strings are joined by the runtime, and two lists of one type here ([`joined_lists`]).
/// Anything else is refused as not lowered.
fn join(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    left: Held,
    right: Held,
) -> Lowered<ir::Value> {
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
            | Prim::Raw => Err(unlowered_operator(Op::Concat, &left, &right)),
        },
        (Ty::List { .. }, Ty::List { .. }) if left.ty == right.ty => {
            Ok(joined_lists(builder, lowering, module, a, b))
        }
        _ => Err(unlowered_operator(Op::Concat, &left, &right)),
    }
}

/// A list holding the elements of `a` and then those of `b`, as `souther-native-abi` lays a list
/// out: new room for the two lengths together, and each list's slots copied into it in order.
///
/// Neither list is changed, and nothing of either is shared with what is made: a list is a value,
/// and so is each of the two. What an element is does not come into it, since every element is one
/// slot whatever it holds, so the slots are copied as they are.
fn joined_lists(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    a: ir::Value,
    b: ir::Value,
) -> ir::Value {
    let first = builder
        .ins()
        .load(types::I64, TRUSTED, a, LIST_LENGTH as i32);
    let second = builder
        .ins()
        .load(types::I64, TRUSTED, b, LIST_LENGTH as i32);
    let length = builder.ins().iadd(first, second);
    let slots = builder.ins().imul_imm_s(length, SLOT);
    let bytes = builder.ins().iadd_imm_s(slots, room_for_list(0));
    let joined = lowering.room_of(builder, module, bytes);
    builder
        .ins()
        .store(TRUSTED, length, joined, LIST_LENGTH as i32);

    let into = builder.ins().iadd_imm_s(joined, list_at(0));
    let from = builder.ins().iadd_imm_s(a, list_at(0));
    copy_slots(builder, from, into, first);
    let past = builder.ins().imul_imm_s(first, SLOT);
    let into = builder.ins().iadd(into, past);
    let from = builder.ins().iadd_imm_s(b, list_at(0));
    copy_slots(builder, from, into, second);
    joined
}

/// `count` slots from `from` onwards copied to `to` onwards, one at a time and in order: none where
/// `count` is nought.
fn copy_slots(builder: &mut FunctionBuilder, from: ir::Value, to: ir::Value, count: ir::Value) {
    let head = builder.create_block();
    builder.append_block_param(head, types::I64);
    let copying = builder.create_block();
    let done = builder.create_block();

    let nought = builder.ins().iconst(types::I64, 0);
    builder.ins().jump(head, &[nought.into()]);

    builder.switch_to_block(head);
    let at = builder.block_params(head)[0];
    let more = builder.ins().icmp(IntCC::SignedLessThan, at, count);
    builder.ins().brif(more, copying, &[], done, &[]);
    builder.seal_block(copying);
    builder.seal_block(done);

    builder.switch_to_block(copying);
    let along = builder.ins().imul_imm_s(at, SLOT);
    let source = builder.ins().iadd(from, along);
    let slot = builder.ins().load(types::I64, TRUSTED, source, 0);
    let target = builder.ins().iadd(to, along);
    builder.ins().store(TRUSTED, slot, target, 0);
    let next = builder.ins().iadd_imm_s(at, 1);
    builder.ins().jump(head, &[next.into()]);
    builder.seal_block(head);

    builder.switch_to_block(done);
}

/// A walk that builds a list (`$build(step, xs, from)`): the list the walk grows starts empty,
/// the step is run where the walk stands on each element of `xs` from `from` onwards, adding to it
/// ([`Growing`]), and what was grown is handed over once, as a list.
///
/// The step is the loop's body, not a function value ([`growing::Step`]): what is bound around it is
/// worked out once, before the walk, and its two parameters are bound here as a `let` binds, the
/// accumulator to what the list is grown in and the element to one slot of `xs` at a time. What it
/// answers is what the next element is handed as the accumulator. The walk goes on while the index
/// is below the length read without a sign, which is where `foldFrom`, the fold it was rewritten
/// from, finds an element: a negative `from` finds none.
///
/// A step never applied ([`unrun`]) is not lowered, nor anything bound around it: it takes a value
/// of what has no value, so `xs` is an empty list literal and the walk answers an empty list. `xs`
/// and `from` are still worked out.
fn build_list(
    builder: &mut FunctionBuilder,
    lowering: &Lowering,
    module: &mut ObjectModule,
    bindings: &mut Bindings,
    abort: ir::Block,
    walk: &Node,
) -> Lowered<ir::Value> {
    let Node::Call { arguments, .. } = walk else {
        unreachable!("a walk is a call");
    };
    let [_, walked, from] = arguments.as_slice() else {
        unreachable!("`Coherent` held a walk to the three arguments it takes");
    };
    // A step never applied is not lowered, the values bound around it included: the list walked
    // is an empty list literal, so the walk answers an empty list.
    if !unrun::never_applied(walk).is_empty() {
        lower(builder, lowering, module, bindings, abort, walked)?;
        lower(builder, lowering, module, bindings, abort, from)?;
        let empty = lowering.room(builder, module, room_for_list(0));
        let nought = builder.ins().iconst(types::I64, 0);
        builder
            .ins()
            .store(TRUSTED, nought, empty, LIST_LENGTH as i32);
        return Ok(empty);
    }
    let Some(step) = growing::Step::of_walk(walk) else {
        unreachable!("`growing` held every walk building a list to walk with a step");
    };
    // Every binding entered here is left again whichever way this ends, as a `let`'s is.
    let mut entered = Vec::new();
    let answer = (|| {
        for around in &step.around {
            let held = lower(builder, lowering, module, bindings, abort, around.value)?;
            let variable = builder.declare_var(machine_type(around.binds)?);
            builder.def_var(variable, held);
            bindings.at(around.binding, variable);
            entered.push(around.binding);
        }
        let list = lower(builder, lowering, module, bindings, abort, walked)?;
        let from = lower(builder, lowering, module, bindings, abort, from)?;

        let [grown, element] = step.parameters else {
            unreachable!("`growing` answers a step only where it takes two parameters");
        };
        let accumulator = builder.declare_var(POINTER);
        let started = Growing::start(builder, lowering, module);
        builder.def_var(accumulator, started.0);
        bindings.at(grown.binding, accumulator);
        bindings.grows(grown.binding);
        entered.push(grown.binding);
        let taken = machine_type(&step.signature.takes[1])?;
        let each = builder.declare_var(taken);
        bindings.at(element.binding, each);
        entered.push(element.binding);
        let at = builder.declare_var(types::I64);
        builder.def_var(at, from);

        let head = builder.create_block();
        let stepping = builder.create_block();
        let done = builder.create_block();
        builder.ins().jump(head, &[]);

        builder.switch_to_block(head);
        let index = builder.use_var(at);
        let length = builder
            .ins()
            .load(types::I64, TRUSTED, list, LIST_LENGTH as i32);
        let inside = builder.ins().icmp(IntCC::UnsignedLessThan, index, length);
        builder.ins().brif(inside, stepping, &[], done, &[]);
        builder.seal_block(stepping);
        builder.seal_block(done);

        builder.switch_to_block(stepping);
        let along = builder.ins().imul_imm_s(index, SLOT);
        let slot = builder.ins().iadd(list, along);
        let held = builder
            .ins()
            .load(types::I64, TRUSTED, slot, list_at(0) as i32);
        let held = out_of_slot(builder, held, taken);
        builder.def_var(each, held);
        let answered = lower(builder, lowering, module, bindings, abort, step.body)?;
        builder.def_var(accumulator, answered);
        let next = builder.ins().iadd_imm_s(index, 1);
        builder.def_var(at, next);
        builder.ins().jump(head, &[]);
        builder.seal_block(head);

        builder.switch_to_block(done);
        Ok(Growing(builder.use_var(accumulator)).sealed(builder))
    })();
    for binding in entered.into_iter().rev() {
        bindings.leave(binding);
    }
    answer
}

/// What a walk building a list grows it in, while it grows it: the address of two slots, the list
/// so far and how many elements that list has room for.
///
/// The list so far is laid out as any list is, its length the elements added so far, in room for
/// more. Where an addition would go past the room, the list is copied into room for twice as many,
/// or for as many as the addition needs where that is more, so each element is copied a bounded
/// number of times however long the walk; and handing the list over is reading it out, since it is
/// already a list. What is past its length is room nothing reads.
///
/// Compiler-private and not in the `abi` crate, as a closure's layout is: it is never handed to a
/// host or to another object, never reaches a call, and is never read as a list until it is handed
/// over, which `growing` holds of every step before any of this is lowered.
struct Growing(ir::Value);

/// Where the list so far is.
const GROWN: i32 = 0;

/// Where how many elements it has room for is.
const GROWN_ROOM: i32 = SLOT as i32;

/// How many elements the first list has room for: a few, so a short walk makes it once.
const FIRST_ROOM: i64 = 8;

impl Growing {
    /// An empty list, with room for [`FIRST_ROOM`] elements.
    fn start(
        builder: &mut FunctionBuilder,
        lowering: &Lowering,
        module: &mut ObjectModule,
    ) -> Growing {
        let growing = lowering.room(builder, module, 2 * SLOT);
        let list = lowering.room(builder, module, room_for_list(FIRST_ROOM));
        let nought = builder.ins().iconst(types::I64, 0);
        builder
            .ins()
            .store(TRUSTED, nought, list, LIST_LENGTH as i32);
        let room = builder.ins().iconst(types::I64, FIRST_ROOM);
        builder.ins().store(TRUSTED, list, growing, GROWN);
        builder.ins().store(TRUSTED, room, growing, GROWN_ROOM);
        Growing(growing)
    }

    /// `value` added after what is there.
    fn add(
        &self,
        builder: &mut FunctionBuilder,
        lowering: &Lowering,
        module: &mut ObjectModule,
        value: ir::Value,
    ) {
        let adding = builder.ins().iconst(types::I64, 1);
        let (list, length) = self.room_for(builder, lowering, module, adding);
        let value = into_slot(builder, value);
        let along = builder.ins().imul_imm_s(length, SLOT);
        let slot = builder.ins().iadd(list, along);
        builder.ins().store(TRUSTED, value, slot, list_at(0) as i32);
        let grown = builder.ins().iadd_imm_s(length, 1);
        builder
            .ins()
            .store(TRUSTED, grown, list, LIST_LENGTH as i32);
    }

    /// Every element of the list `added` added after what is there, in its order.
    fn add_all(
        &self,
        builder: &mut FunctionBuilder,
        lowering: &Lowering,
        module: &mut ObjectModule,
        added: ir::Value,
    ) {
        let adding = builder
            .ins()
            .load(types::I64, TRUSTED, added, LIST_LENGTH as i32);
        let (list, length) = self.room_for(builder, lowering, module, adding);
        let along = builder.ins().imul_imm_s(length, SLOT);
        let into = builder.ins().iadd(list, along);
        let into = builder.ins().iadd_imm_s(into, list_at(0));
        let from = builder.ins().iadd_imm_s(added, list_at(0));
        copy_slots(builder, from, into, adding);
        let grown = builder.ins().iadd(length, adding);
        builder
            .ins()
            .store(TRUSTED, grown, list, LIST_LENGTH as i32);
    }

    /// The list so far, with room for `adding` more elements after its length, and that length.
    fn room_for(
        &self,
        builder: &mut FunctionBuilder,
        lowering: &Lowering,
        module: &mut ObjectModule,
        adding: ir::Value,
    ) -> (ir::Value, ir::Value) {
        let list = builder.ins().load(POINTER, TRUSTED, self.0, GROWN);
        let room = builder.ins().load(types::I64, TRUSTED, self.0, GROWN_ROOM);
        let length = builder
            .ins()
            .load(types::I64, TRUSTED, list, LIST_LENGTH as i32);
        let wanted = builder.ins().iadd(length, adding);

        let moving = builder.create_block();
        let ready = builder.create_block();
        builder.append_block_param(ready, POINTER);
        let fits = builder
            .ins()
            .icmp(IntCC::UnsignedLessThanOrEqual, wanted, room);
        builder.ins().brif(fits, ready, &[list.into()], moving, &[]);
        builder.seal_block(moving);

        builder.switch_to_block(moving);
        let doubled = builder.ins().iadd(room, room);
        let enough = builder.ins().icmp(IntCC::UnsignedLessThan, doubled, wanted);
        let room = builder.ins().select(enough, wanted, doubled);
        let slots = builder.ins().imul_imm_s(room, SLOT);
        let bytes = builder.ins().iadd_imm_s(slots, room_for_list(0));
        let moved = lowering.room_of(builder, module, bytes);
        let into = builder.ins().iadd_imm_s(moved, list_at(0));
        let from = builder.ins().iadd_imm_s(list, list_at(0));
        copy_slots(builder, from, into, length);
        builder
            .ins()
            .store(TRUSTED, length, moved, LIST_LENGTH as i32);
        builder.ins().store(TRUSTED, moved, self.0, GROWN);
        builder.ins().store(TRUSTED, room, self.0, GROWN_ROOM);
        builder.ins().jump(ready, &[moved.into()]);
        builder.seal_block(ready);

        builder.switch_to_block(ready);
        (builder.block_params(ready)[0], length)
    }

    /// The list grown, handed over: it is one already.
    fn sealed(self, builder: &mut FunctionBuilder) -> ir::Value {
        builder.ins().load(POINTER, TRUSTED, self.0, GROWN)
    }
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
) -> ir::Value {
    let difference = builder.ins().isub(a, b);
    let apart = builder.ins().bxor(a, b);
    let moved = builder.ins().bxor(a, difference);
    abort_where_the_sign_bit_is_set(builder, abort, status, apart, moved);
    difference
}

/// Which of the two a truncating division answers.
#[derive(Clone, Copy)]
enum Division {
    /// The quotient, truncated toward zero.
    Quotient,
    /// What is left once the quotient is truncated toward zero, which takes the dividend's sign.
    Remainder,
}

/// A truncating division as a kernel call hands it over: which answer, the two operands already
/// lowered, and what the call names as the reason it can end, which is nothing for a remainder.
struct Dividing<'a> {
    answering: Division,
    dividend: ir::Value,
    divisor: ir::Value,
    aborts: &'a [AbortKind],
}

/// A whole-number division truncated toward zero, answering `Int | DivisionByZero`.
///
/// A zero divisor is a case of the answer and not a reason to end, so it is answered as
/// `DivisionByZero` and nothing is divided. The one pair whose quotient no `Int` holds, the
/// smallest `Int` over -1, ends a quotient with the reason the call names ([`overflow_status`]).
/// Its remainder is nought, which Cranelift's `srem` answers for it. Every other pair is divided,
/// and the answer carried as the union's `Int` case.
///
/// The zero divisor and the quotient past the range are asked before the machine divides, because
/// `sdiv` traps on both and `srem` on the first.
fn truncating_division(
    builder: &mut FunctionBuilder,
    lowering: &Lowerings,
    module: &mut ObjectModule,
    abort: ir::Block,
    division: Dividing,
) -> Lowered<ir::Value> {
    let Dividing {
        answering,
        dividend,
        divisor,
        aborts,
    } = division;
    let by_nought = builder.ins().icmp_imm_s(IntCC::Equal, divisor, 0);
    fork(builder, by_nought, POINTER, |builder, taken| {
        if taken {
            let nothing_divided = Case::Language {
                case: LanguageCase::DivisionByZero,
            };
            return carry(builder, lowering, module, &nothing_divided, None);
        }
        let answered = match answering {
            Division::Quotient => {
                let smallest = builder.ins().icmp_imm_s(IntCC::Equal, dividend, i64::MIN);
                let minus_one = builder.ins().icmp_imm_s(IntCC::Equal, divisor, -1);
                let past = builder.ins().band(smallest, minus_one);
                abort_where(builder, abort, overflow_status(aborts), past);
                builder.ins().sdiv(dividend, divisor)
            }
            Division::Remainder => builder.ins().srem(dividend, divisor),
        };
        let whole = Case::Primitive { prim: Prim::Int };
        carry(builder, lowering, module, &whole, Some(answered))
    })
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
) -> ir::Value {
    let a_wide = builder.ins().sextend(types::I128, a);
    let b_wide = builder.ins().sextend(types::I128, b);
    let wide = builder.ins().imul(a_wide, b_wide);
    let held = builder.ins().ireduce(types::I64, wide);
    let back = builder.ins().sextend(types::I128, held);
    let past = builder.ins().icmp(IntCC::NotEqual, wide, back);
    abort_where(builder, abort, status, past);
    held
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
