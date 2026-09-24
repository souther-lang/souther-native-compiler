//! The kernels this backend lowers, and what each of them takes and answers.
//!
//! A kernel call on the wire says what the checker settled about that application: what it takes
//! (`takes`) and what else it settled beside that (`fact`). That is the checker's statement about
//! one call, and it is not this backend's own contract for a kernel it knows: what `int.add` is
//! lowered as is fixed here whatever a document says, and a document whose settlement disagrees
//! with it is the two halves disagreeing. So [`Coherent`](crate::coherent) holds the one against
//! the other, and the lowering reads the kernel from here and not from the spelling of its key.
//!
//! What a kernel is known as is one [`Contract`], so that a kernel added to this table has to say
//! what it takes, what it answers and what it settles, and cannot say two of the three.
//!
//! A kernel not in this table is not lowered. Nothing about it is known here beyond what the
//! document settled, which is read and held only for the types it writes and the slots its
//! arguments stand in: what it takes, and which fact it carries, are its own kernel's, and the
//! two halves disagreeing about them is refused as this backend not lowering the kernel.

use crate::transport::{KernelFact, Prim, Ty};

/// What this backend knows of a kernel it lowers.
pub(crate) struct Contract {
    /// What it takes, in the order it is handed them.
    pub(crate) takes: Vec<Ty>,
    /// What it answers.
    pub(crate) answers: Ty,
    /// What the checker settles about an application of it beside what it takes. Every kernel
    /// here settles a fact or settles none, and a document carrying another is not one the
    /// checker writes: a pattern belongs to `String.matches` and an ordering subject to the
    /// kernels that order, and no kernel is both.
    pub(crate) fact: KernelFact,
}

/// A kernel this backend lowers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LoweredKernel {
    /// `int.add`: two `Int`s, and their sum.
    IntAdd,
}

impl LoweredKernel {
    /// The kernel a key names, where this backend lowers it.
    pub(crate) fn of(key: &str) -> Option<Self> {
        match key {
            "int.add" => Some(LoweredKernel::IntAdd),
            _ => None,
        }
    }

    /// What this backend knows of it.
    pub(crate) fn contract(self) -> Contract {
        match self {
            LoweredKernel::IntAdd => Contract {
                takes: vec![Ty::Prim { prim: Prim::Int }; 2],
                answers: Ty::Prim { prim: Prim::Int },
                fact: KernelFact::None,
            },
        }
    }
}
