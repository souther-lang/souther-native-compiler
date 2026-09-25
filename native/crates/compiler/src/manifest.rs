//! What the manifest is, as types: the protocol a binding for a host language is written against.
//!
//! Written as types and not built as JSON, so that what a manifest of one version says is a thing
//! the compiler holds this code to. A field renamed here is a change to these types, and the
//! fixture `tests/interface-v7.json` is what version 7 is: every manifest this writes is read back
//! by these same types, which refuse a member they do not name.
//!
//! [`VERSION`] moves when what a manifest says is read differently. What the functions it names
//! answer to is [`Manifest::abi`], the generation in every symbol, and the two move apart.
//! Version 2 is `souther-native-compiler#46`: a module says what it asks a host to implement
//! ([`Module::injections`]) beside what it offers one. Version 3 names what a behavior takes as
//! its declaration does ([`Parameters`]), which a binding writes its functions' parameters under.
//! Version 4 is `souther-native-compiler#50`: what a behavior answers is an [`Answer`], which says
//! beside the type how a host tells apart the cases of a union no declaration names. Version 5 is
//! `souther-native-compiler#53`: a module says what a host builds and reads a list through
//! ([`Module::lists`]), and a function may take the words of many elements at once
//! ([`Parameter::Slice`]). Nothing a function of version 4 was called as changed, so the ABI
//! generation did not move with it. Version 6 is `souther-native-compiler#56`: a behavior says
//! what constructing it requires injected ([`Behavior::requires`]), which a binding takes as what
//! it is bound to. No function changed, and the ABI generation did not move either.
//!
//! Where a function is `null`, the model has the thing and a host has no way to reach it yet: a
//! behavior taking a type with no way across, a field of a type with no representation for a host,
//! a type with no external form here. The thing is still described, so a binding can say what it
//! is and that it cannot be reached, rather than not know it is there.

use serde::{Deserialize, Serialize};
use souther_native_abi::{ABI_GENERATION, HostParameter, HostWord};
use std::collections::BTreeMap;

/// What a manifest says it is.
pub(crate) const FORMAT: &str = "souther-native-interface";

/// Which version of what a manifest says this is: the last of [`MOVES`], and written nowhere else
/// on this side.
pub(crate) const VERSION: u32 = MOVES[MOVES.len() - 1].0;

/// What each version of a manifest moved, oldest first. The versions before the first here are in
/// the history of this file.
///
/// A change to what a manifest says adds its line at the end under the next number. Two branches
/// each moving to the same number add two different lines at one place, which a merge stops at;
/// two edits of one constant to the same number merge without a word. That the numbers follow on
/// from one another is held by a test.
pub(crate) const MOVES: &[(u32, &str)] = &[
    (
        5,
        "a list crosses to a host through functions each module defines for how its element \
         crosses (`lists`), and a parameter may be a `slice`",
    ),
    (
        6,
        "a behavior says what constructing it requires injected (`requires`)",
    ),
    (
        7,
        "a declaration names what reads a value a host built of ordered maps (`decodehost`)",
    ),
];

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
///
/// Read through [`Carried::read`] and nothing else: an object another release of this compiler
/// built is refused by the version it says it is ([`crate::versioned`]).
#[derive(Serialize, Debug, Clone, PartialEq)]
pub(crate) struct Carried {
    /// The [`VERSION`] of what the modules say.
    pub version: u32,
    /// The ABI generation their functions answer to.
    pub abi: u32,
    pub modules: Vec<Module>,
}

/// A [`Carried`] as an object holds it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrittenCarried {
    version: u32,
    abi: u32,
    modules: Vec<Module>,
}

/// What a carried surface says it is.
#[derive(Deserialize)]
struct Says {
    version: u32,
    abi: u32,
}

impl Carried {
    /// The surface `carried` holds, where it is of this [`VERSION`] and ABI generation. One of any
    /// other is refused as that, before anything its modules say is read.
    pub(crate) fn read(carried: &[u8]) -> anyhow::Result<Carried> {
        let written: WrittenCarried = crate::versioned::read(carried, |says: Says| {
            if says.version == VERSION && says.abi == ABI_GENERATION {
                Ok(())
            } else {
                Err(format!(
                    "a surface of manifest version {} and ABI generation {}, and this driver \
                     writes version {VERSION} and generation {ABI_GENERATION}",
                    says.version, says.abi
                ))
            }
        })?;
        Ok(Carried {
            version: written.version,
            abi: written.abi,
            modules: written.modules,
        })
    }
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
    /// What a host builds and reads a list through, one for each way an element of a list crosses
    /// where a list crosses in anything above.
    ///
    /// Apart from [`Type::List`], which is what the model says a position holds: a list of one
    /// declared type and a list of another cross through the same functions, and which those are
    /// is a matter of how the element crosses, not of what the model says it is. A binding finds
    /// the one for a position by working out how the position's element crosses.
    pub lists: Vec<ListCrossing>,
}

/// What a host builds and reads a list whose elements cross as `element` through.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListCrossing {
    pub element: Element,
    /// `(count, a slice for each word an element crosses as) -> list`.
    pub construct: Function,
    /// `(list) -> count`.
    pub length: Function,
    /// `(list, index, room for each word an element crosses as) -> bool`: whether the index is
    /// inside the list, the element written through the room only where it is.
    pub at: Function,
}

/// How an element of a list crosses: one word, or a presence beside one for an optional, the way a
/// field of the element's type is handed across.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Element {
    Whole(Word),
    Present(Word),
}

/// A published behavior.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Behavior {
    pub name: String,
    /// What it takes, in order.
    pub parameters: Parameters,
    pub answers: Answer,
    /// What constructing it requires injected, in the order the checker answered it: each a
    /// behavior a host implements ([`Module::injections`]), or one constructed from what it
    /// requires in turn, of this module or another.
    ///
    /// What a binding is bound to, and not what its body calls: a composition requires what its
    /// stages require. A host calls the behavior with an implementation of each registered, and
    /// one of a behavior constructed in turn is an implementation of each of its own.
    pub requires: Vec<Required>,
    /// What a host calls it through.
    pub call: Option<Function>,
}

/// A behavior another requires injected, by its module and its name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Required {
    pub module: String,
    pub name: String,
}

/// What a published behavior answers: the type, as the model says it, and where that is a union no
/// declaration names, what a host tells its cases apart by.
///
/// Beside the type and not in it. [`Type`] is what the model says, the same wherever the type is
/// written, and a union has no function of its own: what tells the cases apart is the behavior's,
/// under the behavior's name, since a union has none.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Answer {
    #[serde(rename = "type")]
    pub ty: Type,
    /// Where `ty` is a union no declaration names, and null where it is anything else.
    pub union: Option<UnionAnswer>,
}

/// The cases of a union a behavior answers, as a value of it is one of them.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnionAnswer {
    /// The cases the union descends to, in the order `case` counts them: a member that is a sum is
    /// its cases here, the way a sum's own `cases` are ([`Declaration::Sum`]), since a value of the
    /// sum is one of them and says which. Not the union's members, which are the type's.
    pub cases: Vec<Case>,
    /// Which of `cases` a value is, where every case is a declared type.
    pub case: Option<Function>,
}

/// A behavior a host implements, and what it registers an implementation through.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Injection {
    pub name: String,
    /// What it takes, in order, as the model says it. Always named: a behavior a host implements
    /// is declared, and a declaration names every parameter.
    pub parameters: Vec<NamedParameter>,
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

/// What a published behavior takes, as the model says it.
///
/// Named where the behavior declares a parameter list, and in order only for a `>->` composition,
/// which declares none: it takes what its first stage takes, and nothing names those inputs. Two
/// forms and not a name that may be null, so that a list naming some parameters and not others is
/// not something a manifest can say.
///
/// The names are the declaration's, not those of a `let` implementing it, which correspond by place
/// and may differ. Apart from [`Implementation::takes`], which is how a host hands each over.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Parameters {
    Named(Vec<NamedParameter>),
    Positional(Vec<Type>),
}

/// One parameter a declaration names.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct NamedParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
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
        /// Reads a value a host built of ordered maps ([`souther_native_abi::host_decode_host_value_symbol`]).
        #[serde(rename = "decodehost")]
        decode_host: Option<Function>,
        encode: Option<Function>,
    },
    Newtype {
        name: String,
        field: Field,
        construct: Option<Function>,
        decode: Option<Function>,
        /// Reads a value a host built of ordered maps ([`souther_native_abi::host_decode_host_value_symbol`]).
        #[serde(rename = "decodehost")]
        decode_host: Option<Function>,
        encode: Option<Function>,
    },
    Unit {
        name: String,
        construct: Option<Function>,
        decode: Option<Function>,
        /// Reads a value a host built of ordered maps ([`souther_native_abi::host_decode_host_value_symbol`]).
        #[serde(rename = "decodehost")]
        decode_host: Option<Function>,
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
        /// Reads a value a host built of ordered maps ([`souther_native_abi::host_decode_host_value_symbol`]).
        #[serde(rename = "decodehost")]
        decode_host: Option<Function>,
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

/// One parameter: a word handed over, room the function writes one through, or as many of a word
/// as another parameter counts, which the function reads and does not keep.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Parameter {
    Given(Word),
    Room(Word),
    Slice(Word),
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
    List,
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
            HostWord::List => Word::List,
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
            Word::List => HostWord::List,
        }
    }
}

impl From<HostParameter> for Parameter {
    fn from(parameter: HostParameter) -> Parameter {
        match parameter {
            HostParameter::Given(word) => Parameter::Given(word.into()),
            HostParameter::Room(word) => Parameter::Room(word.into()),
            HostParameter::Slice(word) => Parameter::Slice(word.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Carried, FORMAT, MOVES, Manifest, VERSION};

    /// Each version is under the number after the one before it.
    #[test]
    fn every_move_takes_the_next_version() {
        for pair in MOVES.windows(2) {
            assert_eq!(
                pair[1].0,
                pair[0].0 + 1,
                "{:?} after {:?}",
                pair[1],
                pair[0]
            );
        }
    }

    /// What version 7 is. Read by these types, which refuse a member they do not name, and
    /// written back the same: a field renamed or a kind reshaped here stops matching the fixture
    /// the Java half's test also holds a written manifest to.
    const V7: &str = include_str!("../tests/interface-v7.json");

    #[test]
    fn version_seven_is_read_and_written_back_as_it_is() {
        let read: Manifest = serde_json::from_str(V7).expect("version 7 reads");
        assert_eq!(read.format, FORMAT);
        assert_eq!(read.version, VERSION);
        let mut written = serde_json::to_string_pretty(&read).unwrap();
        written.push('\n');
        assert_eq!(written, V7);
    }

    /// A surface an object of an earlier release carries is refused as that, and not as whichever
    /// member moved since: a declaration of version 6 says nothing of `decodehost`, which 7 reads.
    #[test]
    fn a_surface_of_an_earlier_version_is_refused_by_its_version() {
        let earlier = br#"{"version":6,"abi":3,"modules":[{"name":"m","behaviors":[],
            "injections":[],"values":[],"declarations":[{"kind":"unit","name":"U",
            "construct":null,"decode":null,"encode":null}],"lists":[]}]}"#;

        let refused = Carried::read(earlier).expect_err("a surface of another version");

        assert!(
            refused
                .to_string()
                .contains("manifest version 6 and ABI generation 3"),
            "{refused}"
        );
    }
}
