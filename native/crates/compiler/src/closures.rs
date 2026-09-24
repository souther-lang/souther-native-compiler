//! What a function value's closure has to carry, worked out here rather than carried on the wire.
//!
//! `ProgramWriter` writes a [`Node::Block`] whole — its own parameters and its body, nothing about
//! what the body reaches outside itself — because what a backend has to materialise as runtime
//! state is a representation question and not a fact the checker states. This is where this
//! backend answers it: a standard lexical free-variable walk, run once per top-level body before
//! any of it is lowered, so a lifted function can be declared for every site it finds before any
//! body that might reach one (its own enclosing body, or another site nested inside or beside it)
//! is defined — the same two-phase (plan, then define) shape `object_for` already keeps for every
//! other declaration in the object.
//!
//! A nested block's free bindings are worked out against its own parameters alone, never against
//! its enclosing block's — a nested closure has no access to an enclosing closure's stack frame at
//! run time, only to what it was itself given, so whatever it reaches from further out has to be
//! threaded through as one of *its own* captures. What is threaded through is in turn read as a
//! reach of the block that encloses it, exactly as any other free binding is; where that block is
//! itself a closure, the binding becomes one of its own captures too, and where it is a plain
//! top-level body, the binding is simply a live value already in scope there. Which of those it is
//! is not asked here — this only says what each site reaches, in the order it was first reached.
//!
//! These plans are made inside `coherent`, before it reads the bodies, and nothing here checks
//! whether a block's own type, its parameters and its body's type agree, or whether a read is typed
//! as its binder. `coherent` does, after this, and hands the plans on only for a document where
//! every one of those holds. So a capture's type, which a plan takes off a free read, is the type
//! its binder was bound at by the time anything lowers it. A document naming two sites under one
//! `site` ordinal is refused here: `ProgramWriter` promises the number is document-wide unique, but
//! a promise from the other language is not a check on this side of the wire, and the earlier
//! site's plan would otherwise answer for both.

use crate::index;
use crate::transport::{Body, Carrier, FnSignature, Node, Parameter, Ty};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, HashSet};

/// One binding a closure carries forward, and the type it was read at — read off the
/// [`Node::Read`] that reached it (or, where it was reached only because a nested site captured
/// it, off that site's own record of the same fact), since a binding's type does not change
/// between one read of it and another.
#[derive(Clone)]
pub struct Capture {
    pub binding: usize,
    pub ty: Ty,
}

/// One [`Node::Block`], where it stands in the document, and what it reaches.
pub struct Site<'a> {
    /// Where a call from this site's own body is resolved: where the body holding the site stands,
    /// the same fact `Lowering::carrier` threads for every other body, carried here because a site
    /// nested inside one module's body is still defined as this module's own local function.
    pub carrier: Carrier<'a>,
    pub parameters: &'a [Parameter],
    pub body: &'a Node,
    /// What this site takes and answers, unwrapped from the block's own `Ty::Fn` once, here. That
    /// it agrees with `parameters` and with `body`'s own type is held by `coherent` before the plan
    /// leaves it.
    pub signature: &'a FnSignature,
    /// In first-reached order — the order a closure's slots are laid out in, and the order the
    /// lifted function reads them back in.
    pub captures: Vec<Capture>,
}

/// Every closure site the document holds, found once over the whole program.
#[derive(Default)]
pub struct ClosureSites<'a> {
    by_site: BTreeMap<usize, Site<'a>>,
}

impl<'a> ClosureSites<'a> {
    /// Every closure site under `bodies`.
    pub fn of(bodies: impl IntoIterator<Item = Body<'a>>) -> Result<Self> {
        let mut sites = ClosureSites::default();
        for body in bodies {
            Planner::new(&mut sites, body.carrier()).free(body.node, &mut HashSet::new())?;
        }
        Ok(sites)
    }

    pub fn site(&self, site: usize) -> Option<&Site<'a>> {
        self.by_site.get(&site)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&usize, &Site<'a>)> {
        self.by_site.iter()
    }
}

struct Planner<'p, 'a> {
    sites: &'p mut ClosureSites<'a>,
    carrier: Carrier<'a>,
}

impl<'p, 'a> Planner<'p, 'a> {
    fn new(sites: &'p mut ClosureSites<'a>, carrier: Carrier<'a>) -> Self {
        Planner { sites, carrier }
    }

    /// What `node` reaches outside `bound`, first-reached order, with every `Node::Block` under it
    /// recorded as its own site along the way.
    ///
    /// `bound` is entered and left exactly where a scope opens and closes (see `walk`'s own `Let`
    /// and `Match` arms) rather than cloned at every binder: a walk over a body nested `n` `Let`s
    /// deep clones a growing set at every one of them if a binder's scope is threaded down by
    /// value, which is quadratic in nesting depth for every top-level body this is run over —
    /// including one with no closure in it at all, since `ClosureSites::of` runs unconditionally
    /// over every body it is handed.
    fn free(&mut self, node: &'a Node, bound: &mut HashSet<usize>) -> Result<Vec<(usize, Ty)>> {
        let mut acc = Vec::new();
        let mut seen = HashSet::new();
        self.walk(node, bound, &mut acc, &mut seen)?;
        Ok(acc)
    }

    fn walk(
        &mut self,
        node: &'a Node,
        bound: &mut HashSet<usize>,
        acc: &mut Vec<(usize, Ty)>,
        seen: &mut HashSet<usize>,
    ) -> Result<()> {
        match node {
            Node::Read { binding, ty, .. } => {
                if !bound.contains(binding) && seen.insert(*binding) {
                    acc.push((*binding, ty.clone()));
                }
            }
            Node::Block {
                site,
                parameters,
                body,
                ty,
                ..
            } => {
                // Its own parameters and nothing inherited from `bound`: what this site reaches is
                // asked relative to its own lexical boundary alone, never its enclosing one's — see
                // this module's own doc. A fresh set of its own and not a clone of the caller's,
                // since it starts from a different, smaller scope rather than the caller's own.
                let mut own = HashSet::new();
                for parameter in parameters.iter() {
                    own.insert(parameter.binding);
                }
                let captures = self.free(body, &mut own)?;

                let Ty::Fn { fn_ } = ty else {
                    bail!(
                        "closure site {site}'s own type is not a function type: the checker never \
                         gives a `Core.Block` any other type, so this document and this reader \
                         disagree about what a block is"
                    );
                };

                let planned = Site {
                    carrier: self.carrier,
                    parameters,
                    body,
                    signature: fn_,
                    captures: captures
                        .iter()
                        .map(|(binding, ty)| Capture {
                            binding: *binding,
                            ty: ty.clone(),
                        })
                        .collect(),
                };
                // `ProgramWriter` promises this number is unique across the whole document, and
                // this reader does not take that on trust: a duplicate would let the first block's
                // lifted function and captures answer for the second's too.
                index::once(&mut self.sites.by_site, *site, planned, || {
                    format!("two `Node::Block`s both claim closure site {site}")
                })?;

                for (binding, ty) in captures {
                    if !bound.contains(&binding) && seen.insert(binding) {
                        acc.push((binding, ty));
                    }
                }
            }
            Node::Apply {
                function,
                arguments,
                ..
            } => {
                self.walk(function, bound, acc, seen)?;
                for argument in arguments {
                    self.walk(argument, bound, acc, seen)?;
                }
            }
            Node::Let {
                binding,
                value,
                body,
                ..
            } => {
                self.walk(value, bound, acc, seen)?;
                let added = bound.insert(*binding);
                self.walk(body, bound, acc, seen)?;
                if added {
                    bound.remove(binding);
                }
            }
            Node::Match { subject, arms, .. } => {
                self.walk(subject, bound, acc, seen)?;
                for arm in arms {
                    match arm.binding {
                        Some(binding) => {
                            let added = bound.insert(binding);
                            self.walk(&arm.body, bound, acc, seen)?;
                            if added {
                                bound.remove(&binding);
                            }
                        }
                        None => self.walk(&arm.body, bound, acc, seen)?,
                    }
                }
            }
            Node::Binary { left, right, .. } => {
                self.walk(left, bound, acc, seen)?;
                self.walk(right, bound, acc, seen)?;
            }
            Node::Neg { operand, .. } => self.walk(operand, bound, acc, seen)?,
            Node::If {
                cond, then, els, ..
            } => {
                self.walk(cond, bound, acc, seen)?;
                self.walk(then, bound, acc, seen)?;
                self.walk(els, bound, acc, seen)?;
            }
            Node::Construct { values, .. } => {
                for value in values {
                    self.walk(value, bound, acc, seen)?;
                }
            }
            Node::Field { target, .. } => self.walk(target, bound, acc, seen)?,
            Node::Some { value, .. } => self.walk(value, bound, acc, seen)?,
            Node::Tuple { members, .. } => {
                for member in members {
                    self.walk(member, bound, acc, seen)?;
                }
            }
            Node::Member { tuple, .. } => self.walk(tuple, bound, acc, seen)?,
            Node::Widen { value, .. } => self.walk(value, bound, acc, seen)?,
            Node::Call { arguments, .. } => {
                for argument in arguments {
                    self.walk(argument, bound, acc, seen)?;
                }
            }
            Node::Int { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Unit { .. }
            | Node::None { .. } => {}
        }
        Ok(())
    }
}
