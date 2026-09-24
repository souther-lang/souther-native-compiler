//! The kernels this backend lowers, and what each of them takes and answers.
//!
//! A kernel call on the wire says what the checker settled its signature to for that application
//! (`takes`). That is the checker's statement about one call, and it is not this backend's own
//! contract for a kernel it knows: what `int.add` is lowered as is fixed here whatever a document
//! says, and a document whose settlement disagrees with it is the two halves disagreeing. So
//! [`Coherent`](crate::coherent) holds the one against the other, and the lowering reads the
//! kernel from here and not from the spelling of its key.
//!
//! A kernel not in this table is not lowered; nothing about it is known here beyond what the
//! document settled, which is read and not trusted for anything but the slots its arguments stand
//! in.

use crate::transport::{Prim, Ty};

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

    /// What it takes, in the order it is handed them: this backend's own contract.
    pub(crate) fn takes(self) -> Vec<Ty> {
        match self {
            LoweredKernel::IntAdd => vec![Ty::Prim { prim: Prim::Int }; 2],
        }
    }

    /// What it answers.
    pub(crate) fn answers(self) -> Ty {
        match self {
            LoweredKernel::IntAdd => Ty::Prim { prim: Prim::Int },
        }
    }
}
