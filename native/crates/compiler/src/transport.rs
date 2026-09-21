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
    pub modules: Vec<Module>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub name: String,
    pub behaviors: Vec<Behavior>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Behavior {
    pub name: String,
    pub parameters: Vec<String>,
    pub takes: Vec<Ty>,
    pub answers: Ty,
    pub body: Node,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(tag = "prim", deny_unknown_fields)]
pub enum Ty {
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

impl Ty {
    /// What a reader of a refusal is told this was.
    pub fn spelt(self) -> &'static str {
        match self {
            Ty::Int => "Int",
            Ty::String => "String",
            Ty::Bool => "Bool",
            Ty::Decimal => "Decimal",
            Ty::Rational => "Rational",
            Ty::Date => "Date",
            Ty::Time => "Time",
            Ty::DateTime => "DateTime",
            Ty::Instant => "Instant",
            Ty::Raw => "Raw",
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
    Binary {
        op: Op,
        left: Box<Node>,
        right: Box<Node>,
        #[serde(rename = "type")]
        ty: Ty,
    },
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
