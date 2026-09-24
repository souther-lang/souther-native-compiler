//! What the Java half wrote, read back.
//!
//! These types are the document's shape and not a model of Souther. Nothing here decides anything
//! about what a program means: every question was answered by the checker, and what crosses is the
//! answer. A field added here means a field the writer started writing, not a new thing to work out
//! on this side.
//!
//! The vocabularies are whole. Every operator and every primitive the language has is read here,
//! including the ones no lowering exists for, because what this side cannot read and what it cannot
//! lower are different answers and a reader that knows only what is lowered gives the first when
//! the second is true.
//!
//! A document is read strictly: a field nothing here names is a writer saying something this driver
//! has no idea it was told, and reading past it would be reading a program that means more than
//! what was understood of it.

use serde::Deserialize;

/// What this side reads. A document written to say anything else is refused rather than read as
/// much of as happens to parse.
pub const TRANSPORT_VERSION: u32 = 11;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub transport: u32,
    pub declarations: Vec<Declaration>,
    /// Every behavior the program names, which is wider than what it emits: a body may reach a
    /// behavior a module read off the path declares, and that module is not one of these.
    pub behaviors: Vec<Target>,
    pub modules: Vec<Module>,
}

impl Program {
    /// Every body of `Core` the document holds, with the module it stands in and what owns it.
    ///
    /// The one enumeration of them. Every pass that has to see every body — to find its closure
    /// sites, the published values it calls, whether its types hold together — walks this, so a
    /// body one of them skips is a body all of them skip. `Module` is taken apart whole, so a field
    /// it starts carrying tomorrow does not compile here until it is said whether it holds a body.
    pub fn bodies(&self) -> impl Iterator<Item = Body<'_>> {
        self.modules.iter().flat_map(|written| {
            let Module {
                name,
                helpers,
                values,
                entries,
                definitions,
                examples,
            } = written;
            let module = name.as_str();
            let helpers = helpers.iter().map(move |it| Body {
                module,
                owner: Owner::Helper(it),
                node: &it.body,
            });
            let values = values.iter().map(move |it| Body {
                module,
                owner: Owner::Value(it),
                node: &it.body,
            });
            let entries = entries.iter().map(move |it| Body {
                module,
                owner: Owner::Entry(it),
                node: &it.body,
            });
            let definitions = definitions.iter().filter_map(move |it| match it {
                Definition::Body { declared, body, .. } => Some(Body {
                    module,
                    owner: Owner::Definition(declared),
                    node: body,
                }),
                // Stages reach other behaviors by name, and there is no `Core` of its own.
                Definition::Composed { .. } => None,
            });
            let examples = examples.iter().map(move |it| Body {
                module,
                owner: Owner::Example(it),
                node: &it.body,
            });
            helpers
                .chain(values)
                .chain(entries)
                .chain(definitions)
                .chain(examples)
        })
    }
}

/// One body of `Core`, where it stands, and what owns it.
pub struct Body<'p> {
    /// The module it stands in, whose copy of a helper a call from it reaches.
    pub module: &'p str,
    pub owner: Owner<'p>,
    pub node: &'p Node,
}

/// What a body is the body of.
pub enum Owner<'p> {
    Helper(&'p Held),
    Value(&'p Value),
    Entry(&'p ValueEntry),
    /// A behavior's own body, by the name it defines.
    Definition(&'p str),
    Example(&'p Example),
}

/// Who declared a type, which is what decides who defines the byte its values are tagged with.
///
/// The checker's answer and not one worked out here. This side could ask whether the declaration's
/// module is one the document carries and get the same answer for two of these three, which is the
/// kind of agreement that holds until it does not: a declaration the language itself gives is in
/// no module of any compilation, and the rule would file it under the one case it is not.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DeclaredBy {
    /// A module this compile checked. The declaration is at home in this object, which is what
    /// defines its token for whoever links it.
    AModule,
    /// A module this compile read off the path, already built. That build defined the token and
    /// this object names it, so the linker is what brings the two together.
    OnThePath,
    /// The language, in its own namespace and in no module of any compilation. Nothing here ships
    /// an implementation of one, so there is nothing to be at home in this object either.
    TheLanguage,
}

/// What a declared type is made of, as its declaration says.
///
/// The shape and not the layout: how many fields there are and what they are called. Where a field
/// sits and what a value costs to make are decided here on this side, from this.
///
/// The module and the name apart, because this is where a declared type's identity is owned: the
/// symbol its values are tagged with is built from the two, and a reference elsewhere in the
/// document carries the key that reaches this rather than a second copy of what the key stands
/// for. So nothing on this side ever splits a key back up — an identity comes out of a declaration
/// or it does not come out at all.
#[derive(Debug, Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Declaration {
    Product {
        module: String,
        name: String,
        by: DeclaredBy,
        fields: Vec<Field>,
        /// How many clauses every construction of this type owes. Nothing here checks one, so a
        /// type that states any is one no value can be built of yet.
        invariants: usize,
    },
    /// One value under another name: one field, and not a list of them that happens to hold one.
    Newtype {
        module: String,
        name: String,
        by: DeclaredBy,
        field: Field,
        invariants: usize,
    },
    /// One value, and naming it is that value: no field, and no clause, since there is nothing
    /// for one to observe.
    Unit {
        module: String,
        name: String,
        by: DeclaredBy,
    },
    /// A sum is never built. What it says is which types stand as its cases, and a case may be a
    /// sum again — which is why an arm tests the leaves it resolved to rather than this list.
    ///
    /// No value is ever one, so nothing is ever tagged with a sum and no object defines a token
    /// for one. Which is not to say a sum has no identity: it has the one every declaration has,
    /// its module and its name, and that is here. What it has no need of is a byte for a value to
    /// carry the address of.
    Sum {
        module: String,
        name: String,
        by: DeclaredBy,
        cases: Vec<Case>,
        form: AlternativesForm,
    },
}

impl Declaration {
    pub fn module(&self) -> &str {
        match self {
            Declaration::Product { module, .. }
            | Declaration::Newtype { module, .. }
            | Declaration::Unit { module, .. }
            | Declaration::Sum { module, .. } => module,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Declaration::Product { name, .. }
            | Declaration::Newtype { name, .. }
            | Declaration::Unit { name, .. }
            | Declaration::Sum { name, .. } => name,
        }
    }

    pub fn by(&self) -> DeclaredBy {
        match self {
            Declaration::Product { by, .. }
            | Declaration::Newtype { by, .. }
            | Declaration::Unit { by, .. }
            | Declaration::Sum { by, .. } => *by,
        }
    }

    /// What a reference to this declaration in the document says, which is the two halves joined
    /// the one way.
    pub fn key(&self) -> String {
        format!("{}.{}", self.module(), self.name())
    }

    /// Every field a value of this holds, with what each carries, in the order they are laid
    /// out; none for a unit, and none for a sum, which is never built.
    pub fn fields(&self) -> &[Field] {
        match self {
            Declaration::Product { fields, .. } => fields,
            Declaration::Newtype { field, .. } => std::slice::from_ref(field),
            Declaration::Unit { .. } | Declaration::Sum { .. } => &[],
        }
    }

    /// Where a field of this type sits among its fields, by the name it is declared under.
    pub fn position_of(&self, field: &str) -> Option<usize> {
        self.fields().iter().position(|it| it.name == field)
    }

    pub fn field_count(&self) -> usize {
        self.fields().len()
    }

    /// How many clauses every construction of this type owes.
    pub fn invariants(&self) -> usize {
        match self {
            Declaration::Product { invariants, .. } | Declaration::Newtype { invariants, .. } => {
                *invariants
            }
            Declaration::Unit { .. } | Declaration::Sum { .. } => 0,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub name: String,
    pub helpers: Vec<Held>,
    /// The values this module declares: the one place each of them runs.
    pub values: Vec<Value>,
    /// The entries this module publishes, one per published value — the nullary bridge another
    /// module calls in place of holding a copy of the value (ADR-0074).
    pub entries: Vec<ValueEntry>,
    /// What this object puts under a name. A behavior that answers some other way — supplied from
    /// outside, implemented by another build, or not written — is in the table above and nowhere
    /// here.
    pub definitions: Vec<Definition>,
    /// The `example` rows of this module's behaviors that the object runs.
    pub examples: Vec<Example>,
}

/// A value this module declares: the one place it runs.
///
/// Not a `Held`. A helper is a copy a module carries because a call to it was left standing, and
/// two modules holding one hold a copy each; a value has one executable home, the module that
/// declares it (spec ADR-0074), and this is that home — an object-private definition, reached only
/// from within the declaring module ([`Reaches::Value`]) or through the [`ValueEntry`] a module
/// publishes for it. Its identity crosses split — `module` and `name` apart — not because this
/// home needs a linker-visible name built from the two (it does not: the home is free to be named
/// however this side's own local symbols are), but because [`ValueEntry`]'s `value_symbol(module,
/// name)` does, and a joined spelling here would have to be split back up to answer it.
///
/// What it takes at the language level is nothing: a value takes no argument. `handovers` are not
/// that — they are the machine parameters its generated method actually has, one per other value
/// its root region names, already built by whoever calls it. A language-level "takes none" and a
/// method with parameters are not in tension: the same is true of any other zero-argument
/// definition whose generated method still takes what its body captures.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Value {
    pub module: String,
    pub name: String,
    /// Whether this module publishes it is not carried here: souther's own `CheckedModule`
    /// constructor already holds "published ⇔ has a `ValueEntry` among [`Module::entries`]" as an
    /// invariant, so a `publication` field beside this one would be the same fact stated twice —
    /// and the two would agree only until whichever consumer reads `entries` and whichever reads
    /// `publication` were updated on different days. A value's own publication is asked by looking
    /// it up in `entries`, never by a field here.
    pub handovers: Vec<Handover>,
    pub body: Node,
}

impl Value {
    /// What it answers: its body's type, which is what the checker checked the value as.
    pub fn answers(&self) -> &Ty {
        self.body.ty()
    }

    /// What a reference to this value in the document says, which is the two halves joined the one
    /// way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }
}

/// The entry a module publishes for one of its values: the nullary bridge another module calls in
/// place of holding a copy of the value (ADR-0074).
///
/// Not the value's own body — `body` here is a reference to the value and nothing else, a call
/// reaching [`Reaches::Value`], written the same way any other reach to it is. Present for exactly
/// the values a module publishes; a value it keeps has no entry, because nothing outside the module
/// may call through one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueEntry {
    pub value: ValueRef,
    pub body: Node,
}

/// What the method a value runs as is handed: another value its root region names, already built.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handover {
    pub parameter: String,
    #[serde(rename = "type")]
    pub ty: Ty,
    /// The value this handover carries, split the same way [`Value`]'s own identity is.
    pub carries: ValueRef,
}

/// The module and the name of a value, apart — what a `value_symbol` is built from.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ValueRef {
    pub module: String,
    pub name: String,
}

impl ValueRef {
    /// What a reference to this value in the document says, which is the two halves joined the
    /// one way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }
}

/// What an object defines under a behavior's name: a body of {@link Core}, or a composition of
/// other behaviors.
///
/// Both are local definitions and neither is a body the other can be read as: a composition has no
/// Core to fall back to and a body has no stages. A definition added upstream stops this
/// compiling, for the reason every closed set here does — a third way of defining a name would
/// otherwise arrive as whichever of these two it happened to resemble.
#[derive(Debug, Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Definition {
    /// Written as a `let`: the checker's Core for it.
    Body {
        declared: String,
        parameters: Vec<String>,
        /// What the module declaring it says about the name.
        publication: Publication,
        body: Node,
    },
    /// Written as `>->`: the stages, and what each is offered (spec §type-routing). Carried
    /// whole and not translated into a plan for running it — how the routing between stages is
    /// realised is this side's to decide, and none of it is written down upstream.
    ///
    /// What it answers is its own target's answer and is not carried here a second time.
    Composed {
        declared: String,
        /// What the module declaring it says about the name.
        publication: Publication,
        stages: Vec<Stage>,
    },
}

impl Definition {
    /// What a call reaching this definition writes, which is the two halves joined the one way.
    pub fn declared(&self) -> &str {
        match self {
            Definition::Body { declared, .. } | Definition::Composed { declared, .. } => declared,
        }
    }

    /// What the module declaring it says about the name.
    pub fn publication(&self) -> Publication {
        match self {
            Definition::Body { publication, .. } | Definition::Composed { publication, .. } => {
                *publication
            }
        }
    }
}

/// One stage of a composition: the behavior it applies, and when it is applied to the running
/// value (spec §type-routing).
///
/// What the stage answers is the answer of the behavior it names, which the table of targets
/// carries, and is not carried here a second time.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub behavior: String,
    pub routing: Routing,
}

/// When a stage is applied to the running value (spec §type-routing).
///
/// A closed set and a tagged union for the reason `Selects` is one: `accepted = []` and no routing
/// at all are two different facts, and a `bool` or an `Option<Vec<_>>` would make one of them
/// unrepresentable while inventing a third state nothing upstream ever means.
#[derive(Debug, Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Routing {
    /// The first stage, which takes the composition's own arguments, and any stage whose running
    /// value carries no cases to tell apart.
    Always,
    /// Only where the running value is one of `accepted`. Anything else has left the main line,
    /// and the composition answers with it rather than offering it to what follows.
    OnCases { accepted: Vec<Case> },
}

/// Whether the module that declares a behavior publishes it under that name, or keeps it.
///
/// The language's answer about the module's surface, which is not the same question as what this
/// object's symbol table carries. That one is the object's own, worked out from this together with
/// what the object is for.
///
/// Carried by a body and not by a target, because it is the declaring module's answer and a
/// target is answered for a behavior of a module this compile never checked.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Publication {
    Published,
    Kept,
}

/// One `example` row of a behavior of this module, as the object runs it.
///
/// The values the row states are written into the entry rather than handed to it, so the entry
/// takes nothing and what it does is the one call the row is. Numbered by where the row stands
/// among the behavior's rows, so a row the writer carried nothing for leaves its number unused
/// rather than moving every row after it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Example {
    pub behavior: String,
    pub at: usize,
    pub body: Node,
}

/// A behavior as a caller reaches it.
///
/// The module and the name apart, because that is what a behavior's identity is made of and it is
/// what the symbol is built from. Written as one string and split back, the two halves would be
/// recovered from a spelling rather than carried, and a module's name carries dots.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub module: String,
    pub name: String,
    pub is: Answers,
    pub inputs: Vec<BoundaryInput>,
    pub output: BoundaryOutput,
}

impl Target {
    /// What a call reaching this behavior writes, which is the two halves joined the one way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }

    /// What it takes, read off what each parameter can arrive as. Every boundary shape stands for
    /// a type, so this answers for all of them; whether a value of one can be laid out here is
    /// asked of the type afterwards, and is a different question.
    pub fn takes(&self) -> Vec<Ty> {
        self.inputs.iter().map(BoundaryInput::ty).collect()
    }

    /// What it answers, read off what the answer can leave as.
    pub fn answers(&self) -> Ty {
        self.output.ty()
    }
}

/// One of the closed set of scalars a boundary writes as themselves.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum LeafScalar {
    #[serde(rename = "STRING")]
    String,
    #[serde(rename = "INT")]
    Int,
    #[serde(rename = "BOOL")]
    Bool,
    #[serde(rename = "DECIMAL")]
    Decimal,
    #[serde(rename = "DATE")]
    Date,
    #[serde(rename = "TIME")]
    Time,
    #[serde(rename = "DATETIME")]
    DateTime,
    #[serde(rename = "INSTANT")]
    Instant,
}

impl LeafScalar {
    pub fn prim(self) -> Prim {
        match self {
            LeafScalar::String => Prim::String,
            LeafScalar::Int => Prim::Int,
            LeafScalar::Bool => Prim::Bool,
            LeafScalar::Decimal => Prim::Decimal,
            LeafScalar::Date => Prim::Date,
            LeafScalar::Time => Prim::Time,
            LeafScalar::DateTime => Prim::DateTime,
            LeafScalar::Instant => Prim::Instant,
        }
    }
}

/// What a parameter can arrive as, as the checker settled it.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum BoundaryInput {
    Scalar {
        scalar: LeafScalar,
    },
    Nominal {
        declared: String,
    },
    ListOf {
        element: Box<BoundaryInput>,
    },
    SetOf {
        element: Box<BoundaryInput>,
    },
    MapOf {
        key: MapKey,
        value: Box<BoundaryInput>,
    },
}

impl BoundaryInput {
    pub fn ty(&self) -> Ty {
        match self {
            BoundaryInput::Scalar { scalar } => Ty::Prim {
                prim: scalar.prim(),
            },
            BoundaryInput::Nominal { declared } => Ty::Declared {
                declared: declared.clone(),
            },
            BoundaryInput::ListOf { element } => Ty::List {
                list: Box::new(element.ty()),
            },
            BoundaryInput::SetOf { element } => Ty::Set {
                set: Box::new(element.ty()),
            },
            BoundaryInput::MapOf { key, value } => Ty::Map {
                map: MapTy {
                    key: Box::new(key.ty()),
                    value: Box::new(value.ty()),
                },
            },
        }
    }
}

/// What an answer can leave as, as the checker settled it.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum BoundaryOutput {
    Scalar {
        scalar: LeafScalar,
    },
    Nominal {
        declared: String,
    },
    ListOf {
        element: Box<BoundaryOutput>,
    },
    SetOf {
        element: Box<BoundaryOutput>,
    },
    MapOf {
        key: MapKey,
        value: Box<BoundaryOutput>,
    },
    /// A union nobody named: the type exactly as its members were written, beside the cases the
    /// boundary descended to, which are not the same answer.
    Cases {
        #[serde(rename = "type")]
        ty: Ty,
        cases: Vec<Case>,
        form: AlternativesForm,
    },
}

impl BoundaryOutput {
    pub fn ty(&self) -> Ty {
        match self {
            BoundaryOutput::Scalar { scalar } => Ty::Prim {
                prim: scalar.prim(),
            },
            BoundaryOutput::Nominal { declared } => Ty::Declared {
                declared: declared.clone(),
            },
            BoundaryOutput::ListOf { element } => Ty::List {
                list: Box::new(element.ty()),
            },
            BoundaryOutput::SetOf { element } => Ty::Set {
                set: Box::new(element.ty()),
            },
            BoundaryOutput::MapOf { key, value } => Ty::Map {
                map: MapTy {
                    key: Box::new(key.ty()),
                    value: Box::new(value.ty()),
                },
            },
            BoundaryOutput::Cases { ty, .. } => ty.clone(),
        }
    }
}

/// What a boundary map's key is written as.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum MapKey {
    Text,
    Date,
    Time,
    DateTime,
    Instant,
    NamedKey { declared: String },
}

impl MapKey {
    pub fn ty(&self) -> Ty {
        match self {
            MapKey::Text => Ty::Prim { prim: Prim::String },
            MapKey::Date => Ty::Prim { prim: Prim::Date },
            MapKey::Time => Ty::Prim { prim: Prim::Time },
            MapKey::DateTime => Ty::Prim {
                prim: Prim::DateTime,
            },
            MapKey::Instant => Ty::Prim {
                prim: Prim::Instant,
            },
            MapKey::NamedKey { declared } => Ty::Declared {
                declared: declared.clone(),
            },
        }
    }
}

/// How a set of alternatives travels. Both keys of a discriminated form cross, so nothing on
/// this side spells either of them.
///
/// Read through [`FormOnTheWire`], so a form with one key for both the tag and a wrapped case's
/// contents is not a value here: the two stand in one object, and the checker refuses to build
/// one (`CheckedAlternativesForm.Discriminated`).
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(try_from = "FormOnTheWire")]
pub enum AlternativesForm {
    Enumeration,
    Discriminated { tag: String, contents: String },
}

#[derive(Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
enum FormOnTheWire {
    Enumeration,
    Discriminated { tag: String, contents: String },
}

impl TryFrom<FormOnTheWire> for AlternativesForm {
    type Error = String;

    fn try_from(form: FormOnTheWire) -> Result<Self, Self::Error> {
        match form {
            FormOnTheWire::Enumeration => Ok(AlternativesForm::Enumeration),
            FormOnTheWire::Discriminated { tag, contents } if tag == contents => Err(format!(
                "a discriminated form with {tag} for both its tag and a wrapped case's contents, \
                 which stand in one object"
            )),
            FormOnTheWire::Discriminated { tag, contents } => {
                Ok(AlternativesForm::Discriminated { tag, contents })
            }
        }
    }
}

/// What a field carries across the boundary, as the check derived it for where it stands.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum CodecShape {
    Scalar {
        scalar: LeafScalar,
    },
    Named {
        declared: String,
    },
    ListOf {
        element: Box<CodecShape>,
    },
    SetOf {
        element: Box<CodecShape>,
    },
    MapOf {
        key: MapKey,
        value: Box<CodecShape>,
    },
    /// What an optional holds, which is never an optional again: absence has one form wherever it
    /// stands, so the checker has no shape for an optional of one, and neither does this.
    OptionOf {
        present: Box<Bare>,
    },
}

impl CodecShape {
    /// The type a value standing at this shape is, which is what says how it is held in a slot.
    pub fn ty(&self) -> Ty {
        match self {
            CodecShape::Scalar { scalar } => Ty::Prim {
                prim: scalar.prim(),
            },
            CodecShape::Named { declared } => Ty::Declared {
                declared: declared.clone(),
            },
            CodecShape::ListOf { element } => Ty::List {
                list: Box::new(element.ty()),
            },
            CodecShape::SetOf { element } => Ty::Set {
                set: Box::new(element.ty()),
            },
            CodecShape::MapOf { key, value } => Ty::Map {
                map: MapTy {
                    key: Box::new(key.ty()),
                    value: Box::new(value.ty()),
                },
            },
            CodecShape::OptionOf { present } => Ty::Option {
                option: Box::new(present.shape().ty()),
            },
        }
    }
}

/// A shape that is not an optional: what an optional holds (`CheckedCodecShape.Bare`).
///
/// Read as a [`CodecShape`] and refused if it is an optional, so an optional of an optional never
/// becomes a value here — which is what keeps this side from giving one a form the language never
/// decided.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(try_from = "CodecShape")]
pub struct Bare(CodecShape);

impl Bare {
    pub fn shape(&self) -> &CodecShape {
        &self.0
    }
}

impl TryFrom<CodecShape> for Bare {
    type Error = String;

    fn try_from(shape: CodecShape) -> Result<Self, Self::Error> {
        match shape {
            CodecShape::OptionOf { .. } => Err(
                "an optional holding an optional, which the check never settles: absence has one \
                 form wherever it stands"
                    .to_string(),
            ),
            bare => Ok(Bare(bare)),
        }
    }
}

/// A field of a declaration and what it carries across the boundary, held together.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub codec: CodecShape,
}

/// Which case a name is: one a module declares, a primitive standing as a case, or one the
/// language gives. The identity only — how a case is written is read off what it reaches.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Case {
    Declared { declared: String },
    Primitive { prim: Prim },
    Language { case: LanguageCase },
}

impl Case {
    pub fn spelt(&self) -> String {
        match self {
            Case::Declared { declared } => declared.clone(),
            Case::Primitive { prim } => prim.spelt().to_string(),
            Case::Language { case } => case.spelt().to_string(),
        }
    }
}

/// The cases the language itself gives, a closed set.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum LanguageCase {
    #[serde(rename = "SOME")]
    Some,
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "DIVISION_BY_ZERO")]
    DivisionByZero,
    #[serde(rename = "NOT_A_NUMBER")]
    NotANumber,
    #[serde(rename = "NOT_A_DATE")]
    NotADate,
    #[serde(rename = "NOT_A_TIME")]
    NotATime,
    #[serde(rename = "NOT_WHOLE")]
    NotWhole,
    #[serde(rename = "NOT_A_FINITE_DECIMAL")]
    NotAFiniteDecimal,
}

impl LanguageCase {
    pub fn spelt(self) -> &'static str {
        match self {
            LanguageCase::Some => "Some",
            LanguageCase::None => "None",
            LanguageCase::DivisionByZero => "DivisionByZero",
            LanguageCase::NotANumber => "NotANumber",
            LanguageCase::NotADate => "NotADate",
            LanguageCase::NotATime => "NotATime",
            LanguageCase::NotWhole => "NotWhole",
            LanguageCase::NotAFiniteDecimal => "NotAFiniteDecimal",
        }
    }
}

/// How a behavior comes to answer.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Answers {
    /// Code this object holds, which is emitted.
    Body,
    /// Supplied by whoever runs the program. The object names it and defines nothing for it.
    Injected,
    /// Implemented by another build. The same call to whoever reaches in, and a different thing to
    /// whoever links.
    Elsewhere,
    /// By running other behaviors in an order the composition states.
    Composed,
    /// Not written, which the language admits and nothing can run.
    Unwritten,
}

/// A definition the module holds as one of its own.
///
/// Named by where it was declared, held by the module that reaches it. Two modules reaching one
/// definition hold a copy each.
///
/// What it takes is its parameters, each a name and a type together, and what it answers is its
/// body's type. Neither is carried a second time, so the two cannot disagree.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Held {
    pub declared: String,
    pub parameters: Vec<HeldParameter>,
    pub body: Node,
}

impl Held {
    /// What it takes, in the order its parameters are bound.
    pub fn takes(&self) -> Vec<Ty> {
        self.parameters.iter().map(|it| it.ty.clone()).collect()
    }

    /// What it answers: its body's type.
    pub fn answers(&self) -> &Ty {
        self.body.ty()
    }
}

/// One parameter of a [`Held`], bound under the number its position says.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Ty,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum Prim {
    #[serde(rename = "INT")]
    Int,
    #[serde(rename = "STRING")]
    String,
    #[serde(rename = "BOOL")]
    Bool,
    #[serde(rename = "DECIMAL")]
    Decimal,
    #[serde(rename = "RATIONAL")]
    Rational,
    #[serde(rename = "DATE")]
    Date,
    #[serde(rename = "TIME")]
    Time,
    #[serde(rename = "DATETIME")]
    DateTime,
    #[serde(rename = "INSTANT")]
    Instant,
    #[serde(rename = "RAW")]
    Raw,
}

impl Prim {
    /// What a reader of a refusal is told this was.
    pub fn spelt(self) -> &'static str {
        match self {
            Prim::Int => "Int",
            Prim::String => "String",
            Prim::Bool => "Bool",
            Prim::Decimal => "Decimal",
            Prim::Rational => "Rational",
            Prim::Date => "Date",
            Prim::Time => "Time",
            Prim::DateTime => "DateTime",
            Prim::Instant => "Instant",
            Prim::Raw => "Raw",
        }
    }
}

/// The type the checker decided for something, as much of one as crosses.
///
/// Told apart by which key is written rather than by a word beside it, since each of these is a
/// different shape and no two of them are ever both readable.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged, deny_unknown_fields)]
pub enum Ty {
    Prim {
        prim: Prim,
    },
    /// A declaration of the document, by the key that reaches one. The key is what a reference
    /// says and not what a declaration is made of: the module and the name apart are carried by
    /// the declaration, and this finds it.
    Declared {
        declared: String,
    },
    /// Several cases, any one of which a value here may be. A declared one says which it is, so a
    /// union of those is written nowhere at run time: what holds it is what holds one of them. A
    /// primitive or a case the language gives says nothing of the kind, and a union with one
    /// among its members is read and not laid out.
    Union {
        union: Vec<Case>,
    },
    Option {
        option: Box<Ty>,
    },
    Tuple {
        tuple: Vec<Ty>,
    },
    /// A function value: what it takes and what it answers, nothing about what a value of it is
    /// made of. That is a representation question and this side's own — see `machine_type` and
    /// `means_the_same_elsewhere` in the crate root — not a fact the checker states, so no field
    /// here ever names a capture.
    Fn {
        #[serde(rename = "fn")]
        fn_: FnSignature,
    },
    List {
        list: Box<Ty>,
    },
    Set {
        set: Box<Ty>,
    },
    Map {
        map: MapTy,
    },
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct MapTy {
    pub key: Box<Ty>,
    pub value: Box<Ty>,
}

/// What a function type takes and what it answers, nested under `"fn"` rather than written as
/// `takes`/`answers` siblings of it — the same reason [`Reaches`]'s own shape is nested: a reader
/// telling a function type apart from every other [`Ty`] shape by which key is present must not
/// also have to notice a document naming `fn` beside `option` or `tuple` on the same object.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct FnSignature {
    pub takes: Vec<Ty>,
    pub answers: Box<Ty>,
}

impl Ty {
    pub fn spelt(&self) -> String {
        match self {
            Ty::Prim { prim } => prim.spelt().to_string(),
            Ty::Declared { declared } => declared.clone(),
            Ty::Union { union } => union
                .iter()
                .map(Case::spelt)
                .collect::<Vec<_>>()
                .join(" | "),
            Ty::Option { option } => format!("an Option of {}", option.spelt()),
            Ty::Tuple { tuple } => format!("a tuple of {} members", tuple.len()),
            Ty::Fn { fn_ } => format!(
                "a function taking {} and answering {}",
                fn_.takes
                    .iter()
                    .map(Ty::spelt)
                    .collect::<Vec<_>>()
                    .join(", "),
                fn_.answers.spelt()
            ),
            Ty::List { list } => format!("a List of {}", list.spelt()),
            Ty::Set { set } => format!("a Set of {}", set.spelt()),
            Ty::Map { map } => format!("a Map from {} to {}", map.key.spelt(), map.value.spelt()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "core", rename_all = "lowercase", deny_unknown_fields)]
pub enum Node {
    Int {
        value: i64,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// The number the document knows a binding by, counted where the binder was written.
    Read {
        binding: usize,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Bool {
        value: bool,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// The text a literal spells, already in the form the language keeps it in.
    ///
    /// Normalized to NFC on the way in, by the compiler that read the source. Two canonically
    /// equivalent spellings are one text by Unicode's definition and two by a comparison of code
    /// units, and which of them an editor wrote is not something the author chose — so the folding
    /// belongs where text arrives from outside, and that is not here. Nothing on this side
    /// normalizes, and a driver that did would be folding a second time whatever had already
    /// crossed.
    #[serde(rename = "string")]
    Str {
        value: String,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Binary {
        op: Op,
        left: Box<Node>,
        right: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Neg {
        operand: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A name for a value, and what is written under it. The number is the document's, given where
    /// the binder is written.
    ///
    /// `binds` is what the name is in force at, which every read of it is typed as. It is not
    /// `value`'s type: an annotation, or a sum the value is one case of, binds the name wider than
    /// the value it is given.
    Let {
        binding: usize,
        binds: Ty,
        value: Box<Node>,
        body: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    If {
        cond: Box<Node>,
        then: Box<Node>,
        #[serde(rename = "else")]
        els: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A value of a type with nothing in it. It still says which type it is: that is what it is.
    Unit {
        declared: String,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// Every declared field, in declaration order, which is also the order they are worked out in.
    Construct {
        declared: String,
        values: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Field {
        target: Box<Node>,
        field: String,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Match {
        subject: Box<Node>,
        arms: Vec<Arm>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Some {
        value: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    None {
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Tuple {
        members: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    Member {
        tuple: Box<Node>,
        at: usize,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A call, and what the checker settled it reaches.
    Call {
        reaches: Reaches,
        arguments: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A function value: its own parameters, and the body they are bound in — written whole and
    /// not closure-converted on the wire. What of the body's free bindings this side has to carry
    /// forward as runtime state, and how, is this driver's own representation question; see
    /// `closures` in the crate root.
    ///
    /// `site` is this document's own number for where the block stands, minted by `ProgramWriter`
    /// so a lifted function can be declared under it before this side has decided anything about
    /// what that function closes over — the same role a binding's number plays for a read, and
    /// counted the same way: document-wide, where the block is written.
    Block {
        site: usize,
        parameters: Vec<Parameter>,
        body: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A function value applied to arguments — a value the body holds, and not a call to something
    /// declared elsewhere ([`Node::Call`]'s own `reaches`). `function` is a [`Node::Read`] every
    /// time souther's checker builds one (`Core.Apply`'s own contract), read the same way any
    /// other operand's is rather than reduced to a binding number bare beside `arguments`.
    Apply {
        function: Box<Node>,
        arguments: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
    /// A value standing as a type other than its own, where the checker decided that it may.
    ///
    /// `value` is what is evaluated, at the type it was worked out at, and `ty` is what the position
    /// it stands in takes it as. Written only where the two differ, so every other position holds a
    /// value of exactly the type it takes. Why the checker let it stand there is not carried, and
    /// nothing here works it out again.
    Widen {
        value: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
        aborts: Vec<AbortKind>,
    },
}

/// One parameter of a [`Node::Block`], numbered the way any other binder on the wire is: where it
/// is written, by `ProgramWriter`'s own counter.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub binding: usize,
    pub name: String,
}

/// What a call reaches, which the checker decided and nothing here works out again.
///
/// A nested object — `{"is":"behavior","declared":"..."}` — and not `declared`/`kernel` beside
/// `reaches` as siblings of `Node::Call`'s own fields: a document naming `kernel` beside
/// `is:"behavior"`, or naming `is:"kernel"` with no `kernel` at all, does not parse as one of
/// these rather than parsing and leaving `lower` to find out with an `.expect()` at the one place
/// it is read. (An adjacently-nested object rather than `#[serde(flatten)]` on a sibling of
/// `Node::Call`'s own fields, deliberately — `flatten` inside a `Node` whose own variants are
/// chosen by an internal tag (`"core"`) asks serde to buffer the same map twice over, which it
/// does not support and fails at every call site rather than only the ambiguous ones.) `kernel`
/// itself crosses as the key the standard library declares it under (`"int.add"`) and not as a
/// closed enum here: which kernels exist is the language's question and this side's only question
/// is which of them it can lower, answered by `NotLowered` at the one place that tries, not by a
/// vocabulary this file would have to keep in step with every kernel the language ever adds.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Reaches {
    /// A definition the calling module holds, which is a copy of its own.
    Helper { declared: String },
    /// A value that runs where it is declared, and this module is that module: an ordinary call to
    /// the method this object runs the value as, the same call a helper's own reach is (souther's
    /// JVM backend calls it through the identical path a recursive helper's is — `BodyGen`'s
    /// `recursiveHelperCall`). "Runs once" is `ADR-0074`'s checker-level guarantee that the region
    /// reading a value's reference builds each of its dependencies once, threaded through
    /// [`Handover`] — not a runtime cache this side has to keep; nothing here memoizes a call's
    /// answer. Split identity, and not `declared` joined the way [`Helper`](Reaches::Helper)'s and
    /// [`Behavior`](Reaches::Behavior)'s are, because the *entry* a module publishes for this value
    /// needs its module and name apart to build `value_symbol` from — carried apart here too, so a
    /// lowering never has to split a joined spelling back up to answer "which module declares
    /// this."
    Value { module: String, name: String },
    /// A value another module declares, reached through the entry that module publishes for it —
    /// never a method of the emitting module. Its own tag and not [`Value`](Reaches::Value): what
    /// answers the call is `value_symbol(module, name)`, an entry across an object boundary, where
    /// [`Value`](Reaches::Value) is a call within this object to a method reached the way a helper
    /// is — a caller emitting one must not have to tell them apart by re-deriving whether `module`
    /// is its own.
    PublishedValue { module: String, name: String },
    /// A behavior, whether this program answers it or whoever links the object does.
    Behavior { declared: String },
    /// An operation the language itself implements.
    Kernel { kernel: String },
}

/// One arm of a fork on what a value is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    pub selects: Vec<Selects>,
    /// The number the body reads the value under, where the arm binds it at all.
    pub binding: Option<usize>,
    /// What the value is read as inside the arm.
    ///
    /// Carried rather than worked out from what the arm tests, because the test does not say it:
    /// an optional's present carrier is tested the same way whatever it holds, so a reader that
    /// took the type from the test would read every optional's value at one width.
    pub binds: Option<Ty>,
    pub body: Node,
}

/// What one case of an arm tests for.
#[derive(Debug, Deserialize)]
#[serde(tag = "tests", rename_all = "lowercase", deny_unknown_fields)]
pub enum Selects {
    /// The value's own type is one of these. The atoms are the leaves the checker resolved the
    /// case to, so a case that is a sum arrives as the several types it stands for — each as the
    /// case identity it is, and a declared one by the key that reaches its declaration.
    Which {
        atoms: Vec<Case>,
    },
    Held,
    Nothing,
}

impl Node {
    /// The nodes directly under this one, in the order they are written.
    ///
    /// No arm standing for the rest: a node added to the document is one whose children every walk
    /// over this would otherwise silently never reach.
    pub fn children(&self) -> Vec<&Node> {
        match self {
            Node::Binary { left, right, .. } => vec![left, right],
            Node::Neg { operand, .. } => vec![operand],
            Node::Let { value, body, .. } => vec![value, body],
            Node::If {
                cond, then, els, ..
            } => vec![cond, then, els],
            Node::Construct { values, .. } => values.iter().collect(),
            Node::Field { target, .. } => vec![target],
            Node::Match { subject, arms, .. } => std::iter::once(subject.as_ref())
                .chain(arms.iter().map(|arm| &arm.body))
                .collect(),
            Node::Some { value, .. } | Node::Widen { value, .. } => vec![value],
            Node::Tuple { members, .. } => members.iter().collect(),
            Node::Member { tuple, .. } => vec![tuple],
            Node::Call { arguments, .. } => arguments.iter().collect(),
            Node::Block { body, .. } => vec![body],
            Node::Apply {
                function,
                arguments,
                ..
            } => std::iter::once(function.as_ref())
                .chain(arguments.iter())
                .collect(),
            Node::Int { .. }
            | Node::Read { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Unit { .. }
            | Node::None { .. } => Vec::new(),
        }
    }

    /// The type the checker decided for this expression.
    ///
    /// Read off the node rather than worked out from where it sits: what a comparison compares is
    /// not what a comparison answers, and a lowering that took the second for the first would
    /// compare two values at the width of the answer.
    pub fn ty(&self) -> &Ty {
        match self {
            Node::Int { ty, .. }
            | Node::Read { ty, .. }
            | Node::Bool { ty, .. }
            | Node::Str { ty, .. }
            | Node::Binary { ty, .. }
            | Node::Neg { ty, .. }
            | Node::Let { ty, .. }
            | Node::If { ty, .. }
            | Node::Unit { ty, .. }
            | Node::Construct { ty, .. }
            | Node::Field { ty, .. }
            | Node::Match { ty, .. }
            | Node::Some { ty, .. }
            | Node::None { ty, .. }
            | Node::Tuple { ty, .. }
            | Node::Member { ty, .. }
            | Node::Call { ty, .. }
            | Node::Block { ty, .. }
            | Node::Apply { ty, .. }
            | Node::Widen { ty, .. } => ty,
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum Op {
    #[serde(rename = "EQ")]
    Eq,
    #[serde(rename = "NE")]
    Ne,
    #[serde(rename = "LT")]
    Lt,
    #[serde(rename = "LE")]
    Le,
    #[serde(rename = "GT")]
    Gt,
    #[serde(rename = "GE")]
    Ge,
    #[serde(rename = "AND")]
    And,
    #[serde(rename = "OR")]
    Or,
    #[serde(rename = "ADD")]
    Add,
    #[serde(rename = "SUB")]
    Sub,
    #[serde(rename = "MUL")]
    Mul,
    #[serde(rename = "DIV")]
    Div,
    #[serde(rename = "CONCAT")]
    Concat,
}

impl Op {
    /// What a reader of a refusal is told this was.
    pub fn spelt(self) -> &'static str {
        match self {
            Op::Eq => "==",
            Op::Ne => "/=",
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::And => "&&",
            Op::Or => "||",
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Concat => "++",
        }
    }
}

/// A way a Souther computation ends without a value, named by the reason the language gives for
/// it — the checker's own closed set, spelt on the wire the same way `Op` and `Prim` are.
///
/// This is not this side's to reclassify. A site's `Node::aborts` is `program.abortsAt(site)`
/// read off the checker, so which member goes with which machine condition is answered once, by
/// whoever maps a member of this to `souther_native_abi`'s own encoding — never re-derived from
/// what a node looks like here.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum AbortKind {
    #[serde(rename = "INVARIANT_NOT_HELD")]
    InvariantNotHeld,
    #[serde(rename = "ENSURES_NOT_HELD")]
    EnsuresNotHeld,
    #[serde(rename = "UNREACHABLE_REACHED")]
    UnreachableReached,
    #[serde(rename = "DIVISION_BY_ZERO")]
    DivisionByZero,
    #[serde(rename = "REQUIRED_FORM_HAS_NO_PLACE")]
    RequiredFormHasNoPlace,
    #[serde(rename = "INVALID_BOUNDS")]
    InvalidBounds,
}
