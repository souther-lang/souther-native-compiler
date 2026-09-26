//! What the manifest is, as types: the protocol a binding for a host language is written against.
//!
//! Written as types and not built as JSON, so that what a manifest of one version says is a thing
//! the compiler holds this code to. A field renamed here is a change to these types, and the
//! fixture `tests/interface-v11.json` is what version 11 is: every manifest this writes is read
//! back by these same types, which refuse a member they do not name.
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
//! Where what reaches a value is [`Reach::Unavailable`], the model has the thing and a host has no
//! way to reach it yet, and the manifest says why, as a reason and the place in the type it stands
//! at ([`Refusal`]): a behavior taking a type with no representation for a host, a field whose
//! type has none, a union a host would be handed with nothing to say which case it is. The thing
//! is still described, so a binding can say what it is and why it cannot be reached, rather than
//! not know it is there. Version 10 says so; before it, such a function was `null`, which a type
//! with no external form here still is ([`Declaration`]'s `decode` and `encode`).
//!
//! What a function reaching a value hands over and is handed is said beside it as the [`Shape`]
//! each value crosses in, which is the compiler's decision and the one a binding reads: a binding
//! does not work out again from the model how a value crosses.

use serde::{Deserialize, Serialize};
use souther_native_abi::{ABI_GENERATION, HostLeaf, HostParameter, HostShape, HostWord};
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
    (
        8,
        "a behavior is called with the capabilities it was constructed with, what a host constructs \
         them out of is apart from what it calls (`constructions`), a host makes one of a behavior \
         (`bind`) and of an implementation of its own (`implement`), and nothing is registered on a \
         thread",
    ),
    (
        9,
        "a primitive and a case the language gives cross to a host as a case of a union: a host \
         makes and reads each through the runtime (`cases`), and a union's `case` answers for \
         every union and not only one of declared cases",
    ),
    (
        10,
        "what reaches a value says the shape each value crosses in (`signature`), or why it cannot \
         be reached (`unavailable`); a tuple, an optional at any depth and a function value cross, \
         a list is built and read through functions for the shape its element crosses in, and a \
         function value called and made through functions for its own (`functions`), each only \
         where something crossing that way needs it; a field's reader writes the field through \
         room; every type the model has is named, `nothing` and `never` among them",
    ),
    (
        11,
        "a `Decimal` crosses as a leaf of its own (`decimal`): a host makes one of its integer, as \
         integer text, and its scale, and reads the two back, through the runtime \
         (`souther_decimal_of_parts`, `souther_decimal_unscaled`, `souther_decimal_scale`), and \
         carries one as a case of a union",
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
    /// How a host makes and reads a value of each case no declaration names, as a union holds one:
    /// a primitive and a case the language gives. The runtime's too, and apart from `runtime`,
    /// because each is said of a case: a binding finds the functions for a case of a union here, by
    /// the case, whatever union it stands in.
    pub cases: Vec<CaseCrossing>,
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
    /// Every behavior the object defines that a host constructs the capabilities of what a call is
    /// made with out of: each published one that requires something, and each one a published one
    /// requires, at any depth, whether or not the module publishes it.
    ///
    /// Apart from `behaviors`, which is what a host calls by name. A behavior the module keeps that
    /// a published one depends on is here and not there: a host has no name for it and builds a
    /// capability of it all the same. Closed: every behavior one of these requires is one a host
    /// implements (`injections`), one here, or one another build defines, which that build's
    /// surface closes.
    pub constructions: Vec<Construction>,
    /// Every behavior it declares with no body and nothing to depend on, which a host implements.
    ///
    /// Apart from `behaviors`, which a host calls: these are what a host is called for. Whether the
    /// module publishes one does not decide whether it is here. A published behavior that requires
    /// one runs only with a capability for it among what it is called with, whoever may name it.
    pub injections: Vec<Injection>,
    /// Every value it publishes.
    pub values: Vec<PublishedValue>,
    /// Every type it declares and publishes.
    pub declarations: Vec<Declaration>,
    /// What a host builds and reads a list through, one for each shape an element of a list
    /// crosses in, where a list crosses in anything above or in anything here.
    ///
    /// Apart from [`Type::List`], which is what the model says a position holds: a list of one
    /// declared type and a list of another cross through the same functions, and which those are
    /// is a matter of the shape the element crosses in, not of what the model says it is. A binding
    /// finds the one for a position by the [`Shape::List`] said beside it.
    pub lists: Vec<ListCrossing>,
    /// What a host calls and makes a function value through, one for each shape a function value
    /// crosses in, where one crosses in anything above or in anything here. Apart from
    /// [`Type::Function`] for the reason `lists` is apart from [`Type::List`].
    pub functions: Vec<FunctionCrossing>,
}

/// What a host calls a function value that crosses in the shape `signature` says through, and makes
/// one of its own through: each only where something crossing needs it, since which a host may do
/// is decided by the way each function value crosses and not by its shape. A host handed a function
/// taking a union no declaration names calls it, handing the union over, and has nothing to be
/// told which case one is where it would be handed one by a function of its own, so no function of
/// that shape is made by a host unless something takes one from a host. At least one of the two is
/// there.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct FunctionCrossing {
    /// What a function value of this shape takes and answers, as each crosses.
    pub signature: Signature,
    /// `(function, what it takes, room for each word of its answer) -> status`, where a host is
    /// handed a function value of this shape.
    pub call: Option<Function>,
    /// What a host makes one of its own through, where one is taken from a host.
    pub make: Option<FunctionMaking>,
}

/// What a host makes a function value of its own through.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct FunctionMaking {
    /// The function a host writes to make a function value of its own: handed what it was handed
    /// where the value was made, then what the value was called with, and room for each word of
    /// its answer, answering a status as an implementation of a behavior does.
    pub implementation: Implementation,
    /// The symbol a host makes a function value of an implementation of its own through: `(room
    /// for what the header calls `souther_hosted_function`, a function of `implementation`'s type,
    /// what it is handed first) -> function`, the room laid out and answered as the value. The
    /// room, the function and what it is handed are the host's, and stay as they are for as long as
    /// the value may be called, as they are for a behavior a host implements
    /// ([`Injection::implement`]).
    pub implement: String,
}

/// What a function a host reaches takes and answers, as the shape each value crosses in: what it
/// takes in order, and what it answers.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Signature {
    pub takes: Vec<Shape>,
    pub answers: Box<Shape>,
}

/// What a host builds a list whose elements cross in the shape `element` through, and reads one
/// through: each only where something crossing needs it, for the reason a [`FunctionCrossing`]
/// says each only where needed. A list of a union no declaration names may be built by a host,
/// which hands each element over as the case it is, and is read by none. At least one of the two
/// is there.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListCrossing {
    pub element: Shape,
    /// `(count, a slice for each word an element crosses as) -> list`, where a host hands a list
    /// of these over.
    pub construct: Option<Function>,
    /// What a host reads one through, where a host is handed one.
    pub read: Option<ListRead>,
}

/// What a host reads a list through.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListRead {
    /// `(list) -> count`.
    pub length: Function,
    /// `(list, index, room for each word an element crosses as) -> bool`: whether the index is
    /// inside the list, the element written through the room only where it is.
    pub at: Function,
}

/// The shape a value of the model crosses between a host and the library in: the words it is
/// handed over as, and what they are made of. The manifest's spelling of
/// [`souther_native_abi::HostShape`], which is what decides it.
///
/// Not a type of the model: a value of a declared type and one of a union are both one `value`,
/// and a tuple is a `product` of its members. What the value is, is said beside it by [`Type`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Shape {
    /// One word that is the value itself.
    Leaf(Leaf),
    /// A presence, one `bool`, then the words of what it holds, read and written only where it
    /// holds something.
    Option(Box<Shape>),
    /// Each member's words, one member after another.
    Product(Vec<Shape>),
    /// One `list`, built and read through the [`ListCrossing`] for the element's shape.
    List(Box<Shape>),
    /// One `function`, called and made through the [`FunctionCrossing`] for this signature.
    Function(Signature),
}

/// A word a value of the model is itself handed over as.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Leaf {
    Int,
    Bool,
    String,
    Decimal,
    Value,
}

impl From<Leaf> for HostLeaf {
    fn from(leaf: Leaf) -> HostLeaf {
        match leaf {
            Leaf::Int => HostLeaf::Int,
            Leaf::Bool => HostLeaf::Bool,
            Leaf::String => HostLeaf::String,
            Leaf::Decimal => HostLeaf::Decimal,
            Leaf::Value => HostLeaf::Value,
        }
    }
}

impl From<&HostShape> for Shape {
    fn from(shape: &HostShape) -> Shape {
        match shape {
            HostShape::Leaf(leaf) => Shape::Leaf(match leaf {
                HostLeaf::Int => Leaf::Int,
                HostLeaf::Bool => Leaf::Bool,
                HostLeaf::String => Leaf::String,
                HostLeaf::Decimal => Leaf::Decimal,
                HostLeaf::Value => Leaf::Value,
            }),
            HostShape::Option(of) => Shape::Option(Box::new(of.as_ref().into())),
            HostShape::Product(members) => {
                Shape::Product(members.iter().map(|it| it.into()).collect())
            }
            HostShape::List(element) => Shape::List(Box::new(element.as_ref().into())),
            HostShape::Function { takes, answers } => Shape::Function(Signature {
                takes: takes.iter().map(|it| it.into()).collect(),
                answers: Box::new(answers.as_ref().into()),
            }),
        }
    }
}

/// What reaches a value, or why nothing does.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Reach<T> {
    Available(T),
    Unavailable(Refusal),
}

/// Why a host has no way to a value: what stands in the way, and where in what the function would
/// hand over or be handed it stands, from the outside in.
///
/// A reason a binding says in its own words, and not a message: a binding for another language
/// says it the way that language says a thing is not there.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Refusal {
    pub reason: Reason,
    pub path: Vec<Step>,
}

impl std::fmt::Display for Refusal {
    /// The reason in words, then where it stands, from the outside in: `no representation for a
    /// host, at what is taken at 0, what an optional holds`.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self.reason {
            Reason::NoRepresentation => "no representation for a host",
            Reason::NoValue => "no value to hand over",
            Reason::NoDiscriminator => "a union with nothing to say which case it is",
        })?;
        for (at, step) in self.path.iter().enumerate() {
            f.write_str(if at == 0 { ", at " } else { ", " })?;
            match step {
                Step::Takes(place) => write!(f, "what is taken at {place}")?,
                Step::Answers => f.write_str("what is answered")?,
                Step::Field(name) => write!(f, "the field {name}")?,
                Step::Option => f.write_str("what an optional holds")?,
                Step::Member(place) => write!(f, "the member at {place}")?,
                Step::Element => f.write_str("an element")?,
            }
        }
        Ok(())
    }
}

/// What stands in the way of a value crossing to a host.
///
/// Each says what the value has none of, which is how a binding reads it, so each starts alike.
#[allow(clippy::enum_variant_names)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Reason {
    /// A type with no representation for a host yet: a `Decimal`, a date, a `Set`, a `Map`.
    NoRepresentation,
    /// A type with no value to hand over: what an empty list holds, and what does not answer.
    NoValue,
    /// A union no declaration names, which a host would be handed with nothing to say which case
    /// it is: only a behavior's answer is told its case ([`Answer::union`]).
    NoDiscriminator,
}

/// One step into what a function hands over or is handed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Step {
    /// What is taken at this place, counted from nought: a behavior's parameter, or a function
    /// value's.
    Takes(usize),
    /// What is answered.
    Answers,
    /// A field of a declared type, by its name.
    Field(String),
    /// What an optional holds.
    Option,
    /// A tuple's member at this place.
    Member(usize),
    /// A list's element.
    Element,
}

/// A behavior's or a published value's call, and the shape each value it takes and answers
/// crosses in.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Call {
    /// What was constructed with first where it is a behavior that is, then what `signature` takes,
    /// then room for each word of what it answers, answering a status.
    pub function: Function,
    pub signature: Signature,
}

/// A declared type's constructor, and the shape each field is handed over in.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Construct {
    /// Each field, then room for the value, answering a status.
    pub function: Function,
    pub takes: Vec<Shape>,
}

/// A field's reader, and the shape the field is handed over in.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Read {
    /// The value, then room for each word of the field, answering nothing.
    pub function: Function,
    pub answers: Shape,
}

/// A published behavior.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Behavior {
    pub name: String,
    /// What it takes, in order.
    pub parameters: Parameters,
    pub answers: Answer,
    /// What a host calls it through: what it was constructed with first, then what it takes.
    pub call: Reach<Call>,
}

/// What a host constructs the capabilities a behavior is called with out of
/// ([`Module::constructions`]).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Construction {
    pub name: String,
    /// What constructing it requires injected, in the order the checker answered it: each a
    /// behavior a host implements ([`Module::injections`]), or one constructed from what it
    /// requires in turn, of this module or another.
    ///
    /// What a binding is bound to, and not what its body calls: a composition requires what its
    /// stages require. A host calls the behavior with a capability for each, in this order: of an
    /// implementation of its own for a behavior a host implements, and of the behavior for one
    /// constructed in turn, made from capabilities of what that one requires.
    pub requires: Vec<Required>,
    /// What a host makes a capability of it through, out of the capabilities of what it requires,
    /// to hand where something requires it: `(room for a capability, requirements)`. Only for a
    /// behavior with a body, which is what something may depend on.
    pub bind: Option<Function>,
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

/// How a host makes a value of a case no declaration names and reads what it holds.
///
/// Said of the case and not of any union: a value of the case is laid out alike in every union it
/// stands in. Which case a value of a union is, is the union's to say ([`UnionAnswer::case`]);
/// `read` is called only on a value that says it is this case.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaseCrossing {
    pub case: Case,
    /// `(what the case holds, where it holds something) -> value`.
    pub make: Function,
    /// `(value) -> what it holds`, where the case holds something.
    pub read: Option<Function>,
}

/// The cases of a union a behavior answers, as a value of it is one of them.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnionAnswer {
    /// The cases the union descends to, in the order `case` counts them: a member that is a sum is
    /// its cases here, the way a sum's own `cases` are ([`Declaration::Sum`]), since a value of the
    /// sum is one of them and says which. Not the union's members, which are the type's.
    pub cases: Vec<Case>,
    /// Which of `cases` a value is, where a host can be handed a value of the union: `null` only
    /// where the behavior has no way to be called ([`Reach::Unavailable`]). A declared case is then the value itself, and
    /// one no declaration names is read through [`Manifest::cases`].
    pub case: Option<Function>,
}

/// A behavior a host implements, and what it makes a capability of an implementation of its own
/// through.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Injection {
    pub name: String,
    /// What it takes, in order, as the model says it. Always named: a behavior a host implements
    /// is declared, and a declaration names every parameter.
    pub parameters: Vec<NamedParameter>,
    pub answers: Type,
    /// What it takes and answers, as each crosses: what a host is handed and what it hands back.
    pub signature: Signature,
    /// The function a host writes to implement it.
    pub implementation: Implementation,
    /// The symbol a host makes a capability of an implementation through: `(room for a capability,
    /// room for what the header calls `souther_hosted`, a function of `implementation`'s type,
    /// what it is handed first)`, writing both rooms. What the capability is called with is handed
    /// to the function, and what the function answers is held to what an implementation may answer.
    ///
    /// The function, what it is handed and both rooms are the host's, and stay as they are for as
    /// long as the capability may be called: nothing is copied out of them. So a binding makes the
    /// function pointer once for a behavior and hands the same one over each time, telling
    /// implementations apart by what each is handed first; a host language that makes a new C entry
    /// for a function every time it is handed to C (PHP's FFI keeps each until the request ends)
    /// would otherwise grow with every capability it makes.
    pub implement: String,
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
/// which is what C calls a pointer to one. What it takes is what it was handed first where its
/// capability was made, then what the behavior takes, as a host hands each over, and room for its
/// answer, as a host is handed one; it answers a status, of which `ANSWERED` and `HOST_EXCEPTION`
/// are what an implementation may answer.
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
    /// What a host reads it through: nothing taken, and room for each word of the value.
    pub read: Reach<Call>,
}

/// A published type, as its declaration says it, with what a host reaches it through.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Declaration {
    Product {
        name: String,
        fields: Vec<Field>,
        construct: Reach<Construct>,
        decode: Option<Function>,
        /// Reads a value a host built of ordered maps ([`souther_native_abi::host_decode_host_value_symbol`]).
        #[serde(rename = "decodehost")]
        decode_host: Option<Function>,
        encode: Option<Function>,
    },
    Newtype {
        name: String,
        field: Field,
        construct: Reach<Construct>,
        decode: Option<Function>,
        /// Reads a value a host built of ordered maps ([`souther_native_abi::host_decode_host_value_symbol`]).
        #[serde(rename = "decodehost")]
        decode_host: Option<Function>,
        encode: Option<Function>,
    },
    Unit {
        name: String,
        construct: Reach<Construct>,
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
        /// Which of `cases` a value is, where a host can be handed a value of every case.
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
    pub read: Reach<Read>,
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
    /// What has no value: what an empty list holds, before anything says more of it.
    Nothing,
    /// What does not answer: an `unreachable`, standing where anything could.
    Never,
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
    Decimal,
    Decoded,
    Issue,
    List,
    Requirements,
    Capability,
    Userdata,
    Function,
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
            HostWord::Decimal => Word::Decimal,
            HostWord::Decoded => Word::Decoded,
            HostWord::Issue => Word::Issue,
            HostWord::List => Word::List,
            HostWord::Requirements => Word::Requirements,
            HostWord::Capability => Word::Capability,
            HostWord::Userdata => Word::Userdata,
            HostWord::Function => Word::Function,
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
            Word::Decimal => HostWord::Decimal,
            Word::Decoded => HostWord::Decoded,
            Word::Issue => HostWord::Issue,
            Word::List => HostWord::List,
            Word::Requirements => HostWord::Requirements,
            Word::Capability => HostWord::Capability,
            Word::Userdata => HostWord::Userdata,
            Word::Function => HostWord::Function,
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

    /// What version 11 is. Read by these types, which refuse a member they do not name, and
    /// written back the same: a field renamed or a kind reshaped here stops matching the fixture
    /// the Java half's test also holds a written manifest to.
    const V11: &str = include_str!("../tests/interface-v11.json");

    #[test]
    fn version_eleven_is_read_and_written_back_as_it_is() {
        let read: Manifest = serde_json::from_str(V11).expect("version 11 reads");
        assert_eq!(read.format, FORMAT);
        assert_eq!(read.version, VERSION);
        let mut written = serde_json::to_string_pretty(&read).unwrap();
        written.push('\n');
        assert_eq!(written, V11);
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
