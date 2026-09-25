//! A fold the checker's compiler rewrote into a walk that builds what it only grows.
//!
//! `List.map` and `List.filter` are folds seeded with `[]` whose step answers `acc ++ [y]`. Built
//! as written, each step makes a new list of everything so far, so a walk over n elements copies
//! O(n²) slots. The checker's compiler (`GrowingFold`) proves that the step does nothing with the
//! accumulator but add to it and answer it, and rewrites the fold into `$build(step, xs, from)`,
//! whose step adds with `$grow(acc, rhs)`. Nobody can see the lists in between, so one list is grown
//! in place and handed over once.
//!
//! Two things are said here, once each, for everything that reads such a walk.
//!
//! Which block is the walk's step ([`Step::of_walk`]). A step is not a function value. It runs where
//! the walk stands, as the body of a loop, reading what it closes over from the frame around it, so
//! it is never laid out as a closure: the closure planning, the copying of a helper over type
//! variables and the lowering all ask this rather than taking every block for a closure. A step one
//! of whose parameters is the type of what has no value never runs at all: it is the step of a walk
//! over an empty list literal, and nothing of it is lowered ([`Step::never_runs`]). The checker's own
//! backend hands `Fn.NEVER` in its place, because it hands its walk a function; this one hands its
//! walk nothing.
//!
//! That the list a walk grows goes nowhere but where it is grown ([`confined`]). While the walk
//! runs, the accumulator is not a list but the compiler's own record of one being grown, which is
//! laid out as nothing else is. Read as a list anywhere — its length taken, handed to a call, kept
//! in a closure — it would be read as what it is not. The checker's compiler rewrote only steps
//! where that does not happen, and a rewrite proved on the other side of the wire is not a check on
//! this side of it, so the proof is held again here, as a relation the document has to keep.

use crate::transport::{Emitted, FnSignature, Node, Parameter, Reaches, Ty};
use anyhow::{Result, bail};
use std::collections::HashSet;

/// A value bound around a step, which the step reads: the checker binds a captured value this way
/// where it expanded the call the step was handed to.
pub(crate) struct Around<'n> {
    pub binding: usize,
    pub binds: &'n Ty,
    pub value: &'n Node,
}

/// The step of a walk that builds a collection: the values bound around it, and the block they end
/// in.
pub(crate) struct Step<'n> {
    /// Outermost first, which is the order they are worked out in.
    pub around: Vec<Around<'n>>,
    /// The accumulator, and then the element.
    pub parameters: &'n [Parameter],
    pub body: &'n Node,
    pub signature: &'n FnSignature,
}

impl<'n> Step<'n> {
    /// The step `node` walks with, where `node` is a walk that builds a list or a map and its step
    /// is written as a block taking an accumulator and an element, under any number of `let`s.
    /// `None` for any other node.
    ///
    /// Of a list's walk the shape is held ([`confined`]), so where `node` is one, this is `Some`
    /// for every document that got past `Coherent`. Of a map's it is not: no map has a layout
    /// here, so such a walk is refused as not lowered, and until then its step is read as a step
    /// where it has that shape and as ordinary code where it does not.
    pub(crate) fn of_walk(node: &'n Node) -> Option<Step<'n>> {
        let Node::Call {
            reaches:
                Reaches::Emitted {
                    operation: Emitted::BuildList | Emitted::BuildMap,
                },
            arguments,
            ..
        } = node
        else {
            return None;
        };
        let mut around = Vec::new();
        let mut step = arguments.first()?;
        while let Node::Let {
            binding,
            binds,
            value,
            body,
            ..
        } = step
        {
            around.push(Around {
                binding: *binding,
                binds,
                value,
            });
            step = body;
        }
        let Node::Block {
            parameters,
            body,
            ty: Ty::Fn { fn_: signature },
            ..
        } = step
        else {
            return None;
        };
        (parameters.len() == 2 && signature.takes.len() == 2).then_some(Step {
            around,
            parameters,
            body,
            signature,
        })
    }

    /// Whether the step is never applied: one of its parameters is the type of what has no value,
    /// so it is the step of a walk over an empty list literal and there is no element to hand it
    /// (upstream `Core.neverRuns`). An empty accumulator, a `List<Nothing>`, is a list and not this.
    pub(crate) fn never_runs(&self) -> bool {
        self.signature
            .takes
            .iter()
            .any(|taken| matches!(taken, Ty::Nothing { .. }))
    }
}

/// What `node` writes and nothing lowers: the body of its step, where `node` is a walk whose step
/// never runs ([`Step::never_runs`]). `None` for every other node.
///
/// The one statement of which part of a body is not run. A body either runs or does not
/// (`Runs::runs`), and inside one that runs this is the only code that does not: every pass that
/// asks what an object runs, reaches or has to lower asks it through this, by way of
/// [`each_lowered`] or directly, and asks nothing of what it answers. Whether the document is
/// coherent is asked of all of it, run or not.
pub(crate) fn never_lowered(node: &Node) -> Option<&Node> {
    Step::of_walk(node)
        .filter(Step::never_runs)
        .map(|step| step.body)
}

/// Every node lowered where `node` is lowered, `node` first, depth first and in the order they are
/// written: every node under it but what [`never_lowered`] answers of a node above it.
///
/// What a pass asking what an object runs walks. One asking what the document says walks
/// [`Node::each_written`] instead.
pub(crate) fn each_lowered<'n>(node: &'n Node, visit: &mut impl FnMut(&'n Node)) {
    fn walk<'n>(node: &'n Node, unrun: &mut Vec<&'n Node>, visit: &mut impl FnMut(&'n Node)) {
        if unrun.iter().any(|it| std::ptr::eq(*it, node)) {
            return;
        }
        visit(node);
        let entered = never_lowered(node);
        if let Some(body) = entered {
            unrun.push(body);
        }
        for child in node.children() {
            walk(child, unrun, visit);
        }
        if entered.is_some() {
            unrun.pop();
        }
    }
    walk(node, &mut Vec::new(), visit);
}

/// Refuses, as the two halves disagreeing, a body in which the list a walk grows could be read as
/// a list: a `$grow` anywhere but where a step answers, adding to anything but that step's own
/// accumulator; the accumulator read anywhere else, including inside another walk's step or a
/// closure; a place a step answers from answering anything but the accumulator or a growth of it;
/// and a walk building a list whose step is not a block taking an accumulator and an element.
///
/// Where a step answers is where `GrowingFold` rewrites: the step's body, and from there the body
/// of a `let`, both branches of an `if`, every arm of a `match`, and what a `Widen` stands over. A
/// `let` binding the accumulator names it again, and the new name is held the same way.
///
/// Of a map's walk nothing is held: no map is laid out here, so one is refused as not lowered
/// wherever it would run, and its step is read as ordinary code.
pub(crate) fn confined(owner: &str, body: &Node) -> Result<()> {
    Confining {
        owner,
        growing: HashSet::new(),
        outside: HashSet::new(),
    }
    .ordinary(body)
}

/// One step's names for the list it grows, and every enclosing step's.
struct Confining<'o> {
    owner: &'o str,
    /// The accumulator of the step being read, and every name bound to it.
    growing: HashSet<usize>,
    /// The same of every step this one stands inside, none of which may be read here at all.
    outside: HashSet<usize>,
}

impl Confining<'_> {
    /// `node` where it is not what a step answers.
    fn ordinary(&mut self, node: &Node) -> Result<()> {
        match node {
            Node::Read { binding, .. } => {
                if self.growing.contains(binding) || self.outside.contains(binding) {
                    bail!(
                        "{}: binding {binding}, the list a walk grows, is read as a list: the \
                         walk grows it where its step answers and nowhere else, and the two \
                         halves disagree",
                        self.owner
                    );
                }
                Ok(())
            }
            Node::Call {
                reaches:
                    Reaches::Emitted {
                        operation: Emitted::GrowList,
                    },
                ..
            } => bail!(
                "{}: {} stands where no step answers: it adds to the list a walk grows where \
                 the walk's step answers, and the two halves disagree",
                self.owner,
                Emitted::GrowList.spelt()
            ),
            Node::Call {
                reaches:
                    Reaches::Emitted {
                        operation: Emitted::BuildList,
                    },
                arguments,
                ..
            } => {
                let Some(step) = Step::of_walk(node) else {
                    bail!(
                        "{}: {} walks with a step that is not a block taking what it has grown \
                         and an element: the two halves disagree",
                        self.owner,
                        Emitted::BuildList.spelt()
                    );
                };
                for around in &step.around {
                    self.ordinary(around.value)?;
                }
                self.step(&step)?;
                for argument in &arguments[1..] {
                    self.ordinary(argument)?;
                }
                Ok(())
            }
            // A name for the list being grown is not a read of it. Unread, it is nothing; read,
            // the read is refused where it stands.
            Node::Let {
                binding,
                value,
                body,
                ..
            } if self.names_the_grown(value) => {
                self.growing.insert(*binding);
                self.ordinary(body)
            }
            _ => node
                .children()
                .into_iter()
                .try_for_each(|child| self.ordinary(child)),
        }
    }

    /// `step`'s body, where it answers what `step` grows: every name the enclosing step had for
    /// its own list is out of reach in here.
    fn step(&mut self, step: &Step) -> Result<()> {
        let enclosing = std::mem::replace(
            &mut self.growing,
            HashSet::from([step.parameters[0].binding]),
        );
        let outside = self.outside.clone();
        self.outside.extend(enclosing.iter().copied());
        let answered = self.answering(step.body);
        self.growing = enclosing;
        self.outside = outside;
        answered
    }

    /// `node` where a step answers: the list it grows, a growth of it, or a fork of those.
    fn answering(&mut self, node: &Node) -> Result<()> {
        match node {
            Node::Read { binding, .. } if self.growing.contains(binding) => Ok(()),
            Node::Widen { value, .. } => self.answering(value),
            Node::Let {
                binding,
                value,
                body,
                ..
            } => {
                if self.names_the_grown(value) {
                    self.growing.insert(*binding);
                } else {
                    self.ordinary(value)?;
                }
                self.answering(body)
            }
            Node::If {
                cond, then, els, ..
            } => {
                self.ordinary(cond)?;
                self.answering(then)?;
                self.answering(els)
            }
            Node::Match { subject, arms, .. } => {
                self.ordinary(subject)?;
                arms.iter().try_for_each(|arm| self.answering(&arm.body))
            }
            Node::Call {
                reaches:
                    Reaches::Emitted {
                        operation: Emitted::GrowList,
                    },
                arguments,
                ..
            } => {
                let [grown, added] = arguments.as_slice() else {
                    bail!(
                        "{}: {} is handed {} values and takes 2: the two halves disagree",
                        self.owner,
                        Emitted::GrowList.spelt(),
                        arguments.len()
                    );
                };
                if !self.names_the_grown(grown) {
                    bail!(
                        "{}: {} adds to something other than the list its walk grows: the two \
                         halves disagree",
                        self.owner,
                        Emitted::GrowList.spelt()
                    );
                }
                self.ordinary(added)
            }
            _ => bail!(
                "{}: a walk's step answers with something other than the list it grows or a \
                 growth of it: the two halves disagree",
                self.owner
            ),
        }
    }

    /// Whether `node` is a read of one of the names the step being read has for the list it
    /// grows, standing as whatever it stands as.
    fn names_the_grown(&self, node: &Node) -> bool {
        let read = match node {
            Node::Widen { value, .. } => value.as_ref(),
            other => other,
        };
        matches!(read, Node::Read { binding, .. } if self.growing.contains(binding))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn int(value: i64) -> serde_json::Value {
        json!({ "core": "int", "value": value, "type": { "prim": "INT" }, "aborts": [] })
    }

    /// `$build` over `[]` whose step is bound around by a `let` of 7 and answers what a value
    /// published elsewhere answers: the published value is only ever reached by a step that never
    /// runs.
    fn walk_over_nothing() -> Node {
        let nothing = json!({ "nothing": {} });
        let grown = json!({ "list": { "prim": "INT" } });
        let seeded = json!({ "list": nothing });
        let step_ty = json!({ "fn": { "takes": [seeded, nothing], "answers": grown } });
        let published = json!({
            "core": "call", "reaches": { "is": "publishedvalue", "module": "m", "name": "v" },
            "arguments": [], "type": grown, "aborts": []
        });
        let step = json!({
            "core": "let", "binding": 0, "binds": { "prim": "INT" }, "value": int(7),
            "body": {
                "core": "block", "site": 0,
                "parameters": [{ "binding": 1, "name": "acc" }, { "binding": 2, "name": "x" }],
                "body": published, "type": step_ty, "aborts": []
            },
            "type": step_ty, "aborts": []
        });
        serde_json::from_value(json!({
            "core": "call", "reaches": { "is": "emitted", "operation": "BUILD_LIST" },
            "arguments": [
                step,
                { "core": "list", "elements": [], "type": { "list": nothing }, "aborts": [] },
                int(0)
            ],
            "type": grown, "aborts": []
        }))
        .expect("a node")
    }

    fn published(node: &Node) -> bool {
        matches!(
            node,
            Node::Call {
                reaches: Reaches::PublishedValue { .. },
                ..
            }
        )
    }

    #[test]
    fn what_is_lowered_leaves_out_the_step_that_never_runs_and_nothing_else() {
        let walk = walk_over_nothing();
        let mut lowered = Vec::new();
        each_lowered(&walk, &mut |node| lowered.push(node));
        assert!(!lowered.iter().any(|node| published(node)));
        // What is bound around the step, the list walked and where the walk starts all run.
        assert!(
            lowered
                .iter()
                .any(|node| matches!(node, Node::Int { value: 7, .. }))
        );
        assert!(lowered.iter().any(|node| matches!(node, Node::List { .. })));
        assert!(
            lowered
                .iter()
                .any(|node| matches!(node, Node::Int { value: 0, .. }))
        );

        let mut written = Vec::new();
        walk.each_written(&mut |node| written.push(node));
        assert!(written.iter().any(|node| published(node)));
    }

    #[test]
    fn only_a_step_taking_a_value_of_nothing_is_never_lowered() {
        let walk = walk_over_nothing();
        assert!(never_lowered(&walk).is_some_and(published));

        let Node::Call { arguments, .. } = &walk else {
            unreachable!("a walk is a call");
        };
        assert!(never_lowered(&arguments[0]).is_none());
    }
}
