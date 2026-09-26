//! What a body writes and nothing runs: a function that is never applied.
//!
//! A body either runs or does not (`Runs::runs`), and inside one that runs this is the only code
//! that does not. It is upstream's two answers about a function over what has no value, one whose
//! parameters include the type of what has no value (`Core.neverRuns`, here [`never_runs`]),
//! transcribed and not reworked. No value of that type is ever made, so nothing can be handed to
//! that parameter and the function is never applied. What upstream leaves out differs by the call
//! it is handed to, and so does what is left out here.
//!
//! Handed to a call that is not a kernel's, it is `NEVER_APPLIED` (`Core.Call.functionArgument`),
//! and the unit is the argument, whatever it is written as: the `let`s the checker binds around a
//! block are part of it, and upstream emits "none of it — no class, no body". The step of a walk
//! over an empty list literal is the case that happens: `List.map(f, [])` walks with a step taking
//! a `Nothing` for an element. A walk hands its step nothing in its place and answers an empty
//! list. Any other such call would have to hand the function it never applies to a copy that takes
//! it, which is a function taking what has no value; that is refused as not lowered where the call
//! is read (`Coherent`), and nothing of the function is lowered either way.
//!
//! Handed to a kernel, it is `HANDED_OVER` like any other function: what the function is made of
//! is worked out, a `let` around its block and the test of an `if` choosing between two included,
//! and the function is made (`BodyGen.emitFunctionValue`). What is never run is the block's body,
//! so the unit is that body and nothing around the block. The block is still a value, of a
//! function no call reaches: `closures` plans it carrying nothing, and it is made with no code.
//!
//! Every pass asking what an object runs, reaches or has to lower asks it through this: by
//! [`each_lowered`], by [`lowered_children`] and [`lowered_children_mut`], by [`never_lowered`]
//! where it enters what is not lowered itself, or by [`never_applied`] and [`never_runs`] where it
//! walks a call or a block itself. Whether the document is coherent is asked of all of it, run or
//! not.

use crate::transport::{FnSignature, Node, Reaches, Ty};

/// Whether a function taking and answering as `function` does is never applied: one of the types
/// it takes is the type of what has no value (`Core.neverRuns`). Only that type, as upstream asks
/// it: the type of what does not answer is a type an answer has, and never one a function is
/// handed.
pub(crate) fn never_runs(function: &FnSignature) -> bool {
    function
        .takes
        .iter()
        .any(|taken| matches!(taken, Ty::Nothing { .. }))
}

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
        // A kernel's row hands the runtime the function it is given, whatever it is.
        Reaches::Kernel { .. } => Vec::new(),
        Reaches::Emitted { .. }
        | Reaches::Helper { .. }
        | Reaches::Value { .. }
        | Reaches::PublishedValue { .. }
        | Reaches::Behavior { .. } => arguments
            .iter()
            .enumerate()
            .filter(|(_, argument)| matches!(argument.ty(), Ty::Fn { fn_ } if never_runs(fn_)))
            .map(|(at, _)| at)
            .collect(),
    }
}

/// The children of `node` that are written and never run: the functions a call that is not a
/// kernel's never applies, and the body of a block whose function never runs. Empty for every
/// other node.
pub(crate) fn never_lowered(node: &Node) -> Vec<&Node> {
    match node {
        Node::Call { arguments, .. } => never_applied(node)
            .into_iter()
            .map(|at| &arguments[at])
            .collect(),
        Node::Block {
            body,
            ty: Ty::Fn { fn_ },
            ..
        } if never_runs(fn_) => vec![&**body],
        _ => Vec::new(),
    }
}

/// `node`'s children, in the order they are written, less those it never runs ([`never_lowered`]).
pub(crate) fn lowered_children(node: &Node) -> Vec<&Node> {
    let unrun = never_lowered(node);
    node.children()
        .into_iter()
        .filter(|child| !unrun.iter().any(|it| std::ptr::eq(*it, *child)))
        .collect()
}

/// The same children, to be rewritten in place.
pub(crate) fn lowered_children_mut(node: &mut Node) -> Vec<&mut Node> {
    let unrun: Vec<*const Node> = never_lowered(node)
        .into_iter()
        .map(|it| it as *const Node)
        .collect();
    node.children_mut()
        .into_iter()
        .filter(|child| !unrun.contains(&(&**child as *const Node)))
        .collect()
}

/// Every node lowered where `node` is lowered, `node` first, depth first and in the order they are
/// written: every node under it but those a node under it never runs.
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

    /// The same of a helper's call, and not of a kernel's, which is handed every function: what
    /// the function is made of is lowered, the `let` around its block included, and only the
    /// block's body is not.
    #[test]
    fn a_kernel_is_handed_every_function_and_runs_none_of_its_body() {
        let helper = call(json!({ "is": "helper", "reached":
            { "is": "own", "module": "m", "name": "h" } }));
        assert_eq!(never_applied(&helper), vec![0]);
        let kernel = call(
            json!({ "is": "kernel", "kernel": "list.sortBy", "takes": [],
            "fact": { "is": "none" } }),
        );
        assert!(never_applied(&kernel).is_empty());
        let lowered = visited(&kernel);
        assert!(
            lowered
                .iter()
                .any(|node| matches!(node, Node::Int { value: 7, .. }))
        );
        assert!(lowered.iter().any(|node| matches!(node, Node::Let { .. })));
        assert!(
            lowered
                .iter()
                .any(|node| matches!(node, Node::Block { .. }))
        );
        assert!(!lowered.iter().any(|node| matches!(
            node,
            Node::Call {
                reaches: Reaches::PublishedValue { .. },
                ..
            }
        )));
        assert_eq!(visited(&kernel).len() + 1, kernel_written(&kernel));
    }

    fn kernel_written(node: &Node) -> usize {
        let mut written = 0;
        node.each_written(&mut |_| written += 1);
        written
    }
}
