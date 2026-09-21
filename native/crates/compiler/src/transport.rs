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
pub const TRANSPORT_VERSION: u32 = 1;

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

/// What a declared type is made of, as its declaration says.
///
/// The shape and not the layout: how many fields there are and what they are called. Where a field
/// sits and what a value costs to make are decided here on this side, from this.
#[derive(Debug, Deserialize)]
#[serde(tag = "is", rename_all = "lowercase", deny_unknown_fields)]
pub enum Declaration {
    Product {
        declared: String,
        fields: Vec<String>,
        /// How many clauses every construction of this type owes. Nothing here checks one, so a
        /// type that states any is one no value can be built of yet.
        invariants: usize,
    },
    Newtype {
        declared: String,
        fields: Vec<String>,
        invariants: usize,
    },
    Unit {
        declared: String,
        fields: Vec<String>,
        invariants: usize,
    },
    /// A sum is never built. What it says is which types stand as its cases, and a case may be a
    /// sum again — which is why an arm tests the leaves it resolved to rather than this list.
    Sum {
        declared: String,
        cases: Vec<String>,
    },
}

impl Declaration {
    pub fn declared(&self) -> &str {
        match self {
            Declaration::Product { declared, .. }
            | Declaration::Newtype { declared, .. }
            | Declaration::Unit { declared, .. }
            | Declaration::Sum { declared, .. } => declared,
        }
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
    /// What this object puts under a name. A behavior that answers some other way is in the table
    /// above and nowhere here.
    pub bodies: Vec<Body>,
}

/// A behavior's body, under the name the table of targets knows it by.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub declared: String,
    pub parameters: Vec<String>,
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
    },
    /// The number the document knows a binding by, counted where the binder was written.
    Read {
        binding: usize,
        #[serde(rename = "type")]
        ty: Ty,
    },
    Bool {
        value: bool,
        #[serde(rename = "type")]
        ty: Ty,
    },
    Binary {
        op: Op,
        left: Box<Node>,
        right: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
    Neg {
        operand: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
    /// A name for a value, and what is written under it. The number is the document's, given where
    /// the binder is written.
    Let {
        binding: usize,
        value: Box<Node>,
        body: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
    If {
        cond: Box<Node>,
        then: Box<Node>,
        #[serde(rename = "else")]
        els: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
    /// A value of a type with nothing in it. It still says which type it is: that is what it is.
    Unit {
        declared: String,
        #[serde(rename = "type")]
        ty: Ty,
    },
    /// Every declared field, in declaration order, which is also the order they are worked out in.
    Construct {
        declared: String,
        values: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
    Field {
        target: Box<Node>,
        field: String,
        #[serde(rename = "type")]
        ty: Ty,
    },
    Match {
        subject: Box<Node>,
        arms: Vec<Arm>,
        #[serde(rename = "type")]
        ty: Ty,
    },
    Some {
        value: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
    None {
        #[serde(rename = "type")]
        ty: Ty,
    },
    Tuple {
        members: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
    Member {
        tuple: Box<Node>,
        at: usize,
        #[serde(rename = "type")]
        ty: Ty,
    },
    /// A call, and what the checker settled it reaches.
    Call {
        reaches: Reaches,
        declared: String,
        arguments: Vec<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
}

/// What a call reaches, which the checker decided and nothing here works out again.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Reaches {
    /// A definition the calling module holds, which is a copy of its own.
    Helper,
    /// A value that runs in the module that declares it, wherever it is named.
    Value,
    /// A behavior, whether this program answers it or whoever links the object does.
    Behavior,
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
    /// case to, so a case that is a sum arrives as the several types it stands for.
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
