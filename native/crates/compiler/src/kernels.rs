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

use crate::transport::{AbortKind, Case, KernelFact, LanguageCase, Prim, Ty};

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
/// As much of a type as a kernel's contract has needed, and no more: a primitive, a list, an
/// optional and a fixed union of cases, and a variable standing for whatever one call settles it
/// as. It is not the language's
/// type and does not check one; it is matched against the types the checker settled, which are
/// concrete. A kernel taking a function or a tuple adds its shape here when it is lowered.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Shape {
    Prim(Prim),
    /// Whatever the call settles it as, the same type everywhere the one number stands.
    Var(usize),
    List(Box<Shape>),
    Option(Box<Shape>),
    /// A union of exactly these cases, in the order the checker writes them: what a truncating
    /// division answers, which has no variable in it.
    Cases(Vec<Case>),
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
            (Shape::Cases(cases), Ty::Union { union }) => cases == union,
            (Shape::Prim(_) | Shape::List(_) | Shape::Option(_) | Shape::Cases(_), _) => false,
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
            Shape::Cases(cases) => Ty::Union {
                union: cases.clone(),
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
            Shape::Cases(cases) => cases
                .iter()
                .map(Case::spelt)
                .collect::<Vec<_>>()
                .join(" | "),
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
    /// `int.truncatingDivide`: a dividend and a divisor, and the quotient truncated toward zero,
    /// or `DivisionByZero`.
    IntTruncatingDivide,
    /// `int.truncatingRemainder`: a dividend and a divisor, and what is left over once the
    /// quotient is truncated toward zero, or `DivisionByZero`.
    IntTruncatingRemainder,
    /// `string.length`: a string, and how many code points it holds.
    StringLength,
}

impl LoweredKernel {
    /// The kernel a key names, where this backend lowers it.
    pub(crate) fn of(key: &str) -> Option<Self> {
        match key {
            "int.add" => Some(LoweredKernel::IntAdd),
            "list.length" => Some(LoweredKernel::ListLength),
            "list.get" => Some(LoweredKernel::ListGet),
            "int.truncatingDivide" => Some(LoweredKernel::IntTruncatingDivide),
            "int.truncatingRemainder" => Some(LoweredKernel::IntTruncatingRemainder),
            "string.length" => Some(LoweredKernel::StringLength),
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
            // A zero divisor is a case of the answer and not a reason to end: that is what the
            // union says. What ends a quotient is the one pair whose quotient no `Int` holds, the
            // smallest `Int` over -1. The remainder of that pair is nought, which an `Int` holds,
            // so the remainder ends for nothing.
            LoweredKernel::IntTruncatingDivide => Contract {
                takes: vec![Shape::Prim(Prim::Int); 2],
                answers: int_or_division_by_zero(),
                fact: FactContract::None,
                aborts: vec![AbortKind::RequiredFormHasNoPlace],
            },
            LoweredKernel::IntTruncatingRemainder => Contract {
                takes: vec![Shape::Prim(Prim::Int); 2],
                answers: int_or_division_by_zero(),
                fact: FactContract::None,
                aborts: Vec::new(),
            },
            LoweredKernel::StringLength => Contract {
                takes: vec![Shape::Prim(Prim::String)],
                answers: Shape::Prim(Prim::Int),
                fact: FactContract::None,
                aborts: Vec::new(),
            },
        }
    }
}

/// What a truncating division answers, its members in the order the checker writes this union in.
fn int_or_division_by_zero() -> Shape {
    Shape::Cases(vec![
        Case::Language {
            case: LanguageCase::DivisionByZero,
        },
        Case::Primitive { prim: Prim::Int },
    ])
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

    const LOWERED: [(&str, LoweredKernel); 6] = [
        ("int.add", LoweredKernel::IntAdd),
        ("int.truncatingDivide", LoweredKernel::IntTruncatingDivide),
        (
            "int.truncatingRemainder",
            LoweredKernel::IntTruncatingRemainder,
        ),
        ("string.length", LoweredKernel::StringLength),
        ("list.length", LoweredKernel::ListLength),
        ("list.get", LoweredKernel::ListGet),
    ];

    /// No kernel lowered here settles a fact, which is a fact about this backend's reading of them.
    #[test]
    fn no_lowered_kernel_settles_a_fact() {
        for (key, kernel) in LOWERED {
            assert_eq!(kernel.contract().fact, FactContract::None, "{key}");
        }
    }

    /// Each key reaches its own kernel, and a key the language does not write reaches none: this
    /// backend does not accept a name the standard library has no declaration for.
    #[test]
    fn a_key_reaches_the_kernel_it_names_and_no_other() {
        for (key, kernel) in LOWERED {
            assert_eq!(LoweredKernel::of(key), Some(kernel));
        }
        assert_eq!(LoweredKernel::of("int.divide"), None);
    }

    fn some_shape_of(kernel: LoweredKernel) -> Vec<Ty> {
        let int = Ty::Prim { prim: Prim::Int };
        match kernel {
            LoweredKernel::IntAdd
            | LoweredKernel::IntTruncatingDivide
            | LoweredKernel::IntTruncatingRemainder => vec![int.clone(), int],
            LoweredKernel::StringLength => vec![Ty::Prim { prim: Prim::String }],
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
        for (key, kernel) in LOWERED {
            let contract = kernel.contract();
            let mut bound = Bound::default();
            for (shape, ty) in contract.takes.iter().zip(some_shape_of(kernel)) {
                assert!(shape.binds(&ty, &mut bound), "{key}");
            }
            assert!(contract.answers.settled(&bound).is_some(), "{key}");
        }
    }

    /// A zero divisor is answered, so it is a case of what the division answers and never a reason
    /// it ends; and the cases are what the division answers whatever it is handed.
    #[test]
    fn a_zero_divisor_is_a_case_of_the_answer_and_not_an_abort() {
        let answer = Ty::Union {
            union: vec![
                Case::Language {
                    case: LanguageCase::DivisionByZero,
                },
                Case::Primitive { prim: Prim::Int },
            ],
        };
        for kernel in [
            LoweredKernel::IntTruncatingDivide,
            LoweredKernel::IntTruncatingRemainder,
        ] {
            let contract = kernel.contract();
            assert!(!contract.aborts.contains(&AbortKind::DivisionByZero));
            assert_eq!(
                contract.answers.settled(&Bound::default()),
                Some(answer.clone())
            );
            assert!(contract.answers.binds(&answer, &mut Bound::default()));
            let reordered = Ty::Union {
                union: vec![
                    Case::Primitive { prim: Prim::Int },
                    Case::Language {
                        case: LanguageCase::DivisionByZero,
                    },
                ],
            };
            assert!(!contract.answers.binds(&reordered, &mut Bound::default()));
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
