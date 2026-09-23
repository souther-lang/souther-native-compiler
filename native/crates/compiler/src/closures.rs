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
//! A `Node::Block`'s own type, its own parameters and its own body's type are three separate
//! statements of one fact on the wire — `ProgramWriter` writes all three from one `Core.Block`, but
//! nothing upstream holds them to each other the way one Java value would. This reads the document
//! strictly, the same as `agrees_with_its_target` in the crate root does for a target and its local
//! definition: every site's signature is checked against its own parameters and its own body's type
//! once, here, at the point the site is built — not left for a lowering three call sites downstream
//! to each rediscover, and not trusted on the strength of what a well-behaved writer would send. A
//! document naming two sites under one `site` ordinal is the same kind of wrong: `ProgramWriter`
//! promises the number is document-wide unique, but a promise from the other language is not a
//! check on this side of the wire, so a duplicate is refused here rather than let the earlier site's
//! plan silently answer for both.

use crate::transport::{Definition, FnSignature, Node, Parameter, Program, Ty};
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
    /// The module whose copy of a definition a call from this site's own body reaches — the same
    /// fact `Lowering::carrier` already threads for every other body, carried here because a site
    /// nested inside one module's body is still defined as this module's own local function.
    pub module: &'a str,
    pub parameters: &'a [Parameter],
    pub body: &'a Node,
    /// What this site takes and answers, unwrapped from the block's own `Ty::Fn` once, here, and
    /// checked against `parameters` and `body`'s own type at the same time (see this module's own
    /// doc) — so every later reader of a `Site` reads an established fact instead of an unchecked
    /// `Ty` it would otherwise have to unwrap and verify itself.
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
    pub fn of_program(program: &'a Program) -> Result<Self> {
        let mut sites = ClosureSites::default();
        for written in &program.modules {
            for held in &written.helpers {
                Planner::new(&mut sites, &written.name).free(&held.body, &mut HashSet::new())?;
            }
            for value in &written.values {
                Planner::new(&mut sites, &written.name).free(&value.body, &mut HashSet::new())?;
            }
            for entry in &written.entries {
                Planner::new(&mut sites, &written.name).free(&entry.body, &mut HashSet::new())?;
            }
            for local in &written.definitions {
                // A composition names no `Core` of its own — its stages reach other behaviors by
                // name, never by a function value the body holds — so nothing here is a closure
                // site, and only `Body` is walked.
                if let Definition::Body { body, .. } = local {
                    Planner::new(&mut sites, &written.name).free(body, &mut HashSet::new())?;
                }
            }
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
    module: &'a str,
}

impl<'p, 'a> Planner<'p, 'a> {
    fn new(sites: &'p mut ClosureSites<'a>, module: &'a str) -> Self {
        Planner { sites, module }
    }

    /// What `node` reaches outside `bound`, first-reached order, with every `Node::Block` under it
    /// recorded as its own site along the way.
    ///
    /// `bound` is entered and left exactly where a scope opens and closes (see `walk`'s own `Let`
    /// and `Match` arms) rather than cloned at every binder: a walk over a body nested `n` `Let`s
    /// deep clones a growing set at every one of them if a binder's scope is threaded down by
    /// value, which is quadratic in nesting depth for every top-level body this is run over —
    /// including one with no closure in it at all, since `ClosureSites::of_program` runs
    /// unconditionally over the whole program.
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
                if fn_.takes.len() != parameters.len() {
                    bail!(
                        "closure site {site} is declared with {} parameters and a type naming {}: \
                         `Node::Block.parameters` and its own `Ty::Fn.takes` are two statements of \
                         one fact and this document's disagree",
                        parameters.len(),
                        fn_.takes.len()
                    );
                }
                if fn_.answers.as_ref() != body.ty() {
                    bail!(
                        "closure site {site} answers {} at its own type and {} at its body's: \
                         `Ty::Fn.answers` and `Node::Block.body`'s own type are two statements of \
                         one fact and this document's disagree",
                        fn_.answers.spelt(),
                        body.ty().spelt()
                    );
                }

                let already_there = self
                    .sites
                    .by_site
                    .insert(
                        *site,
                        Site {
                            module: self.module,
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
                        },
                    )
                    .is_some();
                if already_there {
                    bail!(
                        "two `Node::Block`s both claim closure site {site}: `ProgramWriter` \
                         promises this number is unique across the whole document, and this reader \
                         does not take that on trust — a duplicate would otherwise let the first \
                         block's lifted function and captures silently answer for the second's too"
                    );
                }

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
