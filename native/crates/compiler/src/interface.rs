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
//! Everything a host is handed is a projection of this: the header ([`Surface::header`]), the
//! manifest ([`Surface::manifest`]), and what a shared library exports ([`Surface::exported`]).
//! None of them is written from anything else, so none of them can say a function the others do
//! not.
//!
//! The manifest is not the document the Java half wrote. That one is a protocol between two halves
//! of one compiler, versioned by [`TRANSPORT_VERSION`](crate::transport::TRANSPORT_VERSION), and the
//! manifest is what a binding for a host language is written against. A reference to a declared
//! type is the key the document reaches it by and never leaves it: the manifest says the module
//! and the name apart, read off the declaration.

use crate::transport::{AbortKind, Case, Declaration, Prim, Ty};
use crate::{Declared, POINTER, native_status};
use cranelift::codegen::ir::{self, AbiParam, types};
use cranelift::codegen::isa::CallConv;
use serde_json::{Value, json};
use souther_native_abi::{
    ABI_GENERATION, ANSWERED, DECODED_ISSUES, DECODED_MALFORMED, DECODED_VALUE, HOST_RUNTIME,
    HostParameter, HostWord,
};
use std::collections::BTreeMap;

/// What the manifest is. Moved when what a manifest says is read differently, and not when the
/// functions it names are called differently: that is [`ABI_GENERATION`], which the manifest
/// carries beside this.
const MANIFEST_VERSION: u32 = 1;

/// A function a host calls: its symbol, which is its name in C, and what it takes and answers.
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
        let mut signature = ir::Signature::new(call_conv);
        for taken in &self.takes {
            signature.params.push(AbiParam::new(match taken {
                HostParameter::Given(word) => machine(*word),
                HostParameter::Room(_) => POINTER,
            }));
        }
        if let Some(word) = self.answers {
            signature.returns.push(AbiParam::new(machine(word)));
        }
        signature
    }

    fn declared(&self) -> String {
        let taken: Vec<String> = self
            .takes
            .iter()
            .map(|taken| match taken {
                HostParameter::Given(word) => c_word(*word).to_string(),
                HostParameter::Room(word) => pointer_to(c_word(*word)),
            })
            .collect();
        let answers = self.answers.map_or("void", c_word);
        format!(
            "{answers}{}{}({});",
            if answers.ends_with('*') { "" } else { " " },
            self.symbol,
            if taken.is_empty() {
                "void".to_string()
            } else {
                taken.join(", ")
            }
        )
    }

    fn described(&self) -> Value {
        json!({
            "name": self.symbol,
            "takes": self.takes.iter().map(|taken| match taken {
                HostParameter::Given(word) => json!({ "given": word_name(*word) }),
                HostParameter::Room(word) => json!({ "room": word_name(*word) }),
            }).collect::<Vec<_>>(),
            "answers": self.answers.map(word_name),
        })
    }
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
        | HostWord::Issue => POINTER,
    }
}

/// What a word is called in the header. An address a host never reads behind is a pointer to a
/// struct nothing defines, so that a C compiler refuses one where another was meant.
fn c_word(word: HostWord) -> &'static str {
    match word {
        HostWord::Status => "souther_status",
        HostWord::Int => "int64_t",
        HostWord::Bool => "uint8_t",
        HostWord::Case => "uint32_t",
        HostWord::Outcome => "int32_t",
        HostWord::Count => "int64_t",
        HostWord::Mark => "int64_t",
        HostWord::Bytes => "const uint8_t *",
        HostWord::Value => "souther_value",
        HostWord::String => "souther_string",
        HostWord::Decoded => "souther_decoded",
        HostWord::Issue => "souther_issue",
    }
}

/// A pointer to what C calls `word`, spelt the way the header spells one.
fn pointer_to(word: &str) -> String {
    if word.ends_with('*') {
        format!("{word}*")
    } else {
        format!("{word} *")
    }
}

/// What a word is called in the manifest.
fn word_name(word: HostWord) -> &'static str {
    match word {
        HostWord::Status => "status",
        HostWord::Int => "int",
        HostWord::Bool => "bool",
        HostWord::Case => "case",
        HostWord::Outcome => "outcome",
        HostWord::Count => "count",
        HostWord::Mark => "mark",
        HostWord::Bytes => "bytes",
        HostWord::Value => "value",
        HostWord::String => "string",
        HostWord::Decoded => "decoded",
        HostWord::Issue => "issue",
    }
}

/// Everything a host can call in one object and the runtime it is linked with.
#[derive(Default)]
pub(crate) struct Surface {
    modules: BTreeMap<String, ModuleSurface>,
}

#[derive(Default)]
struct ModuleSurface {
    behaviors: Vec<BehaviorSurface>,
    values: Vec<ValueSurface>,
    declarations: Vec<DeclarationSurface>,
}

struct BehaviorSurface {
    name: String,
    takes: Vec<Value>,
    answers: Value,
    call: Option<HostFunction>,
}

struct ValueSurface {
    name: String,
    ty: Value,
    read: Option<HostFunction>,
}

/// A published declaration, and whichever of the functions a host reaches one through the object
/// defines for it.
pub(crate) struct DeclarationSurface {
    kind: &'static str,
    name: String,
    fields: Vec<FieldSurface>,
    cases: Option<Vec<Value>>,
    construct: Option<HostFunction>,
    case: Option<HostFunction>,
    decode: Option<HostFunction>,
    encode: Option<HostFunction>,
}

struct FieldSurface {
    name: String,
    ty: Value,
    read: Option<HostFunction>,
}

impl DeclarationSurface {
    /// A declaration as the model says it, with none of its functions yet.
    pub(crate) fn of(declaration: &Declaration, declared: &Declared) -> DeclarationSurface {
        let (kind, cases) = match declaration {
            Declaration::Product { .. } => ("product", None),
            Declaration::Newtype { .. } => ("newtype", None),
            Declaration::Unit { .. } => ("unit", None),
            Declaration::Sum { cases, .. } => (
                "sum",
                Some(cases.iter().map(|case| case_of(case, declared)).collect()),
            ),
        };
        DeclarationSurface {
            kind,
            name: declaration.name().to_string(),
            fields: declaration
                .fields()
                .iter()
                .map(|field| FieldSurface {
                    name: field.name.clone(),
                    ty: type_of(&field.codec.ty(), declared),
                    read: None,
                })
                .collect(),
            cases,
            construct: None,
            case: None,
            decode: None,
            encode: None,
        }
    }

    pub(crate) fn constructed_by(&mut self, function: HostFunction) {
        self.construct = Some(function);
    }

    pub(crate) fn cased_by(&mut self, function: HostFunction) {
        self.case = Some(function);
    }

    pub(crate) fn decoded_by(&mut self, function: HostFunction) {
        self.decode = Some(function);
    }

    pub(crate) fn encoded_by(&mut self, function: HostFunction) {
        self.encode = Some(function);
    }

    /// The field at `at` is read by `function`.
    pub(crate) fn field_read_by(&mut self, at: usize, function: HostFunction) {
        self.fields[at].read = Some(function);
    }

    fn functions(&self) -> impl Iterator<Item = &HostFunction> {
        [&self.construct, &self.case, &self.decode, &self.encode]
            .into_iter()
            .flatten()
            .chain(self.fields.iter().filter_map(|field| field.read.as_ref()))
    }
}

impl Surface {
    /// A published declaration of `module`, and what a host reaches it through.
    pub(crate) fn declaration(&mut self, module: &str, declaration: DeclarationSurface) {
        self.module(module).declarations.push(declaration);
    }

    /// A published behavior this object defines, and what a host calls it through, where a host
    /// can hand it what it takes and take what it answers.
    pub(crate) fn behavior(
        &mut self,
        module: &str,
        name: &str,
        takes: &[Ty],
        answers: &Ty,
        declared: &Declared,
        call: Option<HostFunction>,
    ) {
        let behavior = BehaviorSurface {
            name: name.to_string(),
            takes: takes.iter().map(|ty| type_of(ty, declared)).collect(),
            answers: type_of(answers, declared),
            call,
        };
        self.module(module).behaviors.push(behavior);
    }

    /// A value a module of this object publishes, and what a host reads it through.
    pub(crate) fn value(
        &mut self,
        module: &str,
        name: &str,
        ty: &Ty,
        declared: &Declared,
        read: Option<HostFunction>,
    ) {
        let value = ValueSurface {
            name: name.to_string(),
            ty: type_of(ty, declared),
            read,
        };
        self.module(module).values.push(value);
    }

    fn module(&mut self, name: &str) -> &mut ModuleSurface {
        self.modules.entry(name.to_string()).or_default()
    }

    /// Every function a host calls in the object, in the order the header declares them.
    fn object_functions(&self) -> impl Iterator<Item = &HostFunction> {
        self.modules.values().flat_map(|module| {
            let behaviors = module.behaviors.iter().filter_map(|it| it.call.as_ref());
            let values = module.values.iter().filter_map(|it| it.read.as_ref());
            let declarations = module
                .declarations
                .iter()
                .flat_map(DeclarationSurface::functions);
            behaviors.chain(values).chain(declarations)
        })
    }

    /// Every function a host calls in the object, as the runtime's are described.
    fn runtime_functions() -> Vec<HostFunction> {
        HOST_RUNTIME
            .iter()
            .map(|function| HostFunction {
                symbol: function.name.to_string(),
                takes: function.takes.to_vec(),
                answers: function.answers,
            })
            .collect()
    }

    /// Every symbol a shared library of the object and the runtime exports: what a host calls, and
    /// nothing else.
    pub(crate) fn exported(&self) -> Vec<String> {
        Self::runtime_functions()
            .into_iter()
            .map(|it| it.symbol)
            .chain(self.object_functions().map(|it| it.symbol.clone()))
            .collect()
    }

    /// The C header declaring every function a host calls.
    ///
    /// Only declarations, the typedefs they are written in and the numbers a host compares a status
    /// and a reading's outcome with, as an enumeration rather than a macro: an FFI that reads C
    /// declarations reads those, and it reads no preprocessor. What is said about each function in
    /// the model's terms is a comment, and the manifest is where it is said to be read.
    pub(crate) fn header(&self) -> String {
        let mut header = format!(
            "/* What a host calls in a Souther program built by souther-native-compiler, and in the\n \
             * runtime it is linked with. Written by the build; souther.json describes the same\n \
             * functions in the model's terms. ABI generation {ABI_GENERATION}. */\n\
             #ifndef SOUTHER_H\n\
             #define SOUTHER_H\n\
             \n\
             #include <stdint.h>\n\
             \n\
             #ifdef __cplusplus\n\
             extern \"C\" {{\n\
             #endif\n\
             \n\
             typedef uint32_t souther_status;\n\
             typedef const struct souther_value_ *souther_value;\n\
             typedef const struct souther_string_ *souther_string;\n\
             typedef const struct souther_decoded_ *souther_decoded;\n\
             typedef const struct souther_issue_ *souther_issue;\n\
             \n"
        );
        let statuses: Vec<String> = statuses()
            .into_iter()
            .map(|(name, number)| format!("    SOUTHER_{name} = {number}"))
            .collect();
        header.push_str(&format!("enum {{\n{}\n}};\n\n", statuses.join(",\n")));
        let outcomes: Vec<String> = outcomes()
            .into_iter()
            .map(|(name, number)| format!("    SOUTHER_DECODED_{name} = {number}"))
            .collect();
        header.push_str(&format!("enum {{\n{}\n}};\n\n", outcomes.join(",\n")));

        header.push_str("/* The runtime. */\n");
        for function in Self::runtime_functions() {
            header.push_str(&function.declared());
            header.push('\n');
        }
        for (name, module) in &self.modules {
            header.push_str(&format!("\n/* {name} */\n"));
            for behavior in &module.behaviors {
                if let Some(call) = &behavior.call {
                    header.push_str(&format!("/* behavior {name}.{} */\n", behavior.name));
                    header.push_str(&call.declared());
                    header.push('\n');
                }
            }
            for value in &module.values {
                if let Some(read) = &value.read {
                    header.push_str(&format!("/* value {name}.{} */\n", value.name));
                    header.push_str(&read.declared());
                    header.push('\n');
                }
            }
            for declaration in &module.declarations {
                let functions: Vec<&HostFunction> = declaration.functions().collect();
                if functions.is_empty() {
                    continue;
                }
                header.push_str(&format!(
                    "/* {} {name}.{} */\n",
                    declaration.kind, declaration.name
                ));
                for function in functions {
                    header.push_str(&function.declared());
                    header.push('\n');
                }
            }
        }
        header.push_str(
            "\n#ifdef __cplusplus\n\
             }\n\
             #endif\n\
             \n\
             #endif\n",
        );
        header
    }

    /// The manifest: every function the header declares, each under the part of the model it
    /// reaches, and the model a binding is written against.
    pub(crate) fn manifest(&self) -> String {
        let modules: Vec<Value> = self
            .modules
            .iter()
            .map(|(name, module)| {
                json!({
                    "name": name,
                    "behaviors": module.behaviors.iter().map(|behavior| json!({
                        "name": behavior.name,
                        "takes": behavior.takes,
                        "answers": behavior.answers,
                        "call": behavior.call.as_ref().map(HostFunction::described),
                    })).collect::<Vec<_>>(),
                    "values": module.values.iter().map(|value| json!({
                        "name": value.name,
                        "type": value.ty,
                        "read": value.read.as_ref().map(HostFunction::described),
                    })).collect::<Vec<_>>(),
                    "declarations": module.declarations.iter().map(|declaration| {
                        let mut described = json!({
                            "kind": declaration.kind,
                            "name": declaration.name,
                            "fields": declaration.fields.iter().map(|field| json!({
                                "name": field.name,
                                "type": field.ty,
                                "read": field.read.as_ref().map(HostFunction::described),
                            })).collect::<Vec<_>>(),
                            "construct": declaration.construct.as_ref().map(HostFunction::described),
                            "case": declaration.case.as_ref().map(HostFunction::described),
                            "decode": declaration.decode.as_ref().map(HostFunction::described),
                            "encode": declaration.encode.as_ref().map(HostFunction::described),
                        });
                        if let Some(cases) = &declaration.cases {
                            described["cases"] = json!(cases);
                        }
                        described
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        let manifest = json!({
            "format": "souther-native-interface",
            "version": MANIFEST_VERSION,
            "abi": ABI_GENERATION,
            "statuses": statuses().into_iter().collect::<BTreeMap<_, _>>(),
            "outcomes": outcomes().into_iter().collect::<BTreeMap<_, _>>(),
            "runtime": Self::runtime_functions().iter().map(HostFunction::described).collect::<Vec<_>>(),
            "modules": modules,
        });
        let mut written =
            serde_json::to_string_pretty(&manifest).expect("a manifest is JSON whatever it holds");
        written.push('\n');
        written
    }
}

/// Every status a generated function answers, by the name the header gives it.
fn statuses() -> Vec<(&'static str, u32)> {
    let mut statuses = vec![("ANSWERED", ANSWERED)];
    statuses.extend(
        AbortKind::ALL
            .iter()
            .map(|kind| (kind.spelt(), native_status(*kind))),
    );
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
fn type_of(ty: &Ty, declared: &Declared) -> Value {
    match ty {
        Ty::Prim { prim } => primitive(*prim),
        Ty::Declared { declared: key } => {
            let declaration = declared.laid(key);
            json!({
                "kind": "declared",
                "module": declaration.module(),
                "name": declaration.name(),
            })
        }
        Ty::Union { union } => json!({
            "kind": "union",
            "cases": union.iter().map(|case| case_of(case, declared)).collect::<Vec<_>>(),
        }),
        Ty::Option { option } => json!({ "kind": "option", "of": type_of(option, declared) }),
        Ty::Tuple { tuple } => json!({
            "kind": "tuple",
            "of": tuple.iter().map(|it| type_of(it, declared)).collect::<Vec<_>>(),
        }),
        Ty::Fn { fn_ } => json!({
            "kind": "function",
            "takes": fn_.takes.iter().map(|it| type_of(it, declared)).collect::<Vec<_>>(),
            "answers": type_of(&fn_.answers, declared),
        }),
        Ty::List { list } => json!({ "kind": "list", "of": type_of(list, declared) }),
        Ty::Set { set } => json!({ "kind": "set", "of": type_of(set, declared) }),
        Ty::Map { map } => json!({
            "kind": "map",
            "key": type_of(&map.key, declared),
            "value": type_of(&map.value, declared),
        }),
    }
}

fn primitive(prim: Prim) -> Value {
    json!({ "kind": "primitive", "name": prim.spelt() })
}

/// A case as the manifest says it.
fn case_of(case: &Case, declared: &Declared) -> Value {
    match case {
        Case::Declared { declared: key } => type_of(
            &Ty::Declared {
                declared: key.clone(),
            },
            declared,
        ),
        Case::Primitive { prim } => primitive(*prim),
        Case::Language { case } => json!({ "kind": "language", "name": case.spelt() }),
    }
}
