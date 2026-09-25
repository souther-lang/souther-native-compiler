//! What a host can call, said once: every function a host reaches in the object and in the
//! runtime, with what each takes and answers, and the part of the model each one reaches.
//!
//! A function is put here where it is emitted, by the code that emits it, and from nothing else.
//! What the object defines for a host and what this says it defines are then one decision, made in
//! one place: a condition added to what is emitted is a condition on what is described, and nothing
//! works the surface out a second time from the program. What the model is — a module's types, their
//! fields and cases, what a behavior takes — is read off the document, since that is a description
//! and not a decision about what is emitted.
//!
//! Everything a host is handed is a projection of the [`Manifest`] this makes: the header's
//! declarations ([`declarations`]), the manifest itself, and what a shared library exports
//! ([`exported`]). None of them is written from anything else, so none of them can say a function
//! the others do not.
//!
//! The manifest is not the document the Java half wrote. That one is a protocol between two halves
//! of one compiler, versioned by [`TRANSPORT_VERSION`](crate::transport::TRANSPORT_VERSION), and the
//! manifest is what a binding for a host language is written against. A reference to a declared
//! type is the key the document reaches it by and never leaves it: the manifest says the module
//! and the name apart, read off the declaration.

use crate::manifest::{self, Carried, Manifest, Parameter, Word};
use crate::transport::{self, AbortKind, Declaration, Prim, Ty};
use crate::{Declared, POINTER, index, native_status};
use anyhow::{Result, bail};
use cranelift::codegen::ir::{self, AbiParam, types};
use cranelift::codegen::isa::CallConv;
use cranelift::module::{DataDescription, Linkage, Module};
use cranelift::object::ObjectModule;
use object::{Object, ObjectSection};
use souther_native_abi::{
    ABI_GENERATION, ANSWERED, DECODED_ISSUES, DECODED_MALFORMED, DECODED_VALUE, HOST_RUNTIME,
    HOST_STATUSES, HostParameter, HostWord,
};
use std::collections::BTreeMap;
use target_lexicon::BinaryFormat;

/// A function a host calls, as it is emitted: its symbol, which is its name in C, and what it
/// takes and answers.
#[derive(Clone, Debug)]
pub(crate) struct HostFunction {
    pub symbol: String,
    pub takes: Vec<HostParameter>,
    pub answers: Option<HostWord>,
}

impl HostFunction {
    /// The signature the function is emitted under, read off what a host is told it takes and
    /// answers, so the two cannot differ.
    pub(crate) fn signature(&self, call_conv: CallConv) -> ir::Signature {
        signature_of(&self.takes, self.answers, call_conv)
    }

    /// What a host is told of it.
    fn described(&self) -> manifest::Function {
        manifest::Function {
            name: self.symbol.clone(),
            takes: self.takes.iter().copied().map(Parameter::from).collect(),
            answers: self.answers.map(Word::from),
        }
    }
}

/// The type of a function a host writes to implement a behavior with no body, as it is called: what
/// C calls a pointer to one, and what it takes and answers. Nothing is defined under the name.
#[derive(Clone, Debug)]
pub(crate) struct HostImplementation {
    pub type_name: String,
    pub takes: Vec<HostParameter>,
    pub answers: HostWord,
}

impl HostImplementation {
    /// The signature it is called under, read off what a host is told it takes and answers.
    pub(crate) fn signature(&self, call_conv: CallConv) -> ir::Signature {
        signature_of(&self.takes, Some(self.answers), call_conv)
    }

    /// What a host is told of it.
    fn described(&self) -> manifest::Implementation {
        manifest::Implementation {
            type_name: self.type_name.clone(),
            takes: self.takes.iter().copied().map(Parameter::from).collect(),
            answers: self.answers.into(),
        }
    }
}

/// A signature of what a host hands over and is handed, whichever side of the call a host is on.
fn signature_of(
    takes: &[HostParameter],
    answers: Option<HostWord>,
    call_conv: CallConv,
) -> ir::Signature {
    let mut signature = ir::Signature::new(call_conv);
    for taken in takes {
        signature.params.push(AbiParam::new(match taken {
            HostParameter::Given(word) => machine(*word),
            HostParameter::Room(_) | HostParameter::Slice(_) => POINTER,
        }));
    }
    if let Some(word) = answers {
        signature.returns.push(AbiParam::new(machine(word)));
    }
    signature
}

/// What a word is on the machine.
pub(crate) fn machine(word: HostWord) -> types::Type {
    match word {
        HostWord::Bool => types::I8,
        HostWord::Status | HostWord::Case | HostWord::Outcome => types::I32,
        HostWord::Int | HostWord::Count | HostWord::Mark => types::I64,
        HostWord::Bytes
        | HostWord::Value
        | HostWord::String
        | HostWord::Decoded
        | HostWord::Issue
        | HostWord::List
        | HostWord::Requirements
        | HostWord::Capability
        | HostWord::Userdata => POINTER,
    }
}

/// What a word is called in the header. An address a host never reads behind is a pointer to a
/// struct nothing defines, so that a C compiler refuses one where another was meant.
fn c_word(word: Word) -> &'static str {
    match word {
        Word::Status => "souther_status",
        Word::Int => "int64_t",
        Word::Bool => "uint8_t",
        Word::Case => "uint32_t",
        Word::Outcome => "int32_t",
        Word::Count => "int64_t",
        Word::Mark => "int64_t",
        Word::Bytes => "const uint8_t *",
        Word::Value => "souther_value",
        Word::String => "souther_string",
        Word::Decoded => "souther_decoded",
        Word::Issue => "souther_issue",
        Word::List => "souther_list",
        Word::Requirements => "const souther_capability *const *",
        Word::Capability => "souther_capability",
        Word::Userdata => "void *",
    }
}

/// A pointer to what C calls a word, spelt the way the header spells one.
fn pointer_to(word: &str) -> String {
    if word.ends_with('*') {
        format!("{word}*")
    } else {
        format!("{word} *")
    }
}

/// A pointer to as many of what C calls a word as are read through it, none of them written.
fn slice_of(word: &str) -> String {
    if word.ends_with('*') {
        format!("{word} const *")
    } else {
        format!("const {word} *")
    }
}

/// A function as the header declares it.
fn declared(function: &manifest::Function) -> String {
    let answers = function.answers.map_or("void", c_word);
    format!(
        "{answers}{}{}({});",
        if answers.ends_with('*') { "" } else { " " },
        function.name,
        parameters(&function.takes)
    )
}

/// What a function takes, as C writes it between the parentheses.
fn parameters(takes: &[Parameter]) -> String {
    let taken: Vec<String> = takes
        .iter()
        .map(|taken| match taken {
            Parameter::Given(word) => c_word(*word).to_string(),
            Parameter::Room(word) => pointer_to(c_word(*word)),
            Parameter::Slice(word) => slice_of(c_word(*word)),
        })
        .collect();
    if taken.is_empty() {
        "void".to_string()
    } else {
        taken.join(", ")
    }
}

/// What a host implements a behavior as, and makes a capability of one through, as the header
/// declares them: the pointer's type, named, and the function taking one, with what a host owes
/// what it hands over.
fn declared_injection(injection: &manifest::Injection) -> String {
    let implementation = &injection.implementation;
    let answers = c_word(implementation.answers);
    let pointer = &implementation.type_name;
    format!(
        "typedef {answers} (*{pointer})({});\n\
         /* The rooms, the function and what it is handed stay as they are while the capability may be called. */\n\
         void {}(souther_capability *, souther_hosted *, {pointer}, void *);",
        parameters(&implementation.takes),
        injection.implement
    )
}

/// What one object makes reachable to a host, module by module.
#[derive(Default)]
pub(crate) struct Surface {
    modules: BTreeMap<String, manifest::Module>,
}

/// A published declaration, and whichever of the functions a host reaches one through the object
/// defines for it, gathered while they are emitted.
pub(crate) struct DeclarationSurface {
    name: String,
    shape: Shape,
    fields: Vec<manifest::Field>,
    construct: Option<manifest::Function>,
    case: Option<manifest::Function>,
    decode: Option<manifest::Function>,
    decode_host: Option<manifest::Function>,
    encode: Option<manifest::Function>,
}

/// Which of the declaration's kinds it is, with what only that kind has.
enum Shape {
    Product,
    Newtype,
    Unit,
    Sum(Vec<manifest::Case>),
}

impl DeclarationSurface {
    /// A declaration as the model says it, with none of its functions yet.
    pub(crate) fn of(declaration: &Declaration, declared: &Declared) -> DeclarationSurface {
        let shape = match declaration {
            Declaration::Product { .. } => Shape::Product,
            Declaration::Newtype { .. } => Shape::Newtype,
            Declaration::Unit { .. } => Shape::Unit,
            Declaration::Sum { cases, .. } => {
                Shape::Sum(cases.iter().map(|case| case_of(case, declared)).collect())
            }
        };
        DeclarationSurface {
            name: declaration.name().to_string(),
            shape,
            fields: declaration
                .fields()
                .iter()
                .map(|field| manifest::Field {
                    name: field.name.clone(),
                    ty: type_of(&field.codec.ty(), declared),
                    read: None,
                })
                .collect(),
            construct: None,
            case: None,
            decode: None,
            decode_host: None,
            encode: None,
        }
    }

    pub(crate) fn constructed_by(&mut self, function: &HostFunction) {
        self.construct = Some(function.described());
    }

    pub(crate) fn cased_by(&mut self, function: &HostFunction) {
        self.case = Some(function.described());
    }

    pub(crate) fn decoded_by(&mut self, function: &HostFunction) {
        self.decode = Some(function.described());
    }

    pub(crate) fn host_value_decoded_by(&mut self, function: &HostFunction) {
        self.decode_host = Some(function.described());
    }

    pub(crate) fn encoded_by(&mut self, function: &HostFunction) {
        self.encode = Some(function.described());
    }

    /// The field at `at` is read by `function`.
    pub(crate) fn field_read_by(&mut self, at: usize, function: &HostFunction) {
        self.fields[at].read = Some(function.described());
    }

    /// The declaration as the manifest says it. A function a kind has no place for is this
    /// compiler having emitted one it should not have.
    fn described(self) -> manifest::Declaration {
        let DeclarationSurface {
            name,
            shape,
            fields,
            construct,
            case,
            decode,
            decode_host,
            encode,
        } = self;
        let no_case = |kind: &str| {
            assert!(case.is_none(), "a {kind} is not a sum and has no case");
        };
        match shape {
            Shape::Product => {
                no_case("product");
                manifest::Declaration::Product {
                    name,
                    fields,
                    construct,
                    decode,
                    decode_host,
                    encode,
                }
            }
            Shape::Newtype => {
                no_case("newtype");
                let [field] =
                    <[manifest::Field; 1]>::try_from(fields).expect("a newtype has one field");
                manifest::Declaration::Newtype {
                    name,
                    field,
                    construct,
                    decode,
                    decode_host,
                    encode,
                }
            }
            Shape::Unit => {
                no_case("unit");
                assert!(fields.is_empty(), "a unit has no field");
                manifest::Declaration::Unit {
                    name,
                    construct,
                    decode,
                    decode_host,
                    encode,
                }
            }
            Shape::Sum(cases) => {
                assert!(
                    fields.is_empty() && construct.is_none(),
                    "a sum is never built"
                );
                manifest::Declaration::Sum {
                    name,
                    cases,
                    case,
                    decode,
                    decode_host,
                    encode,
                }
            }
        }
    }
}

/// Each of `takes` under the name `names` gives it at the same place. The two were read as one list
/// ([`crate::transport::Target::names`]), and are paired back here.
fn named(names: &[String], takes: &[Ty], declared: &Declared) -> Vec<manifest::NamedParameter> {
    assert_eq!(
        names.len(),
        takes.len(),
        "a name was read beside every input it names"
    );
    names
        .iter()
        .zip(takes)
        .map(|(name, ty)| manifest::NamedParameter {
            name: name.clone(),
            ty: type_of(ty, declared),
        })
        .collect()
}

impl Surface {
    /// A published declaration of `module`, and what a host reaches it through.
    pub(crate) fn declaration(&mut self, module: &str, declaration: DeclarationSurface) {
        self.module(module)
            .declarations
            .push(declaration.described());
    }

    /// A published behavior this object defines, and what a host calls it through, where a host
    /// can hand it what it takes and take what it answers.
    ///
    /// `names` are what its declaration calls what it takes, and none for a composition. `union` is
    /// where it answers a union no declaration names: the cases that descends to, and what a host
    /// asks which of them an answer is through, where it can.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn behavior(
        &mut self,
        module: &str,
        name: &str,
        names: Option<&[String]>,
        takes: &[Ty],
        answers: &Ty,
        union: Option<(&[transport::Case], Option<&HostFunction>)>,
        declared: &Declared,
        call: Option<&HostFunction>,
    ) {
        let behavior = manifest::Behavior {
            name: name.to_string(),
            parameters: match names {
                Some(names) => manifest::Parameters::Named(named(names, takes, declared)),
                None => manifest::Parameters::Positional(
                    takes.iter().map(|ty| type_of(ty, declared)).collect(),
                ),
            },
            answers: manifest::Answer {
                ty: type_of(answers, declared),
                union: union.map(|(cases, case)| manifest::UnionAnswer {
                    cases: cases.iter().map(|it| case_of(it, declared)).collect(),
                    case: case.map(HostFunction::described),
                }),
            },
            call: call.map(HostFunction::described),
        };
        self.module(module).behaviors.push(behavior);
    }

    /// A behavior this object defines that a host constructs the capabilities of what a call is
    /// made with out of, what it requires, and what a host makes a capability of it through, where
    /// something may require it.
    pub(crate) fn construction(
        &mut self,
        module: &str,
        name: &str,
        requires: &[transport::Requirement],
        bind: Option<&HostFunction>,
    ) {
        let construction = manifest::Construction {
            name: name.to_string(),
            requires: requires
                .iter()
                .map(|it| manifest::Required {
                    module: it.module.clone(),
                    name: it.name.clone(),
                })
                .collect(),
            bind: bind.map(HostFunction::described),
        };
        self.module(module).constructions.push(construction);
    }

    /// A behavior a module of this object declares with no body, which a host implements as
    /// `implementation` says and makes a capability of through `implement`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn injection(
        &mut self,
        module: &str,
        name: &str,
        names: &[String],
        takes: &[Ty],
        answers: &Ty,
        declared: &Declared,
        implementation: &HostImplementation,
        implement: &str,
    ) {
        let injection = manifest::Injection {
            name: name.to_string(),
            parameters: named(names, takes, declared),
            answers: type_of(answers, declared),
            implementation: implementation.described(),
            implement: implement.to_string(),
        };
        self.module(module).injections.push(injection);
    }

    /// A value a module of this object publishes, and what a host reads it through.
    pub(crate) fn value(
        &mut self,
        module: &str,
        name: &str,
        ty: &Ty,
        declared: &Declared,
        read: Option<&HostFunction>,
    ) {
        let value = manifest::PublishedValue {
            name: name.to_string(),
            ty: type_of(ty, declared),
            read: read.map(HostFunction::described),
        };
        self.module(module).values.push(value);
    }

    /// What a host builds and reads a list of `module`'s through, where its elements cross as
    /// `element`.
    pub(crate) fn list(
        &mut self,
        module: &str,
        element: manifest::Element,
        construct: &HostFunction,
        length: &HostFunction,
        at: &HostFunction,
    ) {
        self.module(module).lists.push(manifest::ListCrossing {
            element,
            construct: construct.described(),
            length: length.described(),
            at: at.described(),
        });
    }

    fn module(&mut self, name: &str) -> &mut manifest::Module {
        self.modules
            .entry(name.to_string())
            .or_insert_with(|| manifest::Module {
                name: name.to_string(),
                behaviors: Vec::new(),
                constructions: Vec::new(),
                injections: Vec::new(),
                values: Vec::new(),
                declarations: Vec::new(),
                lists: Vec::new(),
            })
    }

    /// Puts what this object makes reachable to a host into the object itself, in a section of
    /// its own, so that whatever links it reads what it offers off it and from nothing else.
    ///
    /// The object is what a library is made of, and more than one build's object may go into one
    /// library: a build's object defines what reads and builds a value of a type it declares, and
    /// another build's object calls that. So what a library offers is what each of its objects
    /// carries, and one object built before another is described by itself rather than by a
    /// second reading of a program that build no longer has.
    pub(crate) fn carry(&self, module: &mut ObjectModule) {
        let carried = Carried {
            version: manifest::VERSION,
            abi: ABI_GENERATION,
            modules: self.modules.values().cloned().collect(),
        };
        let written =
            serde_json::to_vec(&carried).expect("what an object carries is JSON whatever it holds");
        let mut data = DataDescription::new();
        data.define(written.into_boxed_slice());
        data.set_custom_section(match module.isa().triple().binary_format {
            BinaryFormat::Macho => "__DATA,__souther_host",
            _ => CARRIED,
        });
        // Kept by a link that strips what nothing refers to: nothing in the program refers to this.
        data.set_used(true);
        let id = module
            .declare_data("$host$surface", Linkage::Local, false, false)
            .unwrap_or_else(|refused| panic!("Cranelift refused a data object: {refused}"));
        module
            .define_data(id, &data)
            .unwrap_or_else(|refused| panic!("Cranelift refused a data object: {refused}"));
    }
}

/// The section an object carries its surface in, where its format has no segments; in Mach-O the
/// section of the same name, less its dot and with the two underscores Mach-O names take, in the
/// data segment.
const CARRIED: &str = ".souther_host";

/// Every module an object carries, read back out of it. `named` is what the object is called, for
/// a refusal to say which one it was.
pub(crate) fn carried_by(object: &[u8], named: &str) -> Result<Vec<manifest::Module>> {
    let file = object::File::parse(object)
        .map_err(|problem| anyhow::anyhow!("{named} is not an object: {problem}"))?;
    let section = match file.format() {
        object::BinaryFormat::MachO => "__souther_host",
        _ => CARRIED,
    };
    let Some(section) = file.section_by_name(section) else {
        bail!(
            "{named} carries no surface for a host: it is not an object this compiler built, or \
             one it built before an object carried one"
        );
    };
    let data = section
        .data()
        .map_err(|problem| anyhow::anyhow!("{named}: {problem}"))?;
    // What the section holds is the JSON, and whatever padding its alignment asked for after it.
    let end = data
        .iter()
        .rposition(|byte| *byte != 0)
        .map_or(0, |at| at + 1);
    let carried = Carried::read(&data[..end])
        .map_err(|problem| anyhow::anyhow!("{named} carries {problem}"))?;
    Ok(carried.modules)
}

/// The manifest of a library holding these modules.
pub(crate) fn manifest_of(modules: Vec<manifest::Module>) -> Result<Manifest> {
    constructs_whole(&modules)?;
    Ok(Manifest {
        format: manifest::FORMAT.to_string(),
        version: manifest::VERSION,
        abi: ABI_GENERATION,
        statuses: numbered(statuses()),
        outcomes: numbered(outcomes()),
        runtime: HOST_RUNTIME
            .iter()
            .map(|function| {
                HostFunction {
                    symbol: function.name.to_string(),
                    takes: function.takes.to_vec(),
                    answers: function.answers,
                }
                .described()
            })
            .collect(),
        modules,
    })
}

/// That a host can construct everything a behavior it constructs requires: each requirement of each
/// construction is a behavior a host implements or one constructed in turn, of some object the
/// library holds.
///
/// A behavior reached through a capability is not a symbol the program names, so a library linked
/// without the object that constructs one links all the same, and a host would find out when it
/// built the capability. Refused here instead, where the objects are put together.
fn constructs_whole(modules: &[manifest::Module]) -> Result<()> {
    let mut constructible = std::collections::HashSet::new();
    for module in modules {
        for injection in &module.injections {
            constructible.insert((module.name.as_str(), injection.name.as_str()));
        }
        for construction in &module.constructions {
            constructible.insert((module.name.as_str(), construction.name.as_str()));
        }
    }
    for module in modules {
        for construction in &module.constructions {
            for required in &construction.requires {
                if !constructible.contains(&(required.module.as_str(), required.name.as_str())) {
                    bail!(
                        "{}.{} requires {}.{}, which no object linked constructs or asks a host \
                         to implement",
                        module.name,
                        construction.name,
                        required.module,
                        required.name
                    );
                }
            }
        }
    }
    Ok(())
}

/// The manifest as it is written.
pub(crate) fn written(manifest: &Manifest) -> String {
    let mut written =
        serde_json::to_string_pretty(manifest).expect("a manifest is JSON whatever it holds");
    written.push('\n');
    written
}

/// Every function the manifest names, in the order the header declares them.
fn functions(manifest: &Manifest) -> impl Iterator<Item = &manifest::Function> {
    let modules = manifest.modules.iter().flat_map(|module| {
        let behaviors = module.behaviors.iter().flat_map(behavior_functions);
        let constructions = module
            .constructions
            .iter()
            .filter_map(|it| it.bind.as_ref());
        let values = module.values.iter().filter_map(|it| it.read.as_ref());
        let declarations = module.declarations.iter().flat_map(declaration_functions);
        let lists = module.lists.iter().flat_map(list_functions);
        behaviors
            .chain(constructions)
            .chain(values)
            .chain(declarations)
            .chain(lists)
    });
    manifest.runtime.iter().chain(modules)
}

/// Every function a behavior is reached through, in the order the header declares them: its call,
/// and which case its answer is.
fn behavior_functions(behavior: &manifest::Behavior) -> impl Iterator<Item = &manifest::Function> {
    let case = behavior
        .answers
        .union
        .as_ref()
        .and_then(|union| union.case.as_ref());
    behavior.call.iter().chain(case)
}

/// Every function a list is reached through, in the order the header declares them.
fn list_functions(list: &manifest::ListCrossing) -> [&manifest::Function; 3] {
    [&list.construct, &list.length, &list.at]
}

/// Every function a declaration is reached through, in the order the header declares them.
///
/// Every field of every kind is named, with no rest pattern: a function a kind gains is then one
/// this has to be told about, rather than one the header and the exported symbols quietly leave
/// out.
fn declaration_functions(
    declaration: &manifest::Declaration,
) -> impl Iterator<Item = &manifest::Function> {
    let (operations, fields): ([&Option<manifest::Function>; 5], &[manifest::Field]) =
        match declaration {
            manifest::Declaration::Product {
                name: _,
                fields,
                construct,
                decode,
                decode_host,
                encode,
            } => ([construct, &None, decode, decode_host, encode], fields),
            manifest::Declaration::Newtype {
                name: _,
                field,
                construct,
                decode,
                decode_host,
                encode,
            } => (
                [construct, &None, decode, decode_host, encode],
                std::slice::from_ref(field),
            ),
            manifest::Declaration::Unit {
                name: _,
                construct,
                decode,
                decode_host,
                encode,
            } => ([construct, &None, decode, decode_host, encode], &[]),
            manifest::Declaration::Sum {
                name: _,
                cases: _,
                case,
                decode,
                decode_host,
                encode,
            } => ([&None, case, decode, decode_host, encode], &[]),
        };
    operations
        .into_iter()
        .flatten()
        .chain(fields.iter().filter_map(|field| field.read.as_ref()))
}

/// Every symbol a shared library with this manifest exports: what a host calls, and nothing else.
///
/// What a host makes a capability of an implementation through is one of them. What it implements
/// is not: that is the host's own function, named in C and defined by nobody here.
pub(crate) fn exported(manifest: &Manifest) -> Vec<String> {
    let implements = manifest
        .modules
        .iter()
        .flat_map(|module| &module.injections)
        .map(|injection| &injection.implement);
    functions(manifest)
        .map(|function| &function.name)
        .chain(implements)
        .cloned()
        .collect()
}

/// The declarations of every function a host calls, the typedefs they are written in, and the
/// numbers a host compares a status and a reading's outcome with, as C and nothing else: no
/// directive, no macro, nothing a reader of C declarations that has no preprocessor would have to
/// run one for. `int64_t` and the rest are named as `<stdint.h>` names them, which such a reader
/// knows, and a C compiler reads this after the header that includes that.
pub(crate) fn declarations(manifest: &Manifest) -> String {
    let mut written = format!(
        "/* What a host calls in a Souther program built by souther-native-compiler, and in the\n \
         * runtime it is linked with, as declarations and nothing else, for a reader of C\n \
         * declarations that has no preprocessor. A C or C++ compiler includes souther.h.\n \
         * souther.json describes the same functions in the model's terms. ABI generation {}. */\n\
         \n\
         typedef uint32_t souther_status;\n\
         typedef const struct souther_value_ *souther_value;\n\
         typedef const struct souther_string_ *souther_string;\n\
         typedef const struct souther_decoded_ *souther_decoded;\n\
         typedef const struct souther_issue_ *souther_issue;\n\
         typedef const struct souther_list_ *souther_list;\n\
         /* The address of code, which a host never calls or reads: what makes a capability writes it. */\n\
         typedef void (*souther_code)(void);\n\
         /* What is handed where a behavior is required: laid out by a host as room, and written by\n \
         * what makes one. */\n\
         typedef struct souther_capability {{ souther_code invoke; const void *environment; }} souther_capability;\n\
         /* What a capability of a host's own implementation reads it out of: laid out by a host as\n \
         * room, and written by what makes the capability. */\n\
         typedef struct souther_hosted {{ souther_code implementation; void *userdata; }} souther_hosted;\n\
         \n",
        manifest.abi
    );
    let statuses: Vec<String> = in_order(&manifest.statuses)
        .into_iter()
        .map(|(name, number)| format!("    SOUTHER_{name} = {number}"))
        .collect();
    written.push_str(&format!("enum {{\n{}\n}};\n\n", statuses.join(",\n")));
    let outcomes: Vec<String> = in_order(&manifest.outcomes)
        .into_iter()
        .map(|(name, number)| format!("    SOUTHER_DECODED_{name} = {number}"))
        .collect();
    written.push_str(&format!("enum {{\n{}\n}};\n", outcomes.join(",\n")));

    written.push_str("\n/* The runtime. */\n");
    for function in &manifest.runtime {
        written.push_str(&declared(function));
        written.push('\n');
    }
    for module in &manifest.modules {
        let name = &module.name;
        written.push_str(&format!("\n/* {name} */\n"));
        for behavior in &module.behaviors {
            let functions: Vec<&manifest::Function> = behavior_functions(behavior).collect();
            if functions.is_empty() {
                continue;
            }
            written.push_str(&format!("/* behavior {name}.{} */\n", behavior.name));
            for function in functions {
                written.push_str(&declared(function));
                written.push('\n');
            }
        }
        for construction in &module.constructions {
            if let Some(bind) = &construction.bind {
                written.push_str(&format!("/* constructed {name}.{} */\n", construction.name));
                written.push_str(&declared(bind));
                written.push('\n');
            }
        }
        for injection in &module.injections {
            written.push_str(&format!("/* injected {name}.{} */\n", injection.name));
            written.push_str(&declared_injection(injection));
            written.push('\n');
        }
        for value in &module.values {
            if let Some(read) = &value.read {
                written.push_str(&format!("/* value {name}.{} */\n", value.name));
                written.push_str(&declared(read));
                written.push('\n');
            }
        }
        for declaration in &module.declarations {
            let functions: Vec<&manifest::Function> = declaration_functions(declaration).collect();
            if functions.is_empty() {
                continue;
            }
            let (kind, declared_name) = match declaration {
                manifest::Declaration::Product { name, .. } => ("product", name),
                manifest::Declaration::Newtype { name, .. } => ("newtype", name),
                manifest::Declaration::Unit { name, .. } => ("unit", name),
                manifest::Declaration::Sum { name, .. } => ("sum", name),
            };
            written.push_str(&format!("/* {kind} {name}.{declared_name} */\n"));
            for function in functions {
                written.push_str(&declared(function));
                written.push('\n');
            }
        }
        for list in &module.lists {
            let element = match list.element {
                manifest::Element::Whole(word) => c_word(word).to_string(),
                manifest::Element::Present(word) => format!("{} and its presence", c_word(word)),
            };
            written.push_str(&format!("/* a list of {element} */\n"));
            for function in list_functions(list) {
                written.push_str(&declared(function));
                written.push('\n');
            }
        }
    }
    written
}

/// Names and the numbers they stand for, as the manifest holds them: each name once, and each
/// number once. Two names for one number is this compiler's own tables disagreeing — two reasons a
/// computation ended answered alike — and nothing a host reads could tell them apart, so it stops
/// here rather than reaching the manifest as two names and the header as whichever came last.
fn numbered<N: Copy + Ord>(pairs: Vec<(&str, N)>) -> BTreeMap<String, N> {
    let mut by_name = BTreeMap::new();
    let mut by_number = BTreeMap::new();
    for (name, number) in pairs {
        index::unique(&mut by_number, number, name);
        index::unique(&mut by_name, name.to_string(), number);
    }
    by_name
}

/// Names in the order of the numbers they stand for, every one of them.
fn in_order<N: Copy + Ord>(named: &BTreeMap<String, N>) -> Vec<(&str, N)> {
    let mut ordered: Vec<(&str, N)> = named
        .iter()
        .map(|(name, number)| (name.as_str(), *number))
        .collect();
    ordered.sort_by_key(|(_, number)| *number);
    ordered
}

/// Every status a generated function answers, by the name the header gives it.
fn statuses() -> Vec<(&'static str, u32)> {
    let mut statuses = vec![("ANSWERED", ANSWERED)];
    statuses.extend(
        AbortKind::ALL
            .iter()
            .map(|kind| (kind.spelt(), native_status(*kind))),
    );
    statuses.extend(HOST_STATUSES.iter().copied());
    statuses
}

/// Every outcome a reading comes to, by the name the header gives it.
fn outcomes() -> Vec<(&'static str, i32)> {
    vec![
        ("VALUE", DECODED_VALUE),
        ("ISSUES", DECODED_ISSUES),
        ("MALFORMED", DECODED_MALFORMED),
    ]
}

/// A type as the manifest says it: what it is in the model, and never a key of the document's.
fn type_of(ty: &Ty, declared: &Declared) -> manifest::Type {
    let each = |types: &[Ty]| types.iter().map(|it| type_of(it, declared)).collect();
    let boxed = |ty: &Ty| Box::new(type_of(ty, declared));
    match ty {
        Ty::Prim { prim } => manifest::Type::Primitive {
            name: primitive(*prim),
        },
        Ty::Declared { declared: key } => {
            let declaration = declared.laid(key);
            manifest::Type::Declared {
                module: declaration.module().to_string(),
                name: declaration.name().to_string(),
            }
        }
        Ty::Union { union } => manifest::Type::Union {
            cases: union.iter().map(|case| case_of(case, declared)).collect(),
        },
        Ty::Option { option } => manifest::Type::Option { of: boxed(option) },
        Ty::Tuple { tuple } => manifest::Type::Tuple { of: each(tuple) },
        Ty::Fn { fn_ } => manifest::Type::Function {
            takes: each(&fn_.takes),
            answers: boxed(&fn_.answers),
        },
        Ty::List { list } => manifest::Type::List { of: boxed(list) },
        Ty::Set { set } => manifest::Type::Set { of: boxed(set) },
        Ty::Map { map } => manifest::Type::Map {
            key: boxed(&map.key),
            value: boxed(&map.value),
        },
        Ty::Var { var } => crate::laid_out_nowhere(*var),
        Ty::Nothing { .. } => unreachable!(
            "no source writes the type of what has no value, so a boundary is never read as one, \
             and a published value of one is refused before it is described (`define_values`)"
        ),
    }
}

/// A primitive as the manifest names it. Every one named, for the reason `machine_type` names
/// them.
fn primitive(prim: Prim) -> manifest::Primitive {
    match prim {
        Prim::Int => manifest::Primitive::Int,
        Prim::String => manifest::Primitive::String,
        Prim::Bool => manifest::Primitive::Bool,
        Prim::Decimal => manifest::Primitive::Decimal,
        Prim::Rational => manifest::Primitive::Rational,
        Prim::Date => manifest::Primitive::Date,
        Prim::Time => manifest::Primitive::Time,
        Prim::DateTime => manifest::Primitive::DateTime,
        Prim::Instant => manifest::Primitive::Instant,
        Prim::Raw => manifest::Primitive::Raw,
    }
}

/// A case as the manifest says it.
fn case_of(case: &transport::Case, declared: &Declared) -> manifest::Case {
    match case {
        transport::Case::Declared { declared: key } => {
            let declaration = declared.laid(key);
            manifest::Case::Declared {
                module: declaration.module().to_string(),
                name: declaration.name().to_string(),
            }
        }
        transport::Case::Primitive { prim } => manifest::Case::Primitive {
            name: primitive(*prim),
        },
        transport::Case::Language { case } => manifest::Case::Language {
            name: match case {
                transport::LanguageCase::Some => manifest::LanguageCase::Some,
                transport::LanguageCase::None => manifest::LanguageCase::None,
                transport::LanguageCase::DivisionByZero => manifest::LanguageCase::DivisionByZero,
                transport::LanguageCase::NotANumber => manifest::LanguageCase::NotANumber,
                transport::LanguageCase::NotADate => manifest::LanguageCase::NotADate,
                transport::LanguageCase::NotATime => manifest::LanguageCase::NotATime,
                transport::LanguageCase::NotWhole => manifest::LanguageCase::NotWhole,
                transport::LanguageCase::NotAFiniteDecimal => {
                    manifest::LanguageCase::NotAFiniteDecimal
                }
            },
        },
    }
}
