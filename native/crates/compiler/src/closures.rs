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
    /// The block's own type, always `Ty::Fn` — kept as the checker gave it rather than split into
    /// its `takes`/`answers` here, so a caller reads the one place that shape is named.
    pub ty: &'a Ty,
    /// In first-reached order — the order a closure's slots are laid out in, and the order the
    /// lifted function reads them back in.
    pub captures: Vec<Capture>,
}

impl Site<'_> {
    /// What this site takes and answers — the one place the `Ty::Fn` a `Node::Block`'s own type is
    /// always built from is unwrapped, so every caller asking shares one answer (and one message)
    /// rather than each re-deriving the same fact with its own `bail!`.
    pub fn signature(&self) -> Result<&FnSignature> {
        let Ty::Fn { fn_ } = self.ty else {
            bail!("a closure site whose own type is not a function type");
        };
        Ok(fn_)
    }
}

/// Every closure site the document holds, found once over the whole program.
#[derive(Default)]
pub struct ClosureSites<'a> {
    by_site: BTreeMap<usize, Site<'a>>,
}

impl<'a> ClosureSites<'a> {
    pub fn of_program(program: &'a Program) -> Self {
        let mut sites = ClosureSites::default();
        for written in &program.modules {
            for held in &written.helpers {
                Planner::new(&mut sites, &written.name).free(&held.body, &HashSet::new());
            }
            for value in &written.values {
                Planner::new(&mut sites, &written.name).free(&value.body, &HashSet::new());
            }
            for entry in &written.entries {
                Planner::new(&mut sites, &written.name).free(&entry.body, &HashSet::new());
            }
            for local in &written.definitions {
                // A composition names no `Core` of its own — its stages reach other behaviors by
                // name, never by a function value the body holds — so nothing here is a closure
                // site, and only `Body` is walked.
                if let Definition::Body { body, .. } = local {
                    Planner::new(&mut sites, &written.name).free(body, &HashSet::new());
                }
            }
        }
        sites
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
    fn free(&mut self, node: &'a Node, bound: &HashSet<usize>) -> Vec<(usize, Ty)> {
        let mut acc = Vec::new();
        let mut seen = HashSet::new();
        self.walk(node, bound, &mut acc, &mut seen);
        acc
    }

    fn walk(
        &mut self,
        node: &'a Node,
        bound: &HashSet<usize>,
        acc: &mut Vec<(usize, Ty)>,
        seen: &mut HashSet<usize>,
    ) {
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
                // this module's own doc.
                let mut own = HashSet::new();
                for parameter in parameters.iter() {
                    own.insert(parameter.binding);
                }
                let captures = self.free(body, &own);
                self.sites.by_site.insert(
                    *site,
                    Site {
                        module: self.module,
                        parameters,
                        body,
                        ty,
                        captures: captures
                            .iter()
                            .map(|(binding, ty)| Capture {
                                binding: *binding,
                                ty: ty.clone(),
                            })
                            .collect(),
                    },
                );
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
                self.walk(function, bound, acc, seen);
                for argument in arguments {
                    self.walk(argument, bound, acc, seen);
                }
            }
            Node::Let {
                binding,
                value,
                body,
                ..
            } => {
                self.walk(value, bound, acc, seen);
                let mut inner = bound.clone();
                inner.insert(*binding);
                self.walk(body, &inner, acc, seen);
            }
            Node::Match { subject, arms, .. } => {
                self.walk(subject, bound, acc, seen);
                for arm in arms {
                    match arm.binding {
                        Some(binding) => {
                            let mut inner = bound.clone();
                            inner.insert(binding);
                            self.walk(&arm.body, &inner, acc, seen);
                        }
                        None => self.walk(&arm.body, bound, acc, seen),
                    }
                }
            }
            Node::Binary { left, right, .. } => {
                self.walk(left, bound, acc, seen);
                self.walk(right, bound, acc, seen);
            }
            Node::Neg { operand, .. } => self.walk(operand, bound, acc, seen),
            Node::If { cond, then, els, .. } => {
                self.walk(cond, bound, acc, seen);
                self.walk(then, bound, acc, seen);
                self.walk(els, bound, acc, seen);
            }
            Node::Construct { values, .. } => {
                for value in values {
                    self.walk(value, bound, acc, seen);
                }
            }
            Node::Field { target, .. } => self.walk(target, bound, acc, seen),
            Node::Some { value, .. } => self.walk(value, bound, acc, seen),
            Node::Tuple { members, .. } => {
                for member in members {
                    self.walk(member, bound, acc, seen);
                }
            }
            Node::Member { tuple, .. } => self.walk(tuple, bound, acc, seen),
            Node::Call { arguments, .. } => {
                for argument in arguments {
                    self.walk(argument, bound, acc, seen);
                }
            }
            Node::Int { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Unit { .. }
            | Node::None { .. } => {}
        }
    }
}
