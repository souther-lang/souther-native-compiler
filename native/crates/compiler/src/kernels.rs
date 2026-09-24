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
//! what it takes, what it answers, what kind of fact it settles and what it can end a run for, and
//! cannot say three of the four. A contract holds what this backend reads and not what the checker answered: `int.add`
//! takes two `Int`s and that is the backend's own, and a pattern is a kind of fact the kernel
//! settles and never a value it is known to settle. For the same reason `takes` and `answers` are
//! each a [`Shape`] and not a type: `list.get` takes a list of any element and answers an optional
//! of that element, so what it is known to take is a pattern the checker's settlement of one call
//! is matched against, the element bound once from what the call takes and held in what it answers.
//!
//! A kernel not in this table is not lowered. Nothing about it is known here beyond what the
//! document settled, which is read and held only for the types it writes and the slots its
//! arguments stand in: what it takes, and which fact it carries, are its own kernel's, and the
//! two halves disagreeing about them is refused as this backend not lowering the kernel.

use crate::transport::{AbortKind, KernelFact, Prim, Ty};

/// What this backend knows of a kernel it lowers.
pub(crate) struct Contract {
    /// What it takes, in the order it is handed them.
    pub(crate) takes: Vec<Shape>,
    /// What it answers, over the variables what it takes binds.
    pub(crate) answers: Shape,
    /// The kind of fact the checker settles about an application of it beside what it takes.
    pub(crate) fact: FactContract,
    /// Every reason an application of it can end a run without a value, exactly. What the kernel
    /// ends for is the kernel's and not the call's, and the lowering turns the reason it is given
    /// into the status the run ends with.
    pub(crate) aborts: Vec<AbortKind>,
}

/// A type a kernel is known to take or answer, where some part of it may be any type.
///
/// As much of a type as a kernel's contract has needed, and no more: a primitive, a list and an
/// optional, and a variable standing for whatever one call settles it as. It is not the language's
/// type and does not check one; it is matched against the types the checker settled, which are
/// concrete. A kernel taking a function or a tuple adds its shape here when it is lowered.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Shape {
    Prim(Prim),
    /// Whatever the call settles it as, the same type everywhere the one number stands.
    Var(usize),
    List(Box<Shape>),
    Option(Box<Shape>),
}

/// What a contract's variables were settled as by one call.
#[derive(Default, Debug)]
pub(crate) struct Bound(Vec<Option<Ty>>);

impl Shape {
    /// Whether `ty` is of this shape, binding the variables it reaches the first time and holding
    /// them to that binding every time after.
    pub(crate) fn binds(&self, ty: &Ty, bound: &mut Bound) -> bool {
        match (self, ty) {
            (Shape::Prim(known), Ty::Prim { prim }) => known == prim,
            (Shape::Var(at), _) => {
                if bound.0.len() <= *at {
                    bound.0.resize(at + 1, None);
                }
                match &bound.0[*at] {
                    Some(already) => already == ty,
                    None => {
                        bound.0[*at] = Some(ty.clone());
                        true
                    }
                }
            }
            (Shape::List(element), Ty::List { list }) => element.binds(list, bound),
            (Shape::Option(held), Ty::Option { option }) => held.binds(option, bound),
            (Shape::Prim(_) | Shape::List(_) | Shape::Option(_), _) => false,
        }
    }

    /// The type this shape is once its variables are bound, where every one it reaches is.
    pub(crate) fn settled(&self, bound: &Bound) -> Option<Ty> {
        Some(match self {
            Shape::Prim(prim) => Ty::Prim { prim: *prim },
            Shape::Var(at) => bound.0.get(*at)?.clone()?,
            Shape::List(element) => Ty::List {
                list: Box::new(element.settled(bound)?),
            },
            Shape::Option(held) => Ty::Option {
                option: Box::new(held.settled(bound)?),
            },
        })
    }

    /// How this shape is spelt in a message, a variable as the letter the language would write.
    pub(crate) fn spelt(&self) -> String {
        match self {
            Shape::Prim(prim) => prim.spelt().to_string(),
            Shape::Var(at) => format!("'{}", (b'a' + *at as u8) as char),
            Shape::List(element) => format!("a List of {}", element.spelt()),
            Shape::Option(held) => format!("an optional {}", held.spelt()),
        }
    }
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
    /// `list.length`: a list, and how many elements it holds.
    ListLength,
    /// `list.get`: an index and a list, and the element at the index where there is one.
    ListGet,
}

impl LoweredKernel {
    /// The kernel a key names, where this backend lowers it.
    pub(crate) fn of(key: &str) -> Option<Self> {
        match key {
            "int.add" => Some(LoweredKernel::IntAdd),
            "list.length" => Some(LoweredKernel::ListLength),
            "list.get" => Some(LoweredKernel::ListGet),
            _ => None,
        }
    }

    /// What this backend knows of it.
    pub(crate) fn contract(self) -> Contract {
        match self {
            LoweredKernel::IntAdd => Contract {
                takes: vec![Shape::Prim(Prim::Int); 2],
                answers: Shape::Prim(Prim::Int),
                fact: FactContract::None,
                aborts: vec![AbortKind::RequiredFormHasNoPlace],
            },
            LoweredKernel::ListLength => Contract {
                takes: vec![Shape::List(Box::new(Shape::Var(0)))],
                answers: Shape::Prim(Prim::Int),
                fact: FactContract::None,
                aborts: Vec::new(),
            },
            // An index outside the list answers nothing and ends no run: that is what the language
            // answers an `Option` for.
            LoweredKernel::ListGet => Contract {
                takes: vec![Shape::Prim(Prim::Int), Shape::List(Box::new(Shape::Var(0)))],
                answers: Shape::Option(Box::new(Shape::Var(0))),
                fact: FactContract::None,
                aborts: Vec::new(),
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

    const LOWERED: [LoweredKernel; 3] = [
        LoweredKernel::IntAdd,
        LoweredKernel::ListLength,
        LoweredKernel::ListGet,
    ];

    /// No kernel lowered here settles a fact, which is a fact about this backend's reading of them.
    #[test]
    fn no_lowered_kernel_settles_a_fact() {
        for kernel in LOWERED {
            assert_eq!(kernel.contract().fact, FactContract::None);
        }
    }

    fn some_shape_of(kernel: LoweredKernel) -> Vec<Ty> {
        let int = Ty::Prim { prim: Prim::Int };
        match kernel {
            LoweredKernel::IntAdd => vec![int.clone(), int],
            LoweredKernel::ListLength => vec![Ty::List {
                list: Box::new(Ty::Prim { prim: Prim::Bool }),
            }],
            LoweredKernel::ListGet => vec![
                int,
                Ty::List {
                    list: Box::new(Ty::Prim { prim: Prim::Bool }),
                },
            ],
        }
    }

    /// What a kernel answers is settled by what it takes: a variable in its answer that nothing it
    /// takes binds would leave the answer unknown however a call is settled.
    #[test]
    fn what_a_kernel_takes_settles_what_it_answers() {
        for kernel in LOWERED {
            let contract = kernel.contract();
            let mut bound = Bound::default();
            for (shape, ty) in contract.takes.iter().zip(some_shape_of(kernel)) {
                assert!(shape.binds(&ty, &mut bound), "{kernel:?}");
            }
            assert!(contract.answers.settled(&bound).is_some(), "{kernel:?}");
        }
    }

    #[test]
    fn a_variable_is_held_to_the_first_type_it_is_bound_to() {
        let pair = Shape::Option(Box::new(Shape::Var(0)));
        let mut bound = Bound::default();
        let int = Ty::Prim { prim: Prim::Int };
        assert!(Shape::Var(0).binds(&int, &mut bound));
        assert!(pair.binds(
            &Ty::Option {
                option: Box::new(int.clone())
            },
            &mut bound
        ));
        assert!(!pair.binds(
            &Ty::Option {
                option: Box::new(Ty::Prim { prim: Prim::Bool })
            },
            &mut bound
        ));
        assert!(!Shape::List(Box::new(Shape::Var(1))).binds(&int, &mut bound));
    }
}
