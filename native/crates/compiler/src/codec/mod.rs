//! A value in the language's external form, both ways: generated code that walks a value and
//! builds the runtime's tree of it ([`write`]), and generated code that walks a document and builds
//! a value of it ([`read`]).
//!
//! The two read one thing, what the checker settled about every position a value stands at: which
//! scalar a field is, whether an absent one is left out or written `null`, whether a set of
//! alternatives travels as a bare name or a discriminated object, and both keys of the second. A
//! reader is the writer walked backwards over the same shapes, arm for arm, so what one writes is
//! what the other reads, and nothing about a representation is decided in either. What is this
//! backend's own is where a value is in memory, and reading that is the same reading the rest of the
//! lowering does.
//!
//! One writer and one reader per declaration, each a function of this object's own and compiled
//! from the declaration, so nothing about a declaration is carried to run time for the runtime to
//! interpret. What the runtime is asked is only about one place of a document or one node of the
//! tree: is it an object, what is under this key, is it an `Int`.
//!
//! A reader does not build a value. It gathers what the declaration's fields were written as and
//! hands them to the construction every other value goes through, so a value read from a document
//! is one its type admits by the same clauses, run in the same order, as a value a body built.

pub(crate) mod read;
pub(crate) mod write;

use super::{Declared, Emitting, Lowered, POINTER, accepted};
use crate::transport::{Case, CodecShape, Declaration, DeclaredBy, Prim};
use cranelift::codegen::ir::{self, AbiParam, types};
use cranelift::codegen::isa::CallConv;
use cranelift::module::{FuncId, Linkage, Module};
use cranelift::object::ObjectModule;
use souther_native_abi::{
    DECODE_ABANDON, DECODE_BEGIN, DECODE_END, DECODE_ROOT, EXTERNAL_BOOL, EXTERNAL_INT,
    EXTERNAL_JSON, EXTERNAL_NULL, EXTERNAL_OBJECT, EXTERNAL_PUT, EXTERNAL_STRING, PATH_BELOW,
    READ_BOOL, READ_CASE, READ_INT, READ_INVARIANT, READ_IS, READ_MEMBER, READ_MISSING,
    READ_NOT_A_CASE, READ_NULL, READ_OBJECT, READ_STRING, READ_TAG, reader_symbol,
};
use std::collections::{BTreeMap, BTreeSet};

/// Every declaration whose values have an external form this backend reads and writes, by the key a
/// reference to it says.
///
/// A scalar it holds is an `Int`, a `Bool` or a `String`; a declared type it holds is one of these
/// again; a set of alternatives is made of declared cases, each one of these. What the language
/// declares is none: no build defines its token, so no value of it is made here.
///
/// The greatest set that holds, worked out by striking what fails and then what reaches something
/// struck until nothing more is: a type that holds itself, through an optional, is one of these as
/// long as nothing else it holds is not.
pub(crate) fn carried(declarations: &[Declaration], declared: &Declared) -> BTreeSet<String> {
    let mut held: BTreeSet<String> = declarations
        .iter()
        .filter(|declaration| declaration.by() != DeclaredBy::TheLanguage)
        .map(Declaration::key)
        .collect();
    loop {
        let struck: Vec<String> = held
            .iter()
            .filter(|key| {
                !reaches(declared.laid(key))
                    .iter()
                    .all(|reached| reached.is_some_and(|it| held.contains(it)))
            })
            .cloned()
            .collect();
        if struck.is_empty() {
            return held;
        }
        for key in struck {
            held.remove(&key);
        }
    }
}

/// The declarations a value of `declaration` is written through: each one a field of it holds, and
/// each case of it. `None` stands for something with no external form here at all.
fn reaches(declaration: &Declaration) -> Vec<Option<&str>> {
    match declaration {
        Declaration::Product { .. } | Declaration::Newtype { .. } => {
            let mut reached = Vec::new();
            for field in declaration.fields() {
                shape_reaches(&field.codec, &mut reached);
            }
            reached
        }
        Declaration::Unit { .. } => Vec::new(),
        Declaration::Sum { cases, .. } => cases
            .iter()
            .map(|case| match case {
                Case::Declared { declared } => Some(declared.as_str()),
                Case::Primitive { .. } | Case::Language { .. } => None,
            })
            .collect(),
    }
}

fn shape_reaches<'s>(shape: &'s CodecShape, reached: &mut Vec<Option<&'s str>>) {
    match shape {
        CodecShape::Scalar { scalar } => match scalar.prim() {
            Prim::Int | Prim::Bool | Prim::String => {}
            Prim::Decimal
            | Prim::Rational
            | Prim::Date
            | Prim::Time
            | Prim::DateTime
            | Prim::Instant
            | Prim::Raw => reached.push(None),
        },
        CodecShape::Named { declared } => reached.push(Some(declared)),
        CodecShape::OptionOf { present } => shape_reaches(present.shape(), reached),
        CodecShape::ListOf { .. } | CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => {
            reached.push(None)
        }
    }
}

/// Every declaration a value of one of `roots` is read through, the roots among them.
pub(crate) fn reached_from(
    roots: impl IntoIterator<Item = String>,
    declared: &Declared,
) -> BTreeSet<String> {
    let mut reached = BTreeSet::new();
    let mut left: Vec<String> = roots.into_iter().collect();
    while let Some(key) = left.pop() {
        if !reached.insert(key.clone()) {
            continue;
        }
        for next in reaches(declared.laid(&key)).into_iter().flatten() {
            left.push(next.to_string());
        }
    }
    reached
}

/// What the runtime answers for generated code, each with the signature it is called by. The
/// ownership and meaning of each are stated beside its symbol in `souther-native-abi`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Runtime {
    ExternalNull,
    ExternalBool,
    ExternalInt,
    ExternalString,
    ExternalObject,
    ExternalPut,
    ExternalJson,
    DecodeBegin,
    DecodeRoot,
    DecodeEnd,
    DecodeAbandon,
    PathBelow,
    ReadObject,
    ReadMember,
    ReadMissing,
    ReadNull,
    ReadInt,
    ReadBool,
    ReadString,
    ReadCase,
    ReadTag,
    ReadIs,
    ReadNotACase,
    ReadInvariant,
}

impl Runtime {
    fn symbol(self) -> &'static str {
        match self {
            Runtime::ExternalNull => EXTERNAL_NULL,
            Runtime::ExternalBool => EXTERNAL_BOOL,
            Runtime::ExternalInt => EXTERNAL_INT,
            Runtime::ExternalString => EXTERNAL_STRING,
            Runtime::ExternalObject => EXTERNAL_OBJECT,
            Runtime::ExternalPut => EXTERNAL_PUT,
            Runtime::ExternalJson => EXTERNAL_JSON,
            Runtime::DecodeBegin => DECODE_BEGIN,
            Runtime::DecodeRoot => DECODE_ROOT,
            Runtime::DecodeEnd => DECODE_END,
            Runtime::DecodeAbandon => DECODE_ABANDON,
            Runtime::PathBelow => PATH_BELOW,
            Runtime::ReadObject => READ_OBJECT,
            Runtime::ReadMember => READ_MEMBER,
            Runtime::ReadMissing => READ_MISSING,
            Runtime::ReadNull => READ_NULL,
            Runtime::ReadInt => READ_INT,
            Runtime::ReadBool => READ_BOOL,
            Runtime::ReadString => READ_STRING,
            Runtime::ReadCase => READ_CASE,
            Runtime::ReadTag => READ_TAG,
            Runtime::ReadIs => READ_IS,
            Runtime::ReadNotACase => READ_NOT_A_CASE,
            Runtime::ReadInvariant => READ_INVARIANT,
        }
    }

    /// What it takes and what it answers, if anything.
    fn signature(self) -> (&'static [types::Type], Option<types::Type>) {
        const P: types::Type = POINTER;
        match self {
            Runtime::ExternalNull | Runtime::ExternalObject => (&[], Some(P)),
            Runtime::ExternalBool => (&[types::I8], Some(P)),
            Runtime::ExternalInt => (&[types::I64], Some(P)),
            Runtime::ExternalString | Runtime::ExternalJson | Runtime::DecodeRoot => {
                (&[P], Some(P))
            }
            Runtime::ExternalPut => (&[P, P, P], None),
            Runtime::DecodeBegin => (&[P, types::I64], Some(P)),
            Runtime::DecodeEnd | Runtime::ReadMissing => (&[P, P], None),
            Runtime::DecodeAbandon => (&[P], None),
            Runtime::PathBelow | Runtime::ReadMember => (&[P, P], Some(P)),
            Runtime::ReadObject | Runtime::ReadCase => (&[P, P, P], Some(types::I8)),
            Runtime::ReadNull => (&[P], Some(types::I8)),
            Runtime::ReadInt | Runtime::ReadBool | Runtime::ReadString => {
                (&[P, P, P, P], Some(types::I8))
            }
            Runtime::ReadTag => (&[P, P, P, P], Some(P)),
            Runtime::ReadIs => (&[P, P], Some(types::I8)),
            Runtime::ReadNotACase => (&[P, P, P], None),
            Runtime::ReadInvariant => (&[P, P, P, P, P], None),
        }
    }
}

/// The functions one declaration of one kind is reached through, each declared the first time it
/// is asked for and defined once every entry that asks has been.
#[derive(Default)]
struct Per {
    ids: BTreeMap<String, FuncId>,
    left: Vec<String>,
}

/// Every writer and reader an object holds or reaches, and what of the runtime they call.
pub(crate) struct Codecs {
    call_conv: CallConv,
    imported: BTreeMap<Runtime, FuncId>,
    writers: Per,
    readers: Per,
}

impl Codecs {
    pub(crate) fn new(call_conv: CallConv) -> Self {
        Codecs {
            call_conv,
            imported: BTreeMap::new(),
            writers: Per::default(),
            readers: Per::default(),
        }
    }

    /// The runtime's function, named in the object the first time a function here calls it: an
    /// object that writes and reads nothing names none of them.
    pub(crate) fn runtime(&mut self, module: &mut ObjectModule, called: Runtime) -> FuncId {
        if let Some(&id) = self.imported.get(&called) {
            return id;
        }
        let (takes, answers) = called.signature();
        let mut signature = ir::Signature::new(self.call_conv);
        for &taken in takes {
            signature.params.push(AbiParam::new(taken));
        }
        if let Some(answered) = answers {
            signature.returns.push(AbiParam::new(answered));
        }
        let id = accepted(module.declare_function(called.symbol(), Linkage::Import, &signature));
        crate::index::unique(&mut self.imported, called, id);
        id
    }

    /// A writer's signature: a value of the declaration in, the form it is written as out.
    fn writer_signature(&self) -> ir::Signature {
        let mut signature = ir::Signature::new(self.call_conv);
        signature.params.push(AbiParam::new(POINTER));
        signature.returns.push(AbiParam::new(POINTER));
        signature
    }

    /// A reader's signature, [`reader_symbol`]'s: the node, the path, the reading, and room for
    /// the value, answering a status.
    fn reader_signature(&self) -> ir::Signature {
        let mut signature = ir::Signature::new(self.call_conv);
        for _ in 0..4 {
            signature.params.push(AbiParam::new(POINTER));
        }
        signature.returns.push(AbiParam::new(types::I32));
        signature
    }

    /// The writer of `declared`: this object's own, whoever declared it. A writer reads where a
    /// value keeps what it holds and asks nothing of it that only its build could answer.
    pub(crate) fn writer(&mut self, module: &mut ObjectModule, declared: &str) -> FuncId {
        if let Some(&id) = self.writers.ids.get(declared) {
            return id;
        }
        let id = accepted(module.declare_function(
            &format!("$encode${declared}"),
            Linkage::Local,
            &self.writer_signature(),
        ));
        crate::index::unique(&mut self.writers.ids, declared.to_string(), id);
        self.writers.left.push(declared.to_string());
        id
    }

    /// The reader of `declared`: the one the build that declared it defines, whatever kind of
    /// declaration it is.
    ///
    /// Reading a value's external form is the declaring build's, as building one is: that build
    /// exports its reader and every other reaches it, so a declaration is read one way, by one
    /// function, wherever a document holds a value of it. For a type built from fields this is also
    /// the build running its clauses, which is the only one that can say which of them did not
    /// hold; a unit and a set of alternatives run none and are read there all the same, so what
    /// decides where a reader lives is whose declaration it is and never what the declaration
    /// happens to hold today.
    pub(crate) fn reader(
        &mut self,
        module: &mut ObjectModule,
        declared: &Declared,
        key: &str,
    ) -> FuncId {
        if let Some(&id) = self.readers.ids.get(key) {
            return id;
        }
        let declaration = declared.laid(key);
        let symbol = reader_symbol(declaration.module(), declaration.name());
        let (linkage, defined) = match declaration.by() {
            DeclaredBy::AModule => (Linkage::Export, true),
            DeclaredBy::OnThePath => (Linkage::Import, false),
            DeclaredBy::TheLanguage => unreachable!(
                "`carried` holds that no value of what the language declares is read here"
            ),
        };
        let id = accepted(module.declare_function(&symbol, linkage, &self.reader_signature()));
        crate::index::unique(&mut self.readers.ids, key.to_string(), id);
        if defined {
            self.readers.left.push(key.to_string());
        }
        id
    }

    /// Defines every writer and reader asked for, and every one those ask for in turn.
    pub(crate) fn define(&mut self, emitting: &mut Emitting) -> Lowered<()> {
        loop {
            if let Some(key) = self.writers.left.pop() {
                let id = self.writers.ids[&key];
                let signature = self.writer_signature();
                write::define(emitting, self, id, signature, &key)?;
            } else if let Some(key) = self.readers.left.pop() {
                let id = self.readers.ids[&key];
                let signature = self.reader_signature();
                read::define(emitting, self, id, signature, &key)?;
            } else {
                return Ok(());
            }
        }
    }
}
