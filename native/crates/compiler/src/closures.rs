//! What a function value's closure has to carry, worked out here rather than carried on the wire.
//!
//! `ProgramWriter` writes a [`Node::Block`] whole — its own parameters and its body, nothing about
//! what the body reaches outside itself — because what a backend has to materialise as runtime
//! state is a representation question and not a fact the checker states. This is where this
//! backend answers it: a standard lexical free-variable walk, run once per body that is lowered
//! before any of it is lowered, so a lifted function can be declared for every site it finds before
//! any body that might reach one (its own enclosing body, or another site nested inside or beside
//! it) is defined — the same two-phase (plan, then define) shape `object_for` already keeps for every
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
//! Nothing here checks whether a block's own type, its parameters and its body's type agree, or
//! whether a read is typed as its binder. `coherent` does, over the bodies as they are written, and
//! the plans are made only for a document where every one of those holds. So a capture's type,
//! which a plan takes off a free read, is the type its binder was bound at by the time anything
//! lowers it.
//!
//! What a plan is for is a block as it is lowered, and a helper over type variables is lowered as
//! its copies ([`crate::specialize`]): one block as written is one in each copy, at the types that
//! copy has, with a layout and a signature of its own. So a plan is made of the bodies as they are
//! lowered, once the copies are made, and is known by where the block stands in the body it was
//! read from ([`ClosureSites::at`]) and not by the number the block was written under. That number
//! is the document's own statement that it names each site once, and is held to it by
//! [`ClosureSites::numbered`], which reads every body as written: `ProgramWriter` promises the
//! number is document-wide unique, but a promise from the other language is not a check on this
//! side of the wire.
//!
//! Not every block is a site. The step of a walk that builds a collection runs where the walk
//! stands, as the body of a loop, and is never a value ([`Step::of_walk`]): its parameters are bound
//! in the frame the walk stands in, the way a `let`'s are, and what it reads outside itself is read
//! there. A block inside such a step is a site like any other. A function a call never applies
//! ([`crate::unrun`]) is not lowered at all, so nothing in it is planned either; its sites are still
//! numbered, so a number two sites share is refused wherever they stand. A block whose function
//! never runs, handed to a kernel, is a site all the same, since the kernel is handed a value: one
//! with no code and nothing carried, since nothing ever calls it. Its body is not lowered, so what
//! the body reaches is reached by nothing and the sites in it are only numbered.

use crate::growing::Step;
use crate::index;
use crate::transport::{Body, Carrier, FnSignature, Node, Parameter, Reaches, Requirement, Ty};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, HashMap, HashSet};

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
    /// What the body holding the site was handed a capability for, where the site reaches one of
    /// them, itself or through a site nested in it: carried in the slot after the captures, the
    /// way the JVM carries the dependency instance a lambda calls. None where it reaches none, and
    /// the closure carries nothing more.
    pub environment: Option<&'a [Requirement]>,
    /// Whether a call can reach the function: false where it never runs ([`crate::unrun::never_runs`]),
    /// and then the closure is made with no code, no function is lifted for it, and it carries
    /// nothing.
    pub runs: bool,
}

/// One planned function value, by the order it was found in: a lifted function is declared for
/// each, and what the lowering of the block reads is read by it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ClosureId(usize);

impl std::fmt::Display for ClosureId {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(out)
    }
}

/// A body that is lowered, where it stands: what a plan is made of.
pub struct Standing<'a> {
    pub carrier: Carrier<'a>,
    pub environment: &'a [Requirement],
    pub node: &'a Node,
}

impl<'a> From<Body<'a>> for Standing<'a> {
    fn from(body: Body<'a>) -> Self {
        Standing {
            carrier: body.carrier(),
            environment: body.environment(),
            node: body.node,
        }
    }
}

/// Every closure site of the bodies that are lowered, each known by the block it is.
#[derive(Default)]
pub struct ClosureSites<'a> {
    sites: Vec<Site<'a>>,
    at: HashMap<*const Node, ClosureId>,
    /// Every site number met, planned or not, where they are held to be met once: a number two
    /// sites share is refused wherever they stand.
    numbered: Option<BTreeMap<usize, ()>>,
}

impl<'a> ClosureSites<'a> {
    /// Refuses a document where two blocks of `bodies`, read as written, claim one site number.
    pub fn numbered(bodies: impl IntoIterator<Item = Body<'a>>) -> Result<()> {
        let mut sites = ClosureSites {
            numbered: Some(BTreeMap::new()),
            ..ClosureSites::default()
        };
        for body in bodies {
            Planner::new(&mut sites, body.carrier(), body.environment(), true)
                .free(body.node, &mut HashSet::new())?;
        }
        Ok(())
    }

    /// Every closure site under `bodies`, which are the bodies as they are lowered: a helper over
    /// type variables is not one of them, and each of its copies is. A number two blocks share is
    /// no fault here, since a block written once is in every copy.
    pub fn of(bodies: impl IntoIterator<Item = Standing<'a>>) -> Result<Self> {
        let mut sites = ClosureSites::default();
        for body in bodies {
            Planner::new(&mut sites, body.carrier, body.environment, true)
                .free(body.node, &mut HashSet::new())?;
        }
        Ok(sites)
    }

    /// The plan of the block `node`, a `Node::Block` in a body this was made of.
    pub fn at(&self, node: &Node) -> Option<(ClosureId, &Site<'a>)> {
        let id = *self.at.get(&std::ptr::from_ref(node))?;
        Some((id, &self.sites[id.0]))
    }

    /// Every site a function is lifted for: each one whose function runs. One that never runs is
    /// made with no code, so nothing is lifted for it and nothing here can ask for it.
    pub fn lifted(&self) -> impl Iterator<Item = (ClosureId, &Site<'a>)> {
        self.sites
            .iter()
            .enumerate()
            .filter(|(_, site)| site.runs)
            .map(|(at, site)| (ClosureId(at), site))
    }
}

struct Planner<'p, 'a> {
    sites: &'p mut ClosureSites<'a>,
    carrier: Carrier<'a>,
    /// What the body walked was handed a capability for.
    environment: &'a [Requirement],
    /// Whether what is walked is lowered, so that a site in it is planned and not only numbered.
    lowered: bool,
}

/// What a walk found reached from outside what it walked: bindings, in first-reached order, and
/// whether a capability the body was handed.
#[derive(Default)]
struct Reached {
    bindings: Vec<(usize, Ty)>,
    seen: HashSet<usize>,
    environment: bool,
}

impl<'p, 'a> Planner<'p, 'a> {
    fn new(
        sites: &'p mut ClosureSites<'a>,
        carrier: Carrier<'a>,
        environment: &'a [Requirement],
        lowered: bool,
    ) -> Self {
        Planner {
            sites,
            carrier,
            environment,
            lowered,
        }
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
    fn free(&mut self, node: &'a Node, bound: &mut HashSet<usize>) -> Result<Reached> {
        let mut reached = Reached::default();
        self.walk(node, bound, &mut reached)?;
        Ok(reached)
    }

    fn walk(
        &mut self,
        node: &'a Node,
        bound: &mut HashSet<usize>,
        acc: &mut Reached,
    ) -> Result<()> {
        // A call through a capability the body was handed, which a closure making it carries.
        if let Node::Call {
            reaches: Reaches::Behavior { declared },
            ..
        } = node
            && self
                .environment
                .iter()
                .any(|required| required.declared() == *declared)
        {
            acc.environment = true;
        }
        // A function a call never applies is not lowered, so nothing in it is planned; its sites
        // are still numbered.
        let unrun = crate::unrun::never_applied(node);
        if let Node::Call { arguments, .. } = node
            && !unrun.is_empty()
        {
            for (at, argument) in arguments.iter().enumerate() {
                if unrun.contains(&at) {
                    Planner::new(self.sites, self.carrier, self.environment, false)
                        .free(argument, &mut HashSet::new())?;
                } else {
                    self.walk(argument, bound, acc)?;
                }
            }
            return Ok(());
        }
        if let (Some(step), Node::Call { arguments, .. }) = (Step::of_walk(node), node) {
            return self.step(&step, &arguments[1..], bound, acc);
        }
        match node {
            Node::Read { binding, ty, .. } => acc.binding(bound, *binding, ty),
            Node::Block {
                site,
                parameters,
                body,
                ty,
                ..
            } => {
                let Ty::Fn { fn_ } = ty else {
                    bail!(
                        "closure site {site}'s own type is not a function type: the checker never \
                         gives a `Core.Block` any other type, so this document and this reader \
                         disagree about what a block is"
                    );
                };
                // Its own parameters and nothing inherited from `bound`: what this site reaches is
                // asked relative to its own lexical boundary alone, never its enclosing one's — see
                // this module's own doc. A fresh set of its own and not a clone of the caller's,
                // since it starts from a different, smaller scope rather than the caller's own.
                let mut own = HashSet::new();
                for parameter in parameters.iter() {
                    own.insert(parameter.binding);
                }
                // A body that never runs reaches nothing, and the sites in it are only numbered.
                let runs = !crate::unrun::never_runs(fn_);
                let reached = if runs {
                    self.free(body, &mut own)?
                } else {
                    Planner::new(self.sites, self.carrier, self.environment, false)
                        .free(body, &mut own)?;
                    Reached::default()
                };

                let planned = Site {
                    carrier: self.carrier,
                    parameters,
                    body,
                    signature: fn_,
                    captures: reached
                        .bindings
                        .iter()
                        .map(|(binding, ty)| Capture {
                            binding: *binding,
                            ty: ty.clone(),
                        })
                        .collect(),
                    environment: reached.environment.then_some(self.environment),
                    runs,
                };
                // `ProgramWriter` promises this number is unique across the whole document, and
                // this reader does not take that on trust: a duplicate would let the first block's
                // lifted function and captures answer for the second's too.
                if let Some(numbered) = &mut self.sites.numbered {
                    index::once(numbered, *site, (), || {
                        format!("two `Node::Block`s both claim closure site {site}")
                    })?;
                }
                if self.lowered {
                    let id = ClosureId(self.sites.sites.len());
                    index::unique(&mut self.sites.at, std::ptr::from_ref(node), id);
                    self.sites.sites.push(planned);
                }

                for (binding, ty) in &reached.bindings {
                    acc.binding(bound, *binding, ty);
                }
                // What a nested site carries of the environment, the site around it carries to it.
                acc.environment |= reached.environment;
            }
            Node::Apply {
                function,
                arguments,
                ..
            } => {
                self.walk(function, bound, acc)?;
                for argument in arguments {
                    self.walk(argument, bound, acc)?;
                }
            }
            Node::Let {
                binding,
                value,
                body,
                ..
            } => {
                self.walk(value, bound, acc)?;
                let added = bound.insert(*binding);
                self.walk(body, bound, acc)?;
                if added {
                    bound.remove(binding);
                }
            }
            Node::Match { subject, arms, .. } => {
                self.walk(subject, bound, acc)?;
                for arm in arms {
                    match arm.binding {
                        Some(binding) => {
                            let added = bound.insert(binding);
                            self.walk(&arm.body, bound, acc)?;
                            if added {
                                bound.remove(&binding);
                            }
                        }
                        None => self.walk(&arm.body, bound, acc)?,
                    }
                }
            }
            Node::Binary { left, right, .. } => {
                self.walk(left, bound, acc)?;
                self.walk(right, bound, acc)?;
            }
            Node::Neg { operand, .. } => self.walk(operand, bound, acc)?,
            Node::If {
                cond, then, els, ..
            } => {
                self.walk(cond, bound, acc)?;
                self.walk(then, bound, acc)?;
                self.walk(els, bound, acc)?;
            }
            Node::Construct { values, .. } => {
                for value in values {
                    self.walk(value, bound, acc)?;
                }
            }
            // What is built is bound where every clause held and nowhere else: the fields are
            // worked out before it exists, and a departure is taken where it never did.
            Node::Attempt {
                values,
                binding,
                then,
                departures,
                ..
            } => {
                for value in values {
                    self.walk(value, bound, acc)?;
                }
                let added = bound.insert(*binding);
                self.walk(then, bound, acc)?;
                if added {
                    bound.remove(binding);
                }
                for body in departures.bodies() {
                    self.walk(body, bound, acc)?;
                }
            }
            Node::Field { target, .. } => self.walk(target, bound, acc)?,
            Node::Some { value, .. } => self.walk(value, bound, acc)?,
            Node::Tuple { members, .. }
            | Node::List {
                elements: members, ..
            } => {
                for member in members {
                    self.walk(member, bound, acc)?;
                }
            }
            Node::Member { tuple, .. } => self.walk(tuple, bound, acc)?,
            Node::Widen { value, .. } => self.walk(value, bound, acc)?,
            Node::Call { arguments, .. } => {
                for argument in arguments {
                    self.walk(argument, bound, acc)?;
                }
            }
            Node::Int { .. }
            | Node::Bool { .. }
            | Node::Str { .. }
            | Node::Decimal { .. }
            | Node::Temporal { .. }
            | Node::Unit { .. }
            | Node::Unreachable { .. }
            | Node::None { .. } => {}
        }
        Ok(())
    }

    /// A walk that builds a collection, whose step runs where the walk stands: what is bound around
    /// the step and the step's own parameters are bound here, as a `let` binds, for as long as the
    /// step's body is read, and `rest` is the walk's other arguments.
    fn step(
        &mut self,
        step: &Step<'a>,
        rest: &'a [Node],
        bound: &mut HashSet<usize>,
        acc: &mut Reached,
    ) -> Result<()> {
        let mut added = Vec::new();
        for around in &step.around {
            self.walk(around.value, bound, acc)?;
            if bound.insert(around.binding) {
                added.push(around.binding);
            }
        }
        for parameter in step.parameters {
            if bound.insert(parameter.binding) {
                added.push(parameter.binding);
            }
        }
        let walked = self.walk(step.body, bound, acc);
        for binding in added {
            bound.remove(&binding);
        }
        walked?;
        for argument in rest {
            self.walk(argument, bound, acc)?;
        }
        Ok(())
    }
}

impl Reached {
    /// `binding`, read at `ty`, where it is not bound inside what is walked and not already
    /// reached.
    fn binding(&mut self, bound: &HashSet<usize>, binding: usize, ty: &Ty) {
        if !bound.contains(&binding) && self.seen.insert(binding) {
            self.bindings.push((binding, ty.clone()));
        }
    }
}
