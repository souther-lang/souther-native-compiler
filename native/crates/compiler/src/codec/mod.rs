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
//! One writer's step and one reader per declaration, each a function of this object's own and
//! compiled from the declaration, so nothing about a declaration is carried to run time for the
//! runtime to interpret. What the runtime is asked is only about one place of a document or one node
//! of the tree: is it an object, what is under this key, is it an `Int`. A step does not call the
//! step of what a value holds; it leaves that to a walk ([`write`]), so how deep a value is written
//! is not bounded by the native stack. A reader still calls the reader of what a field holds.
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
    DECODE_ABANDON, DECODE_BEGIN, DECODE_END, DECODE_HOST_BEGIN, DECODE_ROOT, EXTERNAL_APPEND,
    EXTERNAL_ARRAY, EXTERNAL_BOOL, EXTERNAL_DECIMAL, EXTERNAL_INT, EXTERNAL_JSON, EXTERNAL_NULL,
    EXTERNAL_OBJECT, EXTERNAL_PUT, EXTERNAL_STRING, PATH_AT, PATH_BELOW, READ_ARRAY,
    READ_ARRAY_LENGTH, READ_BOOL, READ_CASE, READ_DECIMAL, READ_ELEMENT, READ_INT, READ_INVARIANT,
    READ_IS, READ_MEMBER, READ_MISSING, READ_NOT_A_CASE, READ_NULL, READ_OBJECT, READ_STRING,
    READ_TAG, reader_symbol,
};
use std::collections::{BTreeMap, BTreeSet};
use write::{Continuation, Driver, Element, Work};

/// Every declaration whose values have an external form this backend reads and writes, by the key a
/// reference to it says.
///
/// A scalar it holds is an `Int`, a `Bool` or a `String`; a declared type it holds is one of these
/// again, and so is what a list it holds holds; a set of alternatives is made of cases that are
/// each one of these, a primitive among those scalars, or a case the language gives, which is
/// written as its name alone. What the language declares is none: no build defines its token, so
/// no value of it is made here.
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
        Declaration::Sum { cases, .. } => {
            let mut reached = Vec::new();
            for case in cases {
                match case {
                    Case::Declared { declared } => reached.push(Some(declared.as_str())),
                    // Written under the set's contents key the way a field of it would be.
                    Case::Primitive { prim } => primitive_reaches(*prim, &mut reached),
                    // The tag alone.
                    Case::Language { .. } => {}
                }
            }
            reached
        }
    }
}

/// Nothing, for a primitive with an external form here, and something with none for the rest.
fn primitive_reaches(prim: Prim, reached: &mut Vec<Option<&str>>) {
    match prim {
        Prim::Int | Prim::Bool | Prim::String | Prim::Decimal => {}
        Prim::Rational | Prim::Date | Prim::Time | Prim::DateTime | Prim::Instant | Prim::Raw => {
            reached.push(None)
        }
    }
}

fn shape_reaches<'s>(shape: &'s CodecShape, reached: &mut Vec<Option<&'s str>>) {
    match shape {
        CodecShape::Scalar { scalar } => primitive_reaches(scalar.prim(), reached),
        // A primitive or a case the language gives, named as a field's type on its own, has no
        // codec of its own designed here (`named_as_a_type`).
        CodecShape::Named { named } => reached.push(named.declared()),
        CodecShape::OptionOf { present } => shape_reaches(present.shape(), reached),
        // An array of its elements, each written as one would be anywhere else.
        CodecShape::ListOf { element } => shape_reaches(element, reached),
        CodecShape::SetOf { .. } | CodecShape::MapOf { .. } => reached.push(None),
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
    ExternalDecimal,
    ExternalArray,
    ExternalAppend,
    ExternalObject,
    ExternalPut,
    ExternalJson,
    DecodeBegin,
    DecodeHostBegin,
    DecodeRoot,
    DecodeEnd,
    DecodeAbandon,
    PathBelow,
    PathAt,
    ReadArray,
    ReadArrayLength,
    ReadElement,
    ReadObject,
    ReadMember,
    ReadMissing,
    ReadNull,
    ReadInt,
    ReadBool,
    ReadString,
    ReadDecimal,
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
            Runtime::ExternalDecimal => EXTERNAL_DECIMAL,
            Runtime::ExternalArray => EXTERNAL_ARRAY,
            Runtime::ExternalAppend => EXTERNAL_APPEND,
            Runtime::ExternalObject => EXTERNAL_OBJECT,
            Runtime::ExternalPut => EXTERNAL_PUT,
            Runtime::ExternalJson => EXTERNAL_JSON,
            Runtime::DecodeBegin => DECODE_BEGIN,
            Runtime::DecodeHostBegin => DECODE_HOST_BEGIN,
            Runtime::DecodeRoot => DECODE_ROOT,
            Runtime::DecodeEnd => DECODE_END,
            Runtime::DecodeAbandon => DECODE_ABANDON,
            Runtime::PathBelow => PATH_BELOW,
            Runtime::PathAt => PATH_AT,
            Runtime::ReadArray => READ_ARRAY,
            Runtime::ReadArrayLength => READ_ARRAY_LENGTH,
            Runtime::ReadElement => READ_ELEMENT,
            Runtime::ReadObject => READ_OBJECT,
            Runtime::ReadMember => READ_MEMBER,
            Runtime::ReadMissing => READ_MISSING,
            Runtime::ReadNull => READ_NULL,
            Runtime::ReadInt => READ_INT,
            Runtime::ReadBool => READ_BOOL,
            Runtime::ReadString => READ_STRING,
            Runtime::ReadDecimal => READ_DECIMAL,
            Runtime::ReadCase => READ_CASE,
            Runtime::ReadTag => READ_TAG,
            Runtime::ReadIs => READ_IS,
            Runtime::ReadNotACase => READ_NOT_A_CASE,
            Runtime::ReadInvariant => READ_INVARIANT,
        }
    }
}

/// The functions one kind of thing is reached through, each declared the first time it is asked
/// for and defined once every entry that asks has been.
struct Per<K, F = FuncId> {
    ids: BTreeMap<K, F>,
    left: Vec<K>,
}

impl<K, F> Default for Per<K, F> {
    fn default() -> Self {
        Per {
            ids: BTreeMap::new(),
            left: Vec::new(),
        }
    }
}

/// Every writer's step and reader an object holds or reaches, the functions a walk that writes is
/// made of, and what of the runtime they call.
pub(crate) struct Codecs {
    call_conv: CallConv,
    imported: BTreeMap<Runtime, FuncId>,
    steps: Per<String, Work>,
    continuations: Per<Continuation, Work>,
    drivers: Per<Driver>,
    /// The work writing each list whose elements wait, one for every place such a list is written,
    /// with what its elements are written as, left to define.
    each: Vec<(Work, Element)>,
    eaches: usize,
    readers: Per<String>,
}

impl Codecs {
    pub(crate) fn new(call_conv: CallConv) -> Self {
        Codecs {
            call_conv,
            imported: BTreeMap::new(),
            steps: Per::default(),
            continuations: Per::default(),
            drivers: Per::default(),
            each: Vec::new(),
            eaches: 0,
            readers: Per::default(),
        }
    }

    /// The runtime's function, named in the object the first time a function here calls it: an
    /// object that writes and reads nothing names none of them.
    pub(crate) fn runtime(&mut self, module: &mut ObjectModule, called: Runtime) -> FuncId {
        if let Some(&id) = self.imported.get(&called) {
            return id;
        }
        let id = crate::import_runtime(module, called.symbol(), self.call_conv);
        crate::index::unique(&mut self.imported, called, id);
        id
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

    /// The step of `declared`'s writer: this object's own, whoever declared it. A step reads where
    /// a value keeps what it holds and asks nothing of it that only its build could answer.
    fn step(&mut self, module: &mut ObjectModule, declared: &str) -> Work {
        if let Some(&work) = self.steps.ids.get(declared) {
            return work;
        }
        let work = write::declare_work(module, self.call_conv, &format!("$encode${declared}"));
        crate::index::unique(&mut self.steps.ids, declared.to_string(), work);
        self.steps.left.push(declared.to_string());
        work
    }

    /// One of the continuations a walk that writes is made of, declared the first time a function
    /// here reaches it: an object that writes nothing holds none of them.
    fn continuation(&mut self, module: &mut ObjectModule, part: Continuation) -> Work {
        if let Some(&work) = self.continuations.ids.get(&part) {
            return work;
        }
        let work = write::declare_work(module, self.call_conv, part.symbol());
        crate::index::unique(&mut self.continuations.ids, part, work);
        self.continuations.left.push(part);
        work
    }

    /// One of the functions a walk is driven by, declared the first time a function here reaches it.
    fn driver(&mut self, module: &mut ObjectModule, part: Driver) -> FuncId {
        if let Some(&id) = self.drivers.ids.get(&part) {
            return id;
        }
        let id = accepted(module.declare_function(
            part.symbol(),
            Linkage::Local,
            &part.signature(self.call_conv),
        ));
        crate::index::unique(&mut self.drivers.ids, part, id);
        self.drivers.left.push(part);
        id
    }

    /// The work writing the elements of one list whose elements wait, written as `element`: a
    /// function of its own for every place such a list is written, since what it adds for an
    /// element is compiled from where the list stands.
    fn each(&mut self, module: &mut ObjectModule, element: Element) -> Work {
        let work = write::declare_work(
            module,
            self.call_conv,
            &format!("$encoding$each${}", self.eaches),
        );
        self.eaches += 1;
        self.each.push((work, element));
        work
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

    /// Defines every step, part of a walk and reader asked for, and every one those ask for in
    /// turn.
    pub(crate) fn define(&mut self, emitting: &mut Emitting) -> Lowered<()> {
        loop {
            if let Some(key) = self.steps.left.pop() {
                let work = self.steps.ids[&key];
                write::define(emitting, self, work, &key)?;
            } else if let Some((work, element)) = self.each.pop() {
                write::define_each(emitting, self, work, &element)?;
            } else if let Some(part) = self.continuations.left.pop() {
                let work = self.continuations.ids[&part];
                write::define_continuation(emitting, self, work, part)?;
            } else if let Some(part) = self.drivers.left.pop() {
                let id = self.drivers.ids[&part];
                write::define_driver(emitting, id, part)?;
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
