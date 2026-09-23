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
pub const TRANSPORT_VERSION: u32 = 7;

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
        fields: Vec<String>,
        /// How many clauses every construction of this type owes. Nothing here checks one, so a
        /// type that states any is one no value can be built of yet.
        invariants: usize,
    },
    Newtype {
        module: String,
        name: String,
        by: DeclaredBy,
        fields: Vec<String>,
        invariants: usize,
    },
    Unit {
        module: String,
        name: String,
        by: DeclaredBy,
        fields: Vec<String>,
        invariants: usize,
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
        cases: Vec<String>,
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

    /// Where a field of this type sits among its fields, by the name it is declared under.
    pub fn position_of(&self, field: &str) -> Option<usize> {
        match self {
            Declaration::Product { fields, .. }
            | Declaration::Newtype { fields, .. }
            | Declaration::Unit { fields, .. } => fields.iter().position(|it| it == field),
            Declaration::Sum { .. } => None,
        }
    }

    pub fn field_count(&self) -> usize {
        match self {
            Declaration::Product { fields, .. }
            | Declaration::Newtype { fields, .. }
            | Declaration::Unit { fields, .. } => fields.len(),
            Declaration::Sum { .. } => 0,
        }
    }

    /// How many clauses every construction of this type owes.
    pub fn invariants(&self) -> usize {
        match self {
            Declaration::Product { invariants, .. }
            | Declaration::Newtype { invariants, .. }
            | Declaration::Unit { invariants, .. } => *invariants,
            Declaration::Sum { .. } => 0,
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
/// declares it (spec ADR-0074), and this is that home. Its identity crosses split — `module` and
/// `name` apart, the way a behavior's [`Target`] does and a helper's `declared` does not —
/// because `value_symbol(module, name)` is built from the two, and a joined spelling would have
/// to be split back up to get there, which is a decision this side is not handed to make.
///
/// What its method is handed is not an argument: a value takes none. `handovers` is what its root
/// region demands — other values this one names, built already — so nothing here has to be built
/// twice.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Value {
    pub module: String,
    pub name: String,
    /// Whether the module declaring it publishes it, or keeps it — read off the module's surface,
    /// the same as a behavior's, and not a fact of the value itself.
    pub publication: Publication,
    pub handovers: Vec<Handover>,
    pub answers: Ty,
    pub body: Node,
}

impl Value {
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
    Composed {
        declared: String,
        /// What the module declaring it says about the name.
        publication: Publication,
        stages: Vec<Stage>,
        answers: Ty,
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

/// One stage of a composition: the behavior it applies, what that behavior answers, and when it
/// is applied to the running value (spec §type-routing).
///
/// `answers` is the stage's own output and not the running value after it: the two differ exactly
/// where the stage was offered part of what was running, and what leaves the main line is not
/// offered to what follows.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub behavior: String,
    pub answers: Ty,
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
    OnCases { accepted: Vec<String> },
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
    pub takes: Vec<Ty>,
    pub answers: Ty,
}

impl Target {
    /// What a call reaching this behavior writes, which is the two halves joined the one way.
    pub fn declared(&self) -> String {
        format!("{}.{}", self.module, self.name)
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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Held {
    pub declared: String,
    pub parameters: Vec<String>,
    pub takes: Vec<Ty>,
    pub answers: Ty,
    pub body: Node,
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
    Prim { prim: Prim },
    /// A declaration of the document, by the key that reaches one. The key is what a reference
    /// says and not what a declaration is made of: the module and the name apart are carried by
    /// the declaration, and this finds it.
    Declared { declared: String },
    /// Several declared types, any one of which a value here may be. Each of them says which type
    /// it is, so a union is written nowhere at run time: what holds it is what holds one of them.
    Union { union: Vec<String> },
    Option { option: Box<Ty> },
    Tuple { tuple: Vec<Ty> },
}

impl Ty {
    pub fn spelt(&self) -> String {
        match self {
            Ty::Prim { prim } => prim.spelt().to_string(),
            Ty::Declared { declared } => declared.clone(),
            Ty::Union { union } => union.join(" | "),
            Ty::Option { option } => format!("an Option of {}", option.spelt()),
            Ty::Tuple { tuple } => format!("a tuple of {} members", tuple.len()),
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
    Let {
        binding: usize,
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
    /// A value that runs where it is declared, and this module is that module: a call reaching the
    /// method this object runs the value as. Split identity, and not `declared` joined the way
    /// [`Helper`](Reaches::Helper)'s and [`Behavior`](Reaches::Behavior)'s are — `value_symbol` is
    /// built from the two apart, and this side must not split a joined spelling back up to get
    /// there.
    Value { module: String, name: String },
    /// A value another module declares, reached through the entry that module publishes for it —
    /// never a method of the emitting module. Its own tag and not [`Value`](Reaches::Value): the
    /// two are different runtime semantics (one runs here, once, in this object; the other calls
    /// out to whoever the declaring module's object is), and collapsing them here would hand the
    /// half that lowers a call it cannot tell apart without asking again what only the checker
    /// already knew.
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
    /// case to, so a case that is a sum arrives as the several types it stands for — and each of
    /// them by the key that reaches its declaration, which is where its identity is.
    Which { atoms: Vec<String> },
    Held,
    Nothing,
}

impl Node {
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
            | Node::Call { ty, .. } => ty,
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
