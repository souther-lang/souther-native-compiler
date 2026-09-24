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
//! what it takes, what it answers and what kind of fact it settles, and cannot say two of the
//! three. A contract holds what this backend reads and not what the checker answered: `int.add`
//! takes two `Int`s and that is the backend's own, and a pattern is a kind of fact the kernel
//! settles and never a value it is known to settle. A kernel taking a type variable would need its
//! `takes` and `answers` to be a shape in the same way, and this table has no kernel that does yet.
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
    /// The kind of fact the checker settles about an application of it beside what it takes.
    pub(crate) fact: FactContract,
}

/// The kind of fact a kernel settles, and not a fact.
///
/// What a fact holds is the checker's about one application: a pattern differs from call to call,
/// and so does the type an ordering was checked against. What this backend knows of a kernel is
/// which kind of fact it settles, because that is how the backend reads the fact: a pattern belongs
/// to `String.matches` and an ordering subject to the kernels that order, and no kernel settles
/// both or one it does not read. So a contract holds the kind, and the value crosses as the
/// checker's, resolved like any other type the document writes.
///
/// A kind added to [`KernelFact`] is one [`FactContract::accepts`] stops compiling over until it
/// says which contract takes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FactContract {
    /// Nothing beside what it takes.
    None,
    /// The pattern a text is matched against.
    StringMatches,
    /// The type an ordering was checked against.
    OrderingSubject,
}

impl FactContract {
    /// Whether an application settling `fact` is one a kernel of this contract can carry.
    pub(crate) fn accepts(self, fact: &KernelFact) -> bool {
        match fact {
            KernelFact::None => self == FactContract::None,
            KernelFact::StringMatches { pattern: _ } => self == FactContract::StringMatches,
            KernelFact::OrderingSubject { ty: _ } => self == FactContract::OrderingSubject,
        }
    }
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
                fact: FactContract::None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(text: &str) -> KernelFact {
        KernelFact::StringMatches {
            pattern: text.to_string(),
        }
    }

    /// A contract holds the kind of fact, so every pattern is one a `String.matches` contract
    /// accepts, and no fact of another kind is.
    #[test]
    fn a_contract_accepts_the_kind_of_fact_and_not_one_value_of_it() {
        assert!(FactContract::StringMatches.accepts(&pattern("a")));
        assert!(FactContract::StringMatches.accepts(&pattern("[0-9]+")));
        assert!(!FactContract::StringMatches.accepts(&KernelFact::None));
        assert!(!FactContract::None.accepts(&pattern("a")));
        assert!(FactContract::None.accepts(&KernelFact::None));
    }

    #[test]
    fn an_ordering_subject_of_any_type_is_the_kind_an_ordering_contract_accepts() {
        for ty in [Prim::Int, Prim::String] {
            let fact = KernelFact::OrderingSubject {
                ty: Ty::Prim { prim: ty },
            };
            assert!(FactContract::OrderingSubject.accepts(&fact));
            assert!(!FactContract::StringMatches.accepts(&fact));
            assert!(!FactContract::None.accepts(&fact));
        }
    }

    /// The one kernel lowered settles nothing, which is a fact about this backend's reading of it.
    #[test]
    fn int_add_settles_no_fact() {
        assert_eq!(LoweredKernel::IntAdd.contract().fact, FactContract::None);
    }
}
