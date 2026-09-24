//! What the manifest is, as types: the protocol a binding for a host language is written against.
//!
//! Written as types and not built as JSON, so that what a manifest of one version says is a thing
//! the compiler holds this code to. A field renamed here is a change to these types, and the
//! fixture `tests/interface-v2.json` is what version 2 is: every manifest this writes is read back
//! by these same types, which refuse a member they do not name.
//!
//! [`VERSION`] moves when what a manifest says is read differently. What the functions it names
//! answer to is [`Manifest::abi`], the generation in every symbol, and the two move apart.
//! Version 2 is `souther-native-compiler#46`: a module says what it asks a host to implement
//! ([`Module::injections`]) beside what it offers one.
//!
//! Where a function is `null`, the model has the thing and a host has no way to reach it yet: a
//! behavior taking a type with no way across, a field of a type with no representation for a host,
//! a type with no external form here. The thing is still described, so a binding can say what it
//! is and that it cannot be reached, rather than not know it is there.

use serde::{Deserialize, Serialize};
use souther_native_abi::{HostParameter, HostWord};
use std::collections::BTreeMap;

/// What a manifest says it is.
pub(crate) const FORMAT: &str = "souther-native-interface";

/// Which version of what a manifest says this is.
pub(crate) const VERSION: u32 = 2;

/// Everything a host can call in one shared library, and the model it reaches.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    /// Always [`FORMAT`].
    pub format: String,
    /// Always [`VERSION`].
    pub version: u32,
    /// The ABI generation every function named here answers to.
    pub abi: u32,
    /// Every status a function answering one answers, by name: `ANSWERED`, each reason a Souther
    /// computation ends without a value, and what a host's implementation of a behavior brings
    /// about, which no computation answers.
    pub statuses: BTreeMap<String, u32>,
    /// What a reading comes to, by name.
    pub outcomes: BTreeMap<String, i32>,
    /// The runtime's functions a host calls.
    pub runtime: Vec<Function>,
    /// What each module a library holds publishes.
    pub modules: Vec<Module>,
}

/// What one object carries of its own surface, in a section of its own: every module it holds, as
/// the manifest says them. A library is made of the objects it links, and what it offers a host is
/// what each of them carries, so an object built by another build is described by itself and not
/// by whatever links it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Carried {
    /// The [`VERSION`] of what the modules say.
    pub version: u32,
    /// The ABI generation their functions answer to.
    pub abi: u32,
    pub modules: Vec<Module>,
}

/// What one module publishes, and what it asks a host to implement.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Module {
    /// The module's name, dots and all.
    pub name: String,
    /// Every behavior it publishes and the object defines.
    pub behaviors: Vec<Behavior>,
    /// Every behavior it declares with no body and nothing to depend on, which a host implements.
    ///
    /// Apart from `behaviors`, which a host calls: these are what a host is called for. Whether the
    /// module publishes one does not decide whether it is here. A published behavior that reaches
    /// one runs only once a host registered an implementation for it, whoever may name it.
    pub injections: Vec<Injection>,
    /// Every value it publishes.
    pub values: Vec<PublishedValue>,
    /// Every type it declares and publishes.
    pub declarations: Vec<Declaration>,
}

/// A published behavior.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Behavior {
    pub name: String,
    /// What it takes, in order. The names its parameters are written under do not cross yet.
    pub takes: Vec<Type>,
    pub answers: Type,
    /// What a host calls it through.
    pub call: Option<Function>,
}

/// A behavior a host implements, and what it registers an implementation through.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Injection {
    pub name: String,
    /// What it takes, in order, as the model says it.
    pub takes: Vec<Type>,
    pub answers: Type,
    /// The function a host writes to implement it.
    pub implementation: Implementation,
    /// The symbol a host calls to register an implementation on the thread it calls from: it
    /// takes a pointer to a function of `implementation`'s type and answers the one it replaced,
    /// either null for none.
    ///
    /// What is registered is the host's, and stays callable for as long as it is registered on any
    /// thread: the object calls it on every call of the behavior and keeps no copy of it. So a
    /// binding makes the pointer once for what it registers and hands the same one over each time;
    /// a host language that makes a new C entry for a function every time it is handed to C (PHP's
    /// FFI keeps each until the request ends) would otherwise grow with every call.
    pub register: String,
}

/// The type of a function a host writes, and not a function: nothing is defined under its name,
/// which is what C calls a pointer to one. What it takes is what the behavior takes, as a host
/// hands each over, and room for its answer, as a host is handed one; it answers a status, of which
/// `ANSWERED` and `HOST_EXCEPTION` are what an implementation may answer.
///
/// Its own type and not a [`Function`], so that every [`Function`] a manifest names is a symbol a
/// library defines.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Implementation {
    #[serde(rename = "type")]
    pub type_name: String,
    pub takes: Vec<Parameter>,
    pub answers: Word,
}

/// A published value.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublishedValue {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    /// What a host reads it through.
    pub read: Option<Function>,
}

/// A published type, as its declaration says it, with what a host reaches it through.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Declaration {
    Product {
        name: String,
        fields: Vec<Field>,
        construct: Option<Function>,
        decode: Option<Function>,
        encode: Option<Function>,
    },
    Newtype {
        name: String,
        field: Field,
        construct: Option<Function>,
        decode: Option<Function>,
        encode: Option<Function>,
    },
    Unit {
        name: String,
        construct: Option<Function>,
        decode: Option<Function>,
        encode: Option<Function>,
    },
    Sum {
        name: String,
        /// The cases the sum descends to, in the order `case` counts them. A case the module
        /// keeps is named here and has no declaration of its own in the manifest: a host is told
        /// a value is one, and reaches nothing of it.
        cases: Vec<Case>,
        /// Which of `cases` a value is, where every case is a declared type.
        case: Option<Function>,
        decode: Option<Function>,
        encode: Option<Function>,
    },
}

/// One field of a product or a newtype.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    /// What a host reads it through.
    pub read: Option<Function>,
}

/// A type, as the model says it. A declared type is its module and its name, never a key of the
/// document the compiler read.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Type {
    Primitive {
        name: Primitive,
    },
    Declared {
        module: String,
        name: String,
    },
    Union {
        cases: Vec<Case>,
    },
    Option {
        of: Box<Type>,
    },
    Tuple {
        of: Vec<Type>,
    },
    Function {
        takes: Vec<Type>,
        answers: Box<Type>,
    },
    List {
        of: Box<Type>,
    },
    Set {
        of: Box<Type>,
    },
    Map {
        key: Box<Type>,
        value: Box<Type>,
    },
}

/// One case of a sum or a union.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Case {
    Declared { module: String, name: String },
    Primitive { name: Primitive },
    Language { name: LanguageCase },
}

/// A primitive, by the name the language gives it.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Primitive {
    Int,
    String,
    Bool,
    Decimal,
    Rational,
    Date,
    Time,
    DateTime,
    Instant,
    Raw,
}

/// A case the language gives, by its name.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LanguageCase {
    Some,
    None,
    DivisionByZero,
    NotANumber,
    NotADate,
    NotATime,
    NotWhole,
    NotAFiniteDecimal,
}

/// A function a host calls: its symbol, which is its name in C and which the library defines, and
/// what it takes and answers.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Function {
    pub name: String,
    pub takes: Vec<Parameter>,
    pub answers: Option<Word>,
}

/// One parameter: a word handed over, or room the function writes one through.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Parameter {
    Given(Word),
    Room(Word),
}

/// One word a host hands over or is handed. What each is on the machine is `souther_native_abi`'s,
/// and the manifest names them the way that crate does.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Word {
    Status,
    Int,
    Bool,
    Case,
    Outcome,
    Count,
    Mark,
    Bytes,
    Value,
    String,
    Decoded,
    Issue,
}

impl From<HostWord> for Word {
    fn from(word: HostWord) -> Word {
        match word {
            HostWord::Status => Word::Status,
            HostWord::Int => Word::Int,
            HostWord::Bool => Word::Bool,
            HostWord::Case => Word::Case,
            HostWord::Outcome => Word::Outcome,
            HostWord::Count => Word::Count,
            HostWord::Mark => Word::Mark,
            HostWord::Bytes => Word::Bytes,
            HostWord::Value => Word::Value,
            HostWord::String => Word::String,
            HostWord::Decoded => Word::Decoded,
            HostWord::Issue => Word::Issue,
        }
    }
}

impl From<Word> for HostWord {
    fn from(word: Word) -> HostWord {
        match word {
            Word::Status => HostWord::Status,
            Word::Int => HostWord::Int,
            Word::Bool => HostWord::Bool,
            Word::Case => HostWord::Case,
            Word::Outcome => HostWord::Outcome,
            Word::Count => HostWord::Count,
            Word::Mark => HostWord::Mark,
            Word::Bytes => HostWord::Bytes,
            Word::Value => HostWord::Value,
            Word::String => HostWord::String,
            Word::Decoded => HostWord::Decoded,
            Word::Issue => HostWord::Issue,
        }
    }
}

impl From<HostParameter> for Parameter {
    fn from(parameter: HostParameter) -> Parameter {
        match parameter {
            HostParameter::Given(word) => Parameter::Given(word.into()),
            HostParameter::Room(word) => Parameter::Room(word.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FORMAT, Manifest, VERSION};

    /// What version 2 is. Read by these types, which refuse a member they do not name, and
    /// written back the same: a field renamed or a kind reshaped here stops matching the fixture
    /// the Java half's test also holds a written manifest to.
    const V2: &str = include_str!("../tests/interface-v2.json");

    #[test]
    fn version_two_is_read_and_written_back_as_it_is() {
        let read: Manifest = serde_json::from_str(V2).expect("version 2 reads");
        assert_eq!(read.format, FORMAT);
        assert_eq!(read.version, VERSION);
        let mut written = serde_json::to_string_pretty(&read).unwrap();
        written.push('\n');
        assert_eq!(written, V2);
    }
}
