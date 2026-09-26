//! What a body writes and nothing runs: a function handed to a call that never applies it.
//!
//! A body either runs or does not (`Runs::runs`), and inside one that runs this is the only code
//! that does not. It is upstream's `Core.Call.functionArgument` answering `NEVER_APPLIED`,
//! transcribed, and widened by one kind of call: a call that is not a kernel's, or is one of a
//! kernel this backend lowers, handed a function one of whose parameters is the type of what has
//! no value (`Core.neverRuns`). No value of that type is ever made, so nothing can be handed to
//! that parameter and the function is never applied. The unit is the argument, whatever it is
//! written as: the `let`s the checker binds around a block are part of it, and upstream emits
//! "none of it — no class, no body".
//!
//! The step of a walk over an empty list literal is the case that happens: `List.map(f, [])`
//! walks with a step taking a `Nothing` for an element. A walk hands its step nothing in its place
//! and answers an empty list. So does a kernel this backend lowers: it applies a function it is
//! handed only to what the list or the optional beside it holds, which is nothing where the
//! function takes what has no value, so `List.sortBy(f, [])` answers the empty list it was handed
//! and calls nothing. Upstream hands a kernel every function because its runtime is handed the
//! function as a value; here a kernel is emitted where it is called, and nothing is handed over.
//! Any other call would have to hand the function it never applies to a copy that takes it, which
//! is a function taking what has no value; that is refused as not lowered where the call is read
//! (`Coherent`), and nothing of the function is lowered either way.
//!
//! Every pass asking what an object runs, reaches or has to lower asks it through this: by
//! [`each_lowered`], by [`lowered_children`] and [`lowered_children_mut`], or by
//! [`never_applied`] where it walks a call itself. Whether the document is coherent is asked of
//! all of it, run or not.

use crate::kernels::LoweredKernel;
use crate::transport::{Node, Reaches, Ty};

/// Where among `node`'s arguments the functions it never applies stand, where `node` is a call
/// that is not a kernel's. Empty for every other node.
///
/// Asked of each argument's own type, as upstream asks it, and not of what the argument is
/// written as.
pub(crate) fn never_applied(node: &Node) -> Vec<usize> {
    let Node::Call {
        reaches, arguments, ..
    } = node
    else {
        return Vec::new();
    };
    match reaches {
        // A kernel this backend does not lower is refused whatever it is handed.
        Reaches::Kernel { kernel, .. } if LoweredKernel::of(kernel).is_none() => Vec::new(),
        Reaches::Kernel { .. }
        | Reaches::Emitted { .. }
        | Reaches::Helper { .. }
        | Reaches::Value { .. }
        | Reaches::PublishedValue { .. }
        | Reaches::Behavior { .. } => arguments
            .iter()
            .enumerate()
            .filter(|(_, argument)| takes_what_has_no_value(argument.ty()))
            .map(|(at, _)| at)
            .collect(),
    }
}

/// Whether `ty` is a function one of whose parameters is the type of what has no value.
fn takes_what_has_no_value(ty: &Ty) -> bool {
    matches!(ty, Ty::Fn { fn_ } if fn_.takes.iter().any(|taken| matches!(taken, Ty::Nothing { .. })))
}

/// `node`'s children, in the order they are written, less the functions it never applies.
pub(crate) fn lowered_children(node: &Node) -> Vec<&Node> {
    let unrun = never_applied(node);
    match node {
        Node::Call { arguments, .. } if !unrun.is_empty() => arguments
            .iter()
            .enumerate()
            .filter(|(at, _)| !unrun.contains(at))
            .map(|(_, argument)| argument)
            .collect(),
        _ => node.children(),
    }
}

/// The same children, to be rewritten in place.
pub(crate) fn lowered_children_mut(node: &mut Node) -> Vec<&mut Node> {
    let unrun = never_applied(node);
    if unrun.is_empty() {
        return node.children_mut();
    }
    let Node::Call { arguments, .. } = node else {
        unreachable!("only a call is handed a function it never applies");
    };
    arguments
        .iter_mut()
        .enumerate()
        .filter(|(at, _)| !unrun.contains(at))
        .map(|(_, argument)| argument)
        .collect()
}

/// Every node lowered where `node` is lowered, `node` first, depth first and in the order they are
/// written: every node under it but the functions a call under it never applies.
///
/// What a pass asking what an object runs walks. One asking what the document says walks
/// [`Node::each_written`] instead.
pub(crate) fn each_lowered<'n>(node: &'n Node, visit: &mut impl FnMut(&'n Node)) {
    visit(node);
    for child in lowered_children(node) {
        each_lowered(child, visit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn int(value: i64) -> serde_json::Value {
        json!({ "core": "int", "value": value, "type": { "prim": "INT" }, "aborts": [] })
    }

    /// A function taking what has no value, bound around by a `let` of 7 and answering what a
    /// value published elsewhere answers.
    fn unapplied() -> serde_json::Value {
        let nothing = json!({ "nothing": {} });
        let grown = json!({ "list": { "prim": "INT" } });
        let ty = json!({ "fn": { "takes": [{ "list": nothing }, nothing], "answers": grown } });
        json!({
            "core": "let", "binding": 0, "binds": { "prim": "INT" }, "value": int(7),
            "body": {
                "core": "block", "site": 0,
                "parameters": [{ "binding": 1, "name": "acc" }, { "binding": 2, "name": "x" }],
                "body": {
                    "core": "call",
                    "reaches": { "is": "publishedvalue", "module": "m", "name": "v" },
                    "arguments": [], "type": grown, "aborts": []
                },
                "type": ty, "aborts": []
            },
            "type": ty, "aborts": []
        })
    }

    /// A call reaching `reaches`, handed the function above and then a list and a number.
    fn call(reaches: serde_json::Value) -> Node {
        serde_json::from_value(json!({
            "core": "call", "reaches": reaches,
            "arguments": [
                unapplied(),
                { "core": "list", "elements": [], "type": { "list": { "nothing": {} } },
                  "aborts": [] },
                int(0)
            ],
            "type": { "list": { "prim": "INT" } }, "aborts": []
        }))
        .expect("a node")
    }

    fn visited(node: &Node) -> Vec<&Node> {
        let mut lowered = Vec::new();
        each_lowered(node, &mut |it| lowered.push(it));
        lowered
    }

    /// The whole function is left out, the `let` around its block included, and what the call is
    /// handed besides is not.
    #[test]
    fn a_function_never_applied_is_left_out_whole() {
        let walk = call(json!({ "is": "emitted", "operation": "BUILD_LIST" }));
        assert_eq!(never_applied(&walk), vec![0]);
        let lowered = visited(&walk);
        assert!(!lowered.iter().any(|node| matches!(
            node,
            Node::Let { .. } | Node::Block { .. } | Node::Int { value: 7, .. }
        )));
        assert!(!lowered.iter().any(|node| matches!(
            node,
            Node::Call {
                reaches: Reaches::PublishedValue { .. },
                ..
            }
        )));
        assert!(lowered.iter().any(|node| matches!(node, Node::List { .. })));
        assert!(
            lowered
                .iter()
                .any(|node| matches!(node, Node::Int { value: 0, .. }))
        );
    }

    /// The same of a helper's call and of a kernel's this backend lowers, and not of one it does
    /// not, which is refused with every function it is handed.
    #[test]
    fn a_kernel_this_backend_lowers_never_applies_a_function_over_nothing() {
        let helper = call(json!({ "is": "helper", "reached":
            { "is": "own", "module": "m", "name": "h" } }));
        assert_eq!(never_applied(&helper), vec![0]);
        let lowered = call(
            json!({ "is": "kernel", "kernel": "list.sortBy", "takes": [],
            "fact": { "is": "none" } }),
        );
        assert_eq!(never_applied(&lowered), vec![0]);
        let kernel = call(json!({ "is": "kernel", "kernel": "list.fold", "takes": [],
            "fact": { "is": "none" } }));
        assert!(never_applied(&kernel).is_empty());
        assert_eq!(visited(&kernel).len(), kernel_written(&kernel));
    }

    fn kernel_written(node: &Node) -> usize {
        let mut written = 0;
        node.each_written(&mut |_| written += 1);
        written
    }
}
