//! What the Java half wrote, read back.
//!
//! These types are the document's shape and not a model of Souther. Nothing here decides anything
//! about what a program means: every question was answered by the checker, and what crosses is the
//! answer. A field added here means a field the writer started writing, not a new thing to work out
//! on this side.

use serde::Deserialize;

/// What this side reads. A document written to say anything else is refused rather than read as
/// much of as happens to parse.
pub const TRANSPORT_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct Program {
    pub transport: u32,
    pub modules: Vec<Module>,
}

#[derive(Debug, Deserialize)]
pub struct Module {
    pub name: String,
    pub behaviors: Vec<Behavior>,
}

#[derive(Debug, Deserialize)]
pub struct Behavior {
    pub name: String,
    pub parameters: Vec<String>,
    pub takes: Vec<Ty>,
    pub answers: Ty,
    pub body: Node,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(tag = "prim")]
pub enum Ty {
    #[serde(rename = "INT")]
    Int,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "core", rename_all = "lowercase")]
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
    #[serde(rename = "ADD")]
    Add,
}
