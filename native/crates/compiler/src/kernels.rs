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

use crate::transport::{
    AbortKind, Case, Cases, FnSignature, KernelFact, LanguageCase, MapTy, Prim, Ty,
};

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
/// As much of a type as a kernel's contract has needed, and no more: a primitive, a declared type
/// by its key, a list, an optional, a set, a map, a tuple, a function and a fixed union of cases,
/// and a variable standing for whatever one call settles it as. It is not the language's type and
/// does not check one; it is matched against the types the checker settled, which are concrete.
///
/// A function is matched as the checker settled it and not as one that could stand where it is
/// asked for: what an application takes is what each argument stands at exactly, so the parameter
/// of `List.find`'s predicate is the list's element and nothing wider or narrower.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Shape {
    Prim(Prim),
    /// The declared type this key names: `RoundingMode`, which the language declares and the
    /// `Decimal` kernels that round take.
    Declared(&'static str),
    /// Whatever the call settles it as, the same type everywhere the one number stands.
    Var(usize),
    List(Box<Shape>),
    Option(Box<Shape>),
    Set(Box<Shape>),
    Map {
        key: Box<Shape>,
        value: Box<Shape>,
    },
    /// A tuple of these, in order: an entry of a map as a list holds it.
    Tuple(Vec<Shape>),
    /// A function taking these, in order, and answering that.
    Fn {
        takes: Vec<Shape>,
        answers: Box<Shape>,
    },
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
            (
                Shape::Declared(key),
                Ty::Ref {
                    named: Case::Declared { declared },
                },
            ) => declared == key,
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
            (Shape::Set(element), Ty::Set { set }) => element.binds(set, bound),
            (Shape::Map { key, value }, Ty::Map { map }) => {
                key.binds(&map.key, bound) && value.binds(&map.value, bound)
            }
            (Shape::Tuple(members), Ty::Tuple { tuple }) => {
                members.len() == tuple.len()
                    && members
                        .iter()
                        .zip(tuple)
                        .all(|(shape, member)| shape.binds(member, bound))
            }
            (Shape::Fn { takes, answers }, Ty::Fn { fn_ }) => {
                takes.len() == fn_.takes.len()
                    && takes
                        .iter()
                        .zip(&fn_.takes)
                        .all(|(shape, taken)| shape.binds(taken, bound))
                    && answers.binds(&fn_.answers, bound)
            }
            (Shape::Cases(cases), Ty::Union { union }) => cases[..] == union[..],
            (
                Shape::Prim(_)
                | Shape::Declared(_)
                | Shape::List(_)
                | Shape::Option(_)
                | Shape::Set(_)
                | Shape::Map { .. }
                | Shape::Tuple(_)
                | Shape::Fn { .. }
                | Shape::Cases(_),
                _,
            ) => false,
        }
    }

    /// The type this shape is once its variables are bound, where every one it reaches is.
    pub(crate) fn settled(&self, bound: &Bound) -> Option<Ty> {
        Some(match self {
            Shape::Prim(prim) => Ty::Prim { prim: *prim },
            Shape::Declared(key) => Ty::declared(key.to_string()),
            Shape::Var(at) => bound.0.get(*at)?.clone()?,
            Shape::List(element) => Ty::List {
                list: Box::new(element.settled(bound)?),
            },
            Shape::Option(held) => Ty::Option {
                option: Box::new(held.settled(bound)?),
            },
            Shape::Set(element) => Ty::Set {
                set: Box::new(element.settled(bound)?),
            },
            Shape::Map { key, value } => Ty::Map {
                map: MapTy {
                    key: Box::new(key.settled(bound)?),
                    value: Box::new(value.settled(bound)?),
                },
            },
            Shape::Tuple(members) => Ty::Tuple {
                tuple: members
                    .iter()
                    .map(|member| member.settled(bound))
                    .collect::<Option<_>>()?,
            },
            Shape::Fn { takes, answers } => Ty::Fn {
                fn_: FnSignature {
                    takes: takes
                        .iter()
                        .map(|taken| taken.settled(bound))
                        .collect::<Option<_>>()?,
                    answers: Box::new(answers.settled(bound)?),
                },
            },
            Shape::Cases(cases) => Ty::Union {
                union: Cases::one_or_more(cases.clone())
                    .expect("a kernel answering cases answers one or more"),
            },
        })
    }

    /// How this shape is spelt in a message, a variable as the letter the language would write.
    pub(crate) fn spelt(&self) -> String {
        match self {
            Shape::Prim(prim) => prim.spelt().to_string(),
            Shape::Declared(key) => key.to_string(),
            Shape::Var(at) => format!("'{}", (b'a' + *at as u8) as char),
            Shape::List(element) => format!("a List of {}", element.spelt()),
            Shape::Option(held) => format!("an optional {}", held.spelt()),
            Shape::Set(element) => format!("a Set of {}", element.spelt()),
            Shape::Map { key, value } => {
                format!("a Map from {} to {}", key.spelt(), value.spelt())
            }
            Shape::Tuple(members) => format!(
                "a tuple of ({})",
                members
                    .iter()
                    .map(Shape::spelt)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Shape::Fn { takes, answers } => format!(
                "a function from ({}) to {}",
                takes
                    .iter()
                    .map(Shape::spelt)
                    .collect::<Vec<_>>()
                    .join(", "),
                answers.spelt()
            ),
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
/// Where the kind carries a type, the contract says which of the types the kernel takes it is, as
/// a [`Shape`] over the variables what it takes binds. An ordering subject is what the lowering
/// compares by, so a subject that is not the element a sort orders, or not what a key answers, is
/// the two halves disagreeing about what is compared, and is refused as that rather than lowered
/// as a comparison of one type over values of another. The shape binds nothing: what the kernel
/// takes is the one place its variables are bound, and the fact is held to what they were bound
/// to.
///
/// A kind added to [`KernelFact`] is one [`FactContract::accepts`] stops compiling over until it
/// says which contract takes it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum FactContract {
    /// Nothing beside what it takes.
    None,
    /// The pattern a text is matched against.
    StringMatches,
    /// The type an ordering was checked against, which is this shape once what the kernel takes
    /// has bound it.
    OrderingSubject(Shape),
}

impl FactContract {
    /// Whether an application settling `fact` is one a kernel of this contract can carry: the kind
    /// alone, and not what a fact of it holds.
    pub(crate) fn accepts(&self, fact: &KernelFact) -> bool {
        match fact {
            KernelFact::None => *self == FactContract::None,
            KernelFact::StringMatches {
                written: _,
                meaning: _,
            } => *self == FactContract::StringMatches,
            KernelFact::OrderingSubject { ty: _, ordering: _ } => {
                matches!(self, FactContract::OrderingSubject(_))
            }
        }
    }

    /// The type a fact of this contract has to hold once what the kernel takes bound `bound`,
    /// where the kind carries one.
    pub(crate) fn holds(&self, bound: &Bound) -> Option<Ty> {
        match self {
            FactContract::None | FactContract::StringMatches => None,
            FactContract::OrderingSubject(subject) => Some(
                subject
                    .settled(bound)
                    .expect("what a kernel takes binds every variable its ordering subject names"),
            ),
        }
    }
}

/// A kernel this backend lowers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LoweredKernel {
    /// `int.add`: two `Int`s, and their sum.
    IntAdd,
    /// `int.subtract`: two `Int`s, and the first less the second.
    IntSubtract,
    /// `int.multiply`: two `Int`s, and their product.
    IntMultiply,
    /// `int.compare`: two `Int`s, and -1, 0 or 1 as the first is below, at or above the second.
    IntCompare,
    /// `int.floorMod`: a dividend and a divisor, and what is left once the quotient is floored,
    /// which takes the divisor's sign. A zero divisor ends the run.
    IntFloorMod,
    /// `list.length`: a list, and how many elements it holds.
    ListLength,
    /// `list.get`: an index and a list, and the element at the index where there is one.
    ListGet,
    /// `list.find`: a predicate and a list, and the first element it holds for, where one does.
    ListFind,
    /// `list.sortBy`: a key and a list, and the list ordered by what the key answers for each
    /// element, elements of equal keys in the order they came in. The key is worked out once for
    /// each element.
    ListSortBy,
    /// `list.sort`: a list of what the language orders, ordered, equal elements in the order they
    /// came in.
    ListSort,
    /// `list.max`: a list of what the language orders, and the first of its greatest elements,
    /// where it has one.
    ListMax,
    /// `list.min`: the same, and the first of its least.
    ListMin,
    /// `list.reverse`: a list, the other way round.
    ListReverse,
    /// `list.sum`: a list of numbers, and their sum, nought for none. A sum no number of the
    /// element's type holds ends the run.
    ListSum,
    /// `list.product`: a list of numbers, and their product, one for none. A product no number of
    /// the element's type holds ends the run.
    ListProduct,
    /// `list.rangeInclusive`: two `Int`s, and every `Int` from the first to the second, both
    /// included. A span no list could hold ends the run.
    ListRangeInclusive,
    /// `option.map`: a function and an optional, and what the function answers for the value it
    /// holds, where it holds one.
    OptionMap,
    /// `int.truncatingDivide`: a dividend and a divisor, and the quotient truncated toward zero,
    /// or `DivisionByZero`.
    IntTruncatingDivide,
    /// `int.truncatingRemainder`: a dividend and a divisor, and what is left over once the
    /// quotient is truncated toward zero, or `DivisionByZero`.
    IntTruncatingRemainder,
    /// `string.length`: a string, and how many code points it holds.
    StringLength,
    /// `string.toInt`: a string, and the `Int` it is integer text of, or `NotANumber`.
    StringToInt,
    /// `string.fromInt`: an `Int`, written in decimal.
    StringFromInt,
    /// `string.trim`: a string, and it with the String whitespace at either end taken off.
    StringTrim,
    /// `string.lowercase`: a string, lowercased.
    StringLowercase,
    /// `string.uppercase`: a string, uppercased.
    StringUppercase,
    /// `string.contains`: a string and another, and whether the first is in the second.
    StringContains,
    /// `string.startsWith`: a prefix and a string, and whether the string begins with it.
    StringStartsWith,
    /// `string.endsWith`: a suffix and a string, and whether the string ends with it.
    StringEndsWith,
    /// `string.matches`: a pattern and a string, and whether the whole string is one the pattern
    /// denotes. What the pattern denotes is the fact the checker settles.
    StringMatches,
    /// `string.slice`: two indices and a string, and the code points between them. An index the
    /// string has not got ends the run.
    StringSlice,
    /// `string.append`: two strings, joined.
    StringAppend,
    /// `string.split`: a separator and a string, and the pieces between.
    StringSplit,
    /// `string.join`: a separator and a list of strings, joined with it.
    StringJoin,
    /// `string.concat`: a list of strings, joined.
    StringConcat,
    /// `string.replace`: a target, a replacement and a string, and every run of the target
    /// replaced.
    StringReplace,
    /// `string.words`: a string, and the runs of it between String whitespace.
    StringWords,
    /// `string.lines`: a string, and its lines.
    StringLines,
    /// `string.reverse`: a string, and its code points the other way round. A text no string holds
    /// ends the run.
    StringReverse,
    /// `string.repeat`: a count and a string, and that many copies. Copies that are more text than
    /// a string holds end the run.
    StringRepeat,
    /// `string.padLeft`: a width, a pad and a string, and the string widened on the left. A width
    /// that is more text than a string holds ends the run.
    StringPadLeft,
    /// `string.padRight`: the same, widened on the right.
    StringPadRight,
    /// `string.characters`: a string, and each of its code points as a string.
    StringCharacters,
    /// `string.codePoints`: a string, and each of its code points as an `Int`.
    StringCodePoints,
    /// `string.toDecimal`: a string, and the `Decimal` it is decimal text of, or `NotANumber`.
    StringToDecimal,
    /// `string.fromDecimal`: a `Decimal` in plain notation at its scale. A text no string holds ends
    /// the run.
    StringFromDecimal,
    /// `decimal.add`: two `Decimal`s, and their sum at the larger scale. A sum no `Decimal` holds
    /// ends the run.
    DecimalAdd,
    /// `decimal.subtract`: the same, the first less the second.
    DecimalSubtract,
    /// `decimal.multiply`: two `Decimal`s, and their product at the sum of their scales.
    DecimalMultiply,
    /// `decimal.compare`: two `Decimal`s, and -1, 0 or 1 by amount, whatever their scales.
    DecimalCompare,
    /// `decimal.fromInt`: an `Int`, as a `Decimal` at scale nought.
    DecimalFromInt,
    /// `decimal.toInt`: a rounding mode and a `Decimal`, and the whole number it rounds to. One no
    /// `Int` holds ends the run.
    DecimalToInt,
    /// `decimal.round`: a scale, a rounding mode and a `Decimal`, and the value at that scale. A
    /// scale outside the range, or a value no `Decimal` holds, ends the run.
    DecimalRound,
    /// `decimal.divide`: a dividend, a divisor, a scale and a rounding mode, and the quotient at
    /// that scale, or `DivisionByZero`.
    DecimalDivide,
    /// `rational.fromInt`: an `Int`, as the `Rational` it is exactly.
    RationalFromInt,
    /// `rational.fromDecimal`: a `Decimal`, as the `Rational` it is exactly, at whatever scale.
    RationalFromDecimal,
    /// `rational.toWholeNumber`: a `Rational`, and the `Int` it is where it is a whole number, or
    /// `NotWhole`. A whole number no `Int` holds ends the run.
    RationalToWholeNumber,
    /// `rational.toFiniteDecimal`: a `Rational`, and the `Decimal` it is where it has a finite
    /// decimal spelling, or `NotAFiniteDecimal`. One no `Decimal` holds ends the run.
    RationalToFiniteDecimal,
    /// `rational.toInt`: a rounding mode and a `Rational`, and the whole number it rounds to. One
    /// no `Int` holds ends the run.
    RationalToInt,
    /// `rational.toDecimal`: a scale, a rounding mode and a `Rational`, and the value at that
    /// scale. A scale outside the range, or a value no `Decimal` holds, ends the run.
    RationalToDecimal,
    /// `rational.add`: two `Rational`s, and their exact sum.
    RationalAdd,
    /// `rational.subtract`: the same, the first less the second.
    RationalSubtract,
    /// `rational.multiply`: two `Rational`s, and their exact product.
    RationalMultiply,
    /// `rational.divide`: a dividend and a divisor, and the exact quotient. A zero divisor ends the
    /// run, and so does a quotient with no place.
    RationalDivide,
    /// `rational.compare`: two `Rational`s, and -1, 0 or 1 by exact value.
    RationalCompare,
    /// `date.addDays`: a count of days and a `Date`, and the `Date` that many days on. A day past the end of what a `Date` holds ends the run.
    DateAddDays,
    /// `date.addMonths`: a count of months and a `Date`, and the `Date` that many months on, the last day of the month where the day is past its end. A month past the end of what a `Date` holds ends the run.
    DateAddMonths,
    /// `date.addYears`: a count of years and a `Date`, the twenty-ninth of February becoming the twenty-eighth where the year has none. A year past the end of what a `Date` holds ends the run.
    DateAddYears,
    /// `date.daysBetween`: two `Date`s, and the whole days from the first to the second.
    DateDaysBetween,
    /// `date.year`: a `Date`, and its year.
    DateYear,
    /// `date.month`: a `Date`, and its month, from 1 to 12.
    DateMonth,
    /// `date.day`: a `Date`, and its day of the month.
    DateDay,
    /// `date.fromParts`: a year, a month and a day, and the `Date` they name or `NotADate`.
    DateFromParts,
    /// `time.fromParts`: an hour, a minute and a second, and the `Time` they name or `NotATime`.
    TimeFromParts,
    /// `time.hour`: a `Time`, and its hour.
    TimeHour,
    /// `time.minute`: a `Time`, and its minute.
    TimeMinute,
    /// `time.second`: a `Time`, and its second.
    TimeSecond,
    /// `datetime.addMinutes`: a count of minutes and a `DateTime`, and the `DateTime` that many minutes on. One past the end of what a `DateTime` holds ends the run.
    DateTimeAddMinutes,
    /// `datetime.addHours`: as `datetime.addMinutes`, in hours.
    DateTimeAddHours,
    /// `datetime.addDays`: as `datetime.addMinutes`, in days.
    DateTimeAddDays,
    /// `datetime.minutesBetween`: two `DateTime`s, and the whole minutes from the first to the second, the part of one that is left dropped towards nought.
    DateTimeMinutesBetween,
    /// `datetime.toDate`: a `DateTime`, and its `Date`.
    DateTimeToDate,
    /// `datetime.toTime`: a `DateTime`, and its `Time`.
    DateTimeToTime,
    /// `datetime.fromDateAndTime`: a `Date` and a `Time`, and the `DateTime` they make, which cannot fail.
    DateTimeFromDateAndTime,
    /// `set.empty`: the set with no members, whose element is what the call is settled as.
    SetEmpty,
    /// `set.singleton`: a value, and the set holding it alone.
    SetSingleton,
    /// `set.insert`: a value and a set, and the set holding it too. One more member than a set holds
    /// ends the run.
    SetInsert,
    /// `set.remove`: a value and a set, and the set without it.
    SetRemove,
    /// `set.contains`: a value and a set, and whether the set holds one equal to it.
    SetContains,
    /// `set.union`: two sets, and every member of either. More members than a set holds ends the run.
    SetUnion,
    /// `set.intersection`: two sets, and the first's members the second holds too.
    SetIntersection,
    /// `set.difference`: two sets, and the first's members the second does not hold.
    SetDifference,
    /// `set.isEmpty`: a set, and whether it holds no member.
    SetIsEmpty,
    /// `set.size`: a set, and how many members it holds.
    SetSize,
    /// `set.toList`: a set, and its members as a list, in an order the language does not say.
    SetToList,
    /// `set.fromList`: a list, and the set of its elements.
    SetFromList,
    /// `map.empty`: the map with no keys, whose keys and values are what the call is settled as.
    MapEmpty,
    /// `map.get`: a key and a map, and the value under a key equal to it, where there is one.
    MapGet,
    /// `map.containsKey`: a key and a map, and whether the map holds one equal to it.
    MapContainsKey,
    /// `map.keys`: a map, and its keys as a list, in an order the language does not say.
    MapKeys,
    /// `map.values`: a map, and its values as a list, in the order `map.keys` lists their keys.
    MapValues,
    /// `map.singleton`: a key and a value, and the map holding the one under the other.
    MapSingleton,
    /// `map.insert`: a key, a value and a map, and the map holding the value under the key. One more
    /// key than a map holds ends the run.
    MapInsert,
    /// `map.remove`: a key and a map, and the map without it.
    MapRemove,
    /// `map.isEmpty`: a map, and whether it holds no key.
    MapIsEmpty,
    /// `map.size`: a map, and how many keys it holds.
    MapSize,
    /// `map.toList`: a map, and its entries as a list of pairs.
    MapToList,
    /// `map.fromList`: a list of pairs, and the map of them, a later pair winning a key.
    MapFromList,
}

impl LoweredKernel {
    /// The kernel a key names, where this backend lowers it.
    pub(crate) fn of(key: &str) -> Option<Self> {
        Some(match key {
            "int.add" => LoweredKernel::IntAdd,
            "int.subtract" => LoweredKernel::IntSubtract,
            "int.multiply" => LoweredKernel::IntMultiply,
            "int.compare" => LoweredKernel::IntCompare,
            "int.floorMod" => LoweredKernel::IntFloorMod,
            "list.length" => LoweredKernel::ListLength,
            "list.get" => LoweredKernel::ListGet,
            "list.find" => LoweredKernel::ListFind,
            "list.sortBy" => LoweredKernel::ListSortBy,
            "list.sort" => LoweredKernel::ListSort,
            "list.max" => LoweredKernel::ListMax,
            "list.min" => LoweredKernel::ListMin,
            "list.reverse" => LoweredKernel::ListReverse,
            "list.sum" => LoweredKernel::ListSum,
            "list.product" => LoweredKernel::ListProduct,
            "list.rangeInclusive" => LoweredKernel::ListRangeInclusive,
            "option.map" => LoweredKernel::OptionMap,
            "int.truncatingDivide" => LoweredKernel::IntTruncatingDivide,
            "int.truncatingRemainder" => LoweredKernel::IntTruncatingRemainder,
            "string.length" => LoweredKernel::StringLength,
            "string.toInt" => LoweredKernel::StringToInt,
            "string.fromInt" => LoweredKernel::StringFromInt,
            "string.trim" => LoweredKernel::StringTrim,
            "string.lowercase" => LoweredKernel::StringLowercase,
            "string.uppercase" => LoweredKernel::StringUppercase,
            "string.contains" => LoweredKernel::StringContains,
            "string.startsWith" => LoweredKernel::StringStartsWith,
            "string.endsWith" => LoweredKernel::StringEndsWith,
            "string.matches" => LoweredKernel::StringMatches,
            "string.slice" => LoweredKernel::StringSlice,
            "string.append" => LoweredKernel::StringAppend,
            "string.split" => LoweredKernel::StringSplit,
            "string.join" => LoweredKernel::StringJoin,
            "string.concat" => LoweredKernel::StringConcat,
            "string.replace" => LoweredKernel::StringReplace,
            "string.words" => LoweredKernel::StringWords,
            "string.lines" => LoweredKernel::StringLines,
            "string.reverse" => LoweredKernel::StringReverse,
            "string.repeat" => LoweredKernel::StringRepeat,
            "string.padLeft" => LoweredKernel::StringPadLeft,
            "string.padRight" => LoweredKernel::StringPadRight,
            "string.characters" => LoweredKernel::StringCharacters,
            "string.codePoints" => LoweredKernel::StringCodePoints,
            "string.toDecimal" => LoweredKernel::StringToDecimal,
            "string.fromDecimal" => LoweredKernel::StringFromDecimal,
            "decimal.add" => LoweredKernel::DecimalAdd,
            "decimal.subtract" => LoweredKernel::DecimalSubtract,
            "decimal.multiply" => LoweredKernel::DecimalMultiply,
            "decimal.compare" => LoweredKernel::DecimalCompare,
            "decimal.fromInt" => LoweredKernel::DecimalFromInt,
            "decimal.toInt" => LoweredKernel::DecimalToInt,
            "decimal.round" => LoweredKernel::DecimalRound,
            "decimal.divide" => LoweredKernel::DecimalDivide,
            "rational.fromInt" => LoweredKernel::RationalFromInt,
            "rational.fromDecimal" => LoweredKernel::RationalFromDecimal,
            "rational.toWholeNumber" => LoweredKernel::RationalToWholeNumber,
            "rational.toFiniteDecimal" => LoweredKernel::RationalToFiniteDecimal,
            "rational.toInt" => LoweredKernel::RationalToInt,
            "rational.toDecimal" => LoweredKernel::RationalToDecimal,
            "rational.add" => LoweredKernel::RationalAdd,
            "rational.subtract" => LoweredKernel::RationalSubtract,
            "rational.multiply" => LoweredKernel::RationalMultiply,
            "rational.divide" => LoweredKernel::RationalDivide,
            "rational.compare" => LoweredKernel::RationalCompare,
            "date.addDays" => LoweredKernel::DateAddDays,
            "date.addMonths" => LoweredKernel::DateAddMonths,
            "date.addYears" => LoweredKernel::DateAddYears,
            "date.daysBetween" => LoweredKernel::DateDaysBetween,
            "date.year" => LoweredKernel::DateYear,
            "date.month" => LoweredKernel::DateMonth,
            "date.day" => LoweredKernel::DateDay,
            "date.fromParts" => LoweredKernel::DateFromParts,
            "time.fromParts" => LoweredKernel::TimeFromParts,
            "time.hour" => LoweredKernel::TimeHour,
            "time.minute" => LoweredKernel::TimeMinute,
            "time.second" => LoweredKernel::TimeSecond,
            "datetime.addMinutes" => LoweredKernel::DateTimeAddMinutes,
            "datetime.addHours" => LoweredKernel::DateTimeAddHours,
            "datetime.addDays" => LoweredKernel::DateTimeAddDays,
            "datetime.minutesBetween" => LoweredKernel::DateTimeMinutesBetween,
            "datetime.toDate" => LoweredKernel::DateTimeToDate,
            "datetime.toTime" => LoweredKernel::DateTimeToTime,
            "datetime.fromDateAndTime" => LoweredKernel::DateTimeFromDateAndTime,
            "set.empty" => LoweredKernel::SetEmpty,
            "set.singleton" => LoweredKernel::SetSingleton,
            "set.insert" => LoweredKernel::SetInsert,
            "set.remove" => LoweredKernel::SetRemove,
            "set.contains" => LoweredKernel::SetContains,
            "set.union" => LoweredKernel::SetUnion,
            "set.intersection" => LoweredKernel::SetIntersection,
            "set.difference" => LoweredKernel::SetDifference,
            "set.isEmpty" => LoweredKernel::SetIsEmpty,
            "set.size" => LoweredKernel::SetSize,
            "set.toList" => LoweredKernel::SetToList,
            "set.fromList" => LoweredKernel::SetFromList,
            "map.empty" => LoweredKernel::MapEmpty,
            "map.get" => LoweredKernel::MapGet,
            "map.containsKey" => LoweredKernel::MapContainsKey,
            "map.keys" => LoweredKernel::MapKeys,
            "map.values" => LoweredKernel::MapValues,
            "map.singleton" => LoweredKernel::MapSingleton,
            "map.insert" => LoweredKernel::MapInsert,
            "map.remove" => LoweredKernel::MapRemove,
            "map.isEmpty" => LoweredKernel::MapIsEmpty,
            "map.size" => LoweredKernel::MapSize,
            "map.toList" => LoweredKernel::MapToList,
            "map.fromList" => LoweredKernel::MapFromList,
            _ => return None,
        })
    }

    /// What this backend knows of it.
    pub(crate) fn contract(self) -> Contract {
        let int = || Shape::Prim(Prim::Int);
        let string = || Shape::Prim(Prim::String);
        let decimal = || Shape::Prim(Prim::Decimal);
        let rational = || Shape::Prim(Prim::Rational);
        let mode = || Shape::Declared(ROUNDING_MODE);
        let bool = || Shape::Prim(Prim::Bool);
        let date = || Shape::Prim(Prim::Date);
        let time = || Shape::Prim(Prim::Time);
        let datetime = || Shape::Prim(Prim::DateTime);
        let strings = || Shape::List(Box::new(string()));
        let a = || Shape::Var(0);
        let b = || Shape::Var(1);
        let list = |element: Shape| Shape::List(Box::new(element));
        let optional = |held: Shape| Shape::Option(Box::new(held));
        let set = |element: Shape| Shape::Set(Box::new(element));
        let map = |key: Shape, value: Shape| Shape::Map {
            key: Box::new(key),
            value: Box::new(value),
        };
        let pair = || Shape::Tuple(vec![a(), b()]);
        let function = |taken: Shape, answers: Shape| Shape::Fn {
            takes: vec![taken],
            answers: Box::new(answers),
        };
        let known = |takes: Vec<Shape>, answers: Shape, aborts: Vec<AbortKind>| Contract {
            takes,
            answers,
            fact: FactContract::None,
            aborts,
        };
        match self {
            // A sum, a difference or a product no `Int` holds ends the run.
            LoweredKernel::IntAdd | LoweredKernel::IntSubtract | LoweredKernel::IntMultiply => {
                known(
                    vec![int(), int()],
                    int(),
                    vec![AbortKind::RequiredFormHasNoPlace],
                )
            }
            LoweredKernel::IntCompare => known(vec![int(), int()], int(), Vec::new()),
            // A zero divisor ends the run, which is why what it answers is a plain `Int`; the
            // remainder of the one pair whose quotient no `Int` holds is nought, which one does.
            LoweredKernel::IntFloorMod => {
                known(vec![int(), int()], int(), vec![AbortKind::DivisionByZero])
            }
            LoweredKernel::ListLength => known(
                vec![Shape::List(Box::new(Shape::Var(0)))],
                int(),
                Vec::new(),
            ),
            // An index outside the list answers nothing and ends no run: that is what the language
            // answers an `Option` for.
            LoweredKernel::ListGet => known(
                vec![int(), Shape::List(Box::new(Shape::Var(0)))],
                Shape::Option(Box::new(Shape::Var(0))),
                Vec::new(),
            ),
            LoweredKernel::ListFind => known(
                vec![function(a(), bool()), list(a())],
                optional(a()),
                Vec::new(),
            ),
            // What is ordered is what the key answers, which is the one type the checker checked
            // the ordering against.
            LoweredKernel::ListSortBy => Contract {
                takes: vec![function(a(), b()), list(a())],
                answers: list(a()),
                fact: FactContract::OrderingSubject(b()),
                aborts: Vec::new(),
            },
            LoweredKernel::ListSort => Contract {
                takes: vec![list(a())],
                answers: list(a()),
                fact: FactContract::OrderingSubject(a()),
                aborts: Vec::new(),
            },
            // An empty list has no greatest element, which is what the `Option` is for.
            LoweredKernel::ListMax | LoweredKernel::ListMin => Contract {
                takes: vec![list(a())],
                answers: optional(a()),
                fact: FactContract::OrderingSubject(a()),
                aborts: Vec::new(),
            },
            LoweredKernel::ListReverse => known(vec![list(a())], list(a()), Vec::new()),
            // Which numbers are summed is the checker's to decide: the signature states no
            // constraint, and what this backend has a lowering for is asked where it is lowered.
            LoweredKernel::ListSum | LoweredKernel::ListProduct => known(
                vec![list(a())],
                a(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::ListRangeInclusive => known(
                vec![int(), int()],
                list(int()),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::OptionMap => known(
                vec![function(a(), b()), optional(a())],
                optional(b()),
                Vec::new(),
            ),
            // A zero divisor is a case of the answer and not a reason to end: that is what the
            // union says. What ends a quotient is the one pair whose quotient no `Int` holds, the
            // smallest `Int` over -1. The remainder of that pair is nought, which an `Int` holds,
            // so the remainder ends for nothing.
            LoweredKernel::IntTruncatingDivide => known(
                vec![int(), int()],
                int_or_division_by_zero(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::IntTruncatingRemainder => {
                known(vec![int(), int()], int_or_division_by_zero(), Vec::new())
            }
            LoweredKernel::StringLength => known(vec![string()], int(), Vec::new()),
            // Text that is no integer, or one no `Int` holds, is a case of the answer.
            LoweredKernel::StringToInt => known(vec![string()], int_or_not_a_number(), Vec::new()),
            LoweredKernel::StringFromInt => known(vec![int()], string(), Vec::new()),
            LoweredKernel::StringTrim => known(vec![string()], string(), Vec::new()),
            // A text longer than a string holds has no place, and these build one from what they
            // are given: a case mapping can be longer than its text, and a reversal is put in NFC.
            LoweredKernel::StringLowercase
            | LoweredKernel::StringUppercase
            | LoweredKernel::StringReverse => known(
                vec![string()],
                string(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::StringContains
            | LoweredKernel::StringStartsWith
            | LoweredKernel::StringEndsWith => known(vec![string(), string()], bool(), Vec::new()),
            LoweredKernel::StringMatches => Contract {
                takes: vec![string(), string()],
                answers: bool(),
                fact: FactContract::StringMatches,
                aborts: Vec::new(),
            },
            LoweredKernel::StringSlice => known(
                vec![int(), int(), string()],
                string(),
                vec![AbortKind::InvalidBounds],
            ),
            LoweredKernel::StringAppend => known(
                vec![string(), string()],
                string(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::StringSplit => known(vec![string(), string()], strings(), Vec::new()),
            LoweredKernel::StringJoin => known(
                vec![string(), strings()],
                string(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::StringConcat => known(
                vec![strings()],
                string(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::StringReplace => known(
                vec![string(), string(), string()],
                string(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::StringWords
            | LoweredKernel::StringLines
            | LoweredKernel::StringCharacters => known(vec![string()], strings(), Vec::new()),
            LoweredKernel::StringCodePoints => {
                known(vec![string()], Shape::List(Box::new(int())), Vec::new())
            }
            // Copies or a fill that are more text than a string holds end the run rather than
            // answering fewer copies than were asked for.
            LoweredKernel::StringRepeat => known(
                vec![int(), string()],
                string(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::StringPadLeft | LoweredKernel::StringPadRight => known(
                vec![int(), string(), string()],
                string(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            // Text that is no decimal text is a case of the answer.
            LoweredKernel::StringToDecimal => {
                known(vec![string()], decimal_or_not_a_number(), Vec::new())
            }
            // Plain notation is as long as the scale is far from nought, so the text of a value near
            // either end of the scale range is one no string holds.
            LoweredKernel::StringFromDecimal => known(
                vec![decimal()],
                string(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            // A result whose scale leaves the range, or that is wider than a `Decimal` holds, ends
            // the run where it is computed.
            LoweredKernel::DecimalAdd
            | LoweredKernel::DecimalSubtract
            | LoweredKernel::DecimalMultiply => known(
                vec![decimal(), decimal()],
                decimal(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::DecimalCompare => known(vec![decimal(), decimal()], int(), Vec::new()),
            LoweredKernel::DecimalFromInt => known(vec![int()], decimal(), Vec::new()),
            LoweredKernel::DecimalToInt => known(
                vec![mode(), decimal()],
                int(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::DecimalRound => known(
                vec![int(), mode(), decimal()],
                decimal(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            // A zero divisor is a case of the answer, answered before the scale is looked at; a
            // scale outside the range, or a quotient no `Decimal` holds at it, ends the run.
            LoweredKernel::DecimalDivide => known(
                vec![decimal(), decimal(), int(), mode()],
                decimal_or_division_by_zero(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            // Every `Int` and every `Decimal` has one exact value, so neither way in ends a run
            // and neither states a policy.
            LoweredKernel::RationalFromInt => known(vec![int()], rational(), Vec::new()),
            LoweredKernel::RationalFromDecimal => known(vec![decimal()], rational(), Vec::new()),
            // What a carrier does not hold is a case and not a value chosen for it; a value it does
            // not hold for want of room ends the run.
            LoweredKernel::RationalToWholeNumber => known(
                vec![rational()],
                int_or_not_whole(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::RationalToFiniteDecimal => known(
                vec![rational()],
                decimal_or_not_a_finite_decimal(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::RationalToInt => known(
                vec![mode(), rational()],
                int(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::RationalToDecimal => known(
                vec![int(), mode(), rational()],
                decimal(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            // The exact sum always has a value, but writing it brings a common exponent out and
            // builds the distance between the two, which can want more than a `Rational` holds.
            LoweredKernel::RationalAdd
            | LoweredKernel::RationalSubtract
            | LoweredKernel::RationalMultiply => known(
                vec![rational(), rational()],
                rational(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            // A zero divisor ends the run outright, as `/` does, and so does a quotient with no
            // place: the one kernel that ends for two reasons.
            LoweredKernel::RationalDivide => known(
                vec![rational(), rational()],
                rational(),
                vec![AbortKind::DivisionByZero, AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::RationalCompare => {
                known(vec![rational(), rational()], int(), Vec::new())
            }
            // A shift off the end of what a temporal holds ends the run, as an `Int` overflow does
            // (spec §a-shift-off-the-end-of-a-temporal-aborts).
            LoweredKernel::DateAddDays
            | LoweredKernel::DateAddMonths
            | LoweredKernel::DateAddYears => known(
                vec![int(), date()],
                date(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::DateTimeAddMinutes
            | LoweredKernel::DateTimeAddHours
            | LoweredKernel::DateTimeAddDays => known(
                vec![int(), datetime()],
                datetime(),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::DateDaysBetween => known(vec![date(), date()], int(), Vec::new()),
            LoweredKernel::DateYear | LoweredKernel::DateMonth | LoweredKernel::DateDay => {
                known(vec![date()], int(), Vec::new())
            }
            // Parts that name none are a case of the answer, and end no run.
            LoweredKernel::DateFromParts => known(
                vec![int(), int(), int()],
                Shape::Cases(vec![
                    Case::Primitive { prim: Prim::Date },
                    Case::Language {
                        case: LanguageCase::NotADate,
                    },
                ]),
                Vec::new(),
            ),
            LoweredKernel::TimeFromParts => known(
                vec![int(), int(), int()],
                // In the order the checker writes this union in, which is by name: `NotATime` comes
                // before `Time`, where `Date` comes before `NotADate`.
                Shape::Cases(vec![
                    Case::Language {
                        case: LanguageCase::NotATime,
                    },
                    Case::Primitive { prim: Prim::Time },
                ]),
                Vec::new(),
            ),
            LoweredKernel::TimeHour | LoweredKernel::TimeMinute | LoweredKernel::TimeSecond => {
                known(vec![time()], int(), Vec::new())
            }
            LoweredKernel::DateTimeMinutesBetween => {
                known(vec![datetime(), datetime()], int(), Vec::new())
            }
            LoweredKernel::DateTimeToDate => known(vec![datetime()], date(), Vec::new()),
            LoweredKernel::DateTimeToTime => known(vec![datetime()], time(), Vec::new()),
            LoweredKernel::DateTimeFromDateAndTime => {
                known(vec![date(), time()], datetime(), Vec::new())
            }
            // What neither takes anything to bind is bound by what the call is settled as.
            LoweredKernel::SetEmpty => known(Vec::new(), set(a()), Vec::new()),
            LoweredKernel::SetSingleton => known(vec![a()], set(a()), Vec::new()),
            // One more member than a set holds has no place (upstream `KernelContracts`), and a
            // set built from one member, or from a list, never is.
            LoweredKernel::SetInsert => known(
                vec![a(), set(a())],
                set(a()),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::SetRemove => known(vec![a(), set(a())], set(a()), Vec::new()),
            LoweredKernel::SetContains => known(vec![a(), set(a())], bool(), Vec::new()),
            LoweredKernel::SetUnion => known(
                vec![set(a()), set(a())],
                set(a()),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::SetIntersection | LoweredKernel::SetDifference => {
                known(vec![set(a()), set(a())], set(a()), Vec::new())
            }
            LoweredKernel::SetIsEmpty => known(vec![set(a())], bool(), Vec::new()),
            LoweredKernel::SetSize => known(vec![set(a())], int(), Vec::new()),
            LoweredKernel::SetToList => known(vec![set(a())], list(a()), Vec::new()),
            LoweredKernel::SetFromList => known(vec![list(a())], set(a()), Vec::new()),
            LoweredKernel::MapEmpty => known(Vec::new(), map(a(), b()), Vec::new()),
            LoweredKernel::MapGet => known(vec![a(), map(a(), b())], optional(b()), Vec::new()),
            LoweredKernel::MapContainsKey => known(vec![a(), map(a(), b())], bool(), Vec::new()),
            LoweredKernel::MapKeys => known(vec![map(a(), b())], list(a()), Vec::new()),
            LoweredKernel::MapValues => known(vec![map(a(), b())], list(b()), Vec::new()),
            LoweredKernel::MapSingleton => known(vec![a(), b()], map(a(), b()), Vec::new()),
            LoweredKernel::MapInsert => known(
                vec![a(), b(), map(a(), b())],
                map(a(), b()),
                vec![AbortKind::RequiredFormHasNoPlace],
            ),
            LoweredKernel::MapRemove => known(vec![a(), map(a(), b())], map(a(), b()), Vec::new()),
            LoweredKernel::MapIsEmpty => known(vec![map(a(), b())], bool(), Vec::new()),
            LoweredKernel::MapSize => known(vec![map(a(), b())], int(), Vec::new()),
            LoweredKernel::MapToList => known(vec![map(a(), b())], list(pair()), Vec::new()),
            LoweredKernel::MapFromList => known(vec![list(pair())], map(a(), b()), Vec::new()),
        }
    }
}

/// The key of `RoundingMode`, which the language declares in `souther.decimal`.
pub(crate) const ROUNDING_MODE: &str = "souther.decimal.RoundingMode";

/// What reading decimal text answers, its members in the order the checker writes this union in.
fn decimal_or_not_a_number() -> Shape {
    Shape::Cases(vec![
        Case::Primitive {
            prim: Prim::Decimal,
        },
        Case::Language {
            case: LanguageCase::NotANumber,
        },
    ])
}

/// What `Rational.toWholeNumber` answers, its members in the order its declaration writes this
/// union in.
fn int_or_not_whole() -> Shape {
    Shape::Cases(vec![
        Case::Primitive { prim: Prim::Int },
        Case::Language {
            case: LanguageCase::NotWhole,
        },
    ])
}

/// What `Rational.toFiniteDecimal` answers, its members in the order its declaration writes this
/// union in.
fn decimal_or_not_a_finite_decimal() -> Shape {
    Shape::Cases(vec![
        Case::Primitive {
            prim: Prim::Decimal,
        },
        Case::Language {
            case: LanguageCase::NotAFiniteDecimal,
        },
    ])
}

/// What `Decimal.divide` answers, its members in the order the checker writes this union in.
fn decimal_or_division_by_zero() -> Shape {
    Shape::Cases(vec![
        Case::Primitive {
            prim: Prim::Decimal,
        },
        Case::Language {
            case: LanguageCase::DivisionByZero,
        },
    ])
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

/// What reading integer text answers, its members in the order the checker writes this union in,
/// which is the order `String.toInt`'s declaration writes it in.
fn int_or_not_a_number() -> Shape {
    Shape::Cases(vec![
        Case::Primitive { prim: Prim::Int },
        Case::Language {
            case: LanguageCase::NotANumber,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::PatternPart;

    fn pattern(text: &str) -> KernelFact {
        KernelFact::StringMatches {
            written: text.to_string(),
            meaning: vec![PatternPart::Nothing],
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
                ordering: Some(Ty::Prim { prim: ty }),
            };
            assert!(FactContract::OrderingSubject(Shape::Var(0)).accepts(&fact));
            assert!(!FactContract::StringMatches.accepts(&fact));
            assert!(!FactContract::None.accepts(&fact));
        }
    }

    const LOWERED: [(&str, LoweredKernel); 107] = [
        ("int.add", LoweredKernel::IntAdd),
        ("int.subtract", LoweredKernel::IntSubtract),
        ("int.multiply", LoweredKernel::IntMultiply),
        ("int.compare", LoweredKernel::IntCompare),
        ("int.floorMod", LoweredKernel::IntFloorMod),
        ("int.truncatingDivide", LoweredKernel::IntTruncatingDivide),
        (
            "int.truncatingRemainder",
            LoweredKernel::IntTruncatingRemainder,
        ),
        ("list.length", LoweredKernel::ListLength),
        ("list.get", LoweredKernel::ListGet),
        ("list.find", LoweredKernel::ListFind),
        ("list.sortBy", LoweredKernel::ListSortBy),
        ("list.sort", LoweredKernel::ListSort),
        ("list.max", LoweredKernel::ListMax),
        ("list.min", LoweredKernel::ListMin),
        ("list.reverse", LoweredKernel::ListReverse),
        ("list.sum", LoweredKernel::ListSum),
        ("list.product", LoweredKernel::ListProduct),
        ("list.rangeInclusive", LoweredKernel::ListRangeInclusive),
        ("option.map", LoweredKernel::OptionMap),
        ("string.length", LoweredKernel::StringLength),
        ("string.toInt", LoweredKernel::StringToInt),
        ("string.fromInt", LoweredKernel::StringFromInt),
        ("string.trim", LoweredKernel::StringTrim),
        ("string.lowercase", LoweredKernel::StringLowercase),
        ("string.uppercase", LoweredKernel::StringUppercase),
        ("string.contains", LoweredKernel::StringContains),
        ("string.startsWith", LoweredKernel::StringStartsWith),
        ("string.endsWith", LoweredKernel::StringEndsWith),
        ("string.matches", LoweredKernel::StringMatches),
        ("string.slice", LoweredKernel::StringSlice),
        ("string.append", LoweredKernel::StringAppend),
        ("string.split", LoweredKernel::StringSplit),
        ("string.join", LoweredKernel::StringJoin),
        ("string.concat", LoweredKernel::StringConcat),
        ("string.replace", LoweredKernel::StringReplace),
        ("string.words", LoweredKernel::StringWords),
        ("string.lines", LoweredKernel::StringLines),
        ("string.reverse", LoweredKernel::StringReverse),
        ("string.repeat", LoweredKernel::StringRepeat),
        ("string.padLeft", LoweredKernel::StringPadLeft),
        ("string.padRight", LoweredKernel::StringPadRight),
        ("string.characters", LoweredKernel::StringCharacters),
        ("string.codePoints", LoweredKernel::StringCodePoints),
        ("string.toDecimal", LoweredKernel::StringToDecimal),
        ("string.fromDecimal", LoweredKernel::StringFromDecimal),
        ("decimal.add", LoweredKernel::DecimalAdd),
        ("decimal.subtract", LoweredKernel::DecimalSubtract),
        ("decimal.multiply", LoweredKernel::DecimalMultiply),
        ("decimal.compare", LoweredKernel::DecimalCompare),
        ("decimal.fromInt", LoweredKernel::DecimalFromInt),
        ("decimal.toInt", LoweredKernel::DecimalToInt),
        ("decimal.round", LoweredKernel::DecimalRound),
        ("decimal.divide", LoweredKernel::DecimalDivide),
        ("rational.fromInt", LoweredKernel::RationalFromInt),
        ("rational.fromDecimal", LoweredKernel::RationalFromDecimal),
        (
            "rational.toWholeNumber",
            LoweredKernel::RationalToWholeNumber,
        ),
        (
            "rational.toFiniteDecimal",
            LoweredKernel::RationalToFiniteDecimal,
        ),
        ("rational.toInt", LoweredKernel::RationalToInt),
        ("rational.toDecimal", LoweredKernel::RationalToDecimal),
        ("rational.add", LoweredKernel::RationalAdd),
        ("rational.subtract", LoweredKernel::RationalSubtract),
        ("rational.multiply", LoweredKernel::RationalMultiply),
        ("rational.divide", LoweredKernel::RationalDivide),
        ("rational.compare", LoweredKernel::RationalCompare),
        ("date.addDays", LoweredKernel::DateAddDays),
        ("date.addMonths", LoweredKernel::DateAddMonths),
        ("date.addYears", LoweredKernel::DateAddYears),
        ("date.daysBetween", LoweredKernel::DateDaysBetween),
        ("date.year", LoweredKernel::DateYear),
        ("date.month", LoweredKernel::DateMonth),
        ("date.day", LoweredKernel::DateDay),
        ("date.fromParts", LoweredKernel::DateFromParts),
        ("time.fromParts", LoweredKernel::TimeFromParts),
        ("time.hour", LoweredKernel::TimeHour),
        ("time.minute", LoweredKernel::TimeMinute),
        ("time.second", LoweredKernel::TimeSecond),
        ("datetime.addMinutes", LoweredKernel::DateTimeAddMinutes),
        ("datetime.addHours", LoweredKernel::DateTimeAddHours),
        ("datetime.addDays", LoweredKernel::DateTimeAddDays),
        (
            "datetime.minutesBetween",
            LoweredKernel::DateTimeMinutesBetween,
        ),
        ("datetime.toDate", LoweredKernel::DateTimeToDate),
        ("datetime.toTime", LoweredKernel::DateTimeToTime),
        (
            "datetime.fromDateAndTime",
            LoweredKernel::DateTimeFromDateAndTime,
        ),
        ("set.empty", LoweredKernel::SetEmpty),
        ("set.singleton", LoweredKernel::SetSingleton),
        ("set.insert", LoweredKernel::SetInsert),
        ("set.remove", LoweredKernel::SetRemove),
        ("set.contains", LoweredKernel::SetContains),
        ("set.union", LoweredKernel::SetUnion),
        ("set.intersection", LoweredKernel::SetIntersection),
        ("set.difference", LoweredKernel::SetDifference),
        ("set.isEmpty", LoweredKernel::SetIsEmpty),
        ("set.size", LoweredKernel::SetSize),
        ("set.toList", LoweredKernel::SetToList),
        ("set.fromList", LoweredKernel::SetFromList),
        ("map.empty", LoweredKernel::MapEmpty),
        ("map.get", LoweredKernel::MapGet),
        ("map.containsKey", LoweredKernel::MapContainsKey),
        ("map.keys", LoweredKernel::MapKeys),
        ("map.values", LoweredKernel::MapValues),
        ("map.singleton", LoweredKernel::MapSingleton),
        ("map.insert", LoweredKernel::MapInsert),
        ("map.remove", LoweredKernel::MapRemove),
        ("map.isEmpty", LoweredKernel::MapIsEmpty),
        ("map.size", LoweredKernel::MapSize),
        ("map.toList", LoweredKernel::MapToList),
        ("map.fromList", LoweredKernel::MapFromList),
    ];

    /// `String.matches` settles what its pattern means, and the kernels that order settle what
    /// they order by: the element of the list for `sort`, `max` and `min`, and what the key answers
    /// for `sortBy`. No other kernel lowered here settles anything beside what it takes.
    #[test]
    fn a_kernel_that_orders_settles_what_it_orders_by_and_no_other_does() {
        for (key, kernel) in LOWERED {
            let settled = match kernel {
                LoweredKernel::StringMatches => FactContract::StringMatches,
                LoweredKernel::ListSort | LoweredKernel::ListMax | LoweredKernel::ListMin => {
                    FactContract::OrderingSubject(Shape::Var(0))
                }
                LoweredKernel::ListSortBy => FactContract::OrderingSubject(Shape::Var(1)),
                _ => FactContract::None,
            };
            assert_eq!(kernel.contract().fact, settled, "{key}");
        }
    }

    /// What a sort orders by is what what it takes bound: the key's answer for `sortBy`, and not
    /// the element it hands the key.
    #[test]
    fn an_ordering_subject_is_what_the_kernel_takes_bound_it_to() {
        let int = Ty::Prim { prim: Prim::Int };
        let string = Ty::Prim { prim: Prim::String };
        let contract = LoweredKernel::ListSortBy.contract();
        let mut bound = Bound::default();
        let key = Ty::Fn {
            fn_: FnSignature {
                takes: vec![int.clone()],
                answers: Box::new(string.clone()),
            },
        };
        let ints = Ty::List {
            list: Box::new(int),
        };
        assert!(contract.takes[0].binds(&key, &mut bound));
        assert!(contract.takes[1].binds(&ints, &mut bound));
        assert_eq!(contract.fact.holds(&bound), Some(string));
        assert_eq!(FactContract::None.holds(&bound), None);
    }

    /// A function is of a function's shape where it takes as many as the shape does, each of the
    /// shape it is taken at, and answers what the shape answers; its variables are the ones the
    /// rest of what a kernel takes is held to.
    #[test]
    fn a_function_binds_what_it_takes_and_answers() {
        let int = Ty::Prim { prim: Prim::Int };
        let bool = Ty::Prim { prim: Prim::Bool };
        let function = |takes: Vec<Ty>, answers: &Ty| Ty::Fn {
            fn_: FnSignature {
                takes,
                answers: Box::new(answers.clone()),
            },
        };
        let predicate = Shape::Fn {
            takes: vec![Shape::Var(0)],
            answers: Box::new(Shape::Prim(Prim::Bool)),
        };
        let mut bound = Bound::default();
        assert!(predicate.binds(&function(vec![int.clone()], &bool), &mut bound));
        assert!(!Shape::List(Box::new(Shape::Var(0))).binds(
            &Ty::List {
                list: Box::new(bool.clone())
            },
            &mut bound
        ));
        assert_eq!(
            predicate.settled(&bound),
            Some(function(vec![int.clone()], &bool))
        );
        let mut bound = Bound::default();
        assert!(!predicate.binds(&function(vec![int.clone()], &int), &mut bound));
        assert!(!predicate.binds(&function(vec![int.clone(), int.clone()], &bool), &mut bound));
        assert!(!predicate.binds(&int, &mut Bound::default()));
    }

    /// Each key reaches its own kernel, and a key the language does not write reaches none: this
    /// backend does not accept a name the standard library has no declaration for. A kernel over a
    /// `Rational` is not lowered here yet.
    #[test]
    fn a_key_reaches_the_kernel_it_names_and_no_other() {
        for (key, kernel) in LOWERED {
            assert_eq!(LoweredKernel::of(key), Some(kernel));
        }
        assert_eq!(LoweredKernel::of("int.divide"), None);
        assert_eq!(LoweredKernel::of("decimal.abs"), None);
        assert_eq!(LoweredKernel::of("rational.abs"), None);
    }

    /// A kernel that can end a run ends it for one reason, so what it hands back says only whether
    /// it answered and the reason is read off the contract.
    #[test]
    fn a_kernel_ends_a_run_for_one_reason_at_most() {
        for (key, kernel) in LOWERED {
            // `Rational.divide` is the one that ends for two, as the operator it stands behind does.
            let most = if kernel == LoweredKernel::RationalDivide {
                2
            } else {
                1
            };
            assert!(kernel.contract().aborts.len() <= most, "{key}");
        }
    }

    fn some_shape_of(kernel: LoweredKernel) -> Vec<Ty> {
        let int = Ty::Prim { prim: Prim::Int };
        let string = Ty::Prim { prim: Prim::String };
        let decimal = Ty::Prim {
            prim: Prim::Decimal,
        };
        let rational = Ty::Prim {
            prim: Prim::Rational,
        };
        let mode = Ty::declared(ROUNDING_MODE.to_string());
        let date = Ty::Prim { prim: Prim::Date };
        let time = Ty::Prim { prim: Prim::Time };
        let datetime = Ty::Prim {
            prim: Prim::DateTime,
        };
        let strings = Ty::List {
            list: Box::new(string.clone()),
        };
        let bools = Ty::List {
            list: Box::new(Ty::Prim { prim: Prim::Bool }),
        };
        match kernel {
            LoweredKernel::IntAdd
            | LoweredKernel::IntSubtract
            | LoweredKernel::IntMultiply
            | LoweredKernel::IntCompare
            | LoweredKernel::IntFloorMod
            | LoweredKernel::IntTruncatingDivide
            | LoweredKernel::IntTruncatingRemainder => vec![int.clone(), int],
            LoweredKernel::ListLength
            | LoweredKernel::ListSort
            | LoweredKernel::ListMax
            | LoweredKernel::ListMin
            | LoweredKernel::ListReverse => vec![bools],
            LoweredKernel::ListSum | LoweredKernel::ListProduct => vec![Ty::List {
                list: Box::new(int),
            }],
            LoweredKernel::ListGet => vec![int, bools],
            LoweredKernel::ListRangeInclusive => vec![int.clone(), int],
            LoweredKernel::ListFind | LoweredKernel::ListSortBy => {
                let key = Ty::Fn {
                    fn_: FnSignature {
                        takes: vec![Ty::Prim { prim: Prim::Bool }],
                        answers: Box::new(Ty::Prim { prim: Prim::Bool }),
                    },
                };
                vec![key, bools]
            }
            LoweredKernel::OptionMap => {
                let function = Ty::Fn {
                    fn_: FnSignature {
                        takes: vec![int.clone()],
                        answers: Box::new(string),
                    },
                };
                vec![
                    function,
                    Ty::Option {
                        option: Box::new(int),
                    },
                ]
            }
            LoweredKernel::StringLength
            | LoweredKernel::StringToInt
            | LoweredKernel::StringTrim
            | LoweredKernel::StringLowercase
            | LoweredKernel::StringUppercase
            | LoweredKernel::StringReverse
            | LoweredKernel::StringWords
            | LoweredKernel::StringLines
            | LoweredKernel::StringCharacters
            | LoweredKernel::StringCodePoints => vec![string],
            LoweredKernel::StringFromInt => vec![int],
            LoweredKernel::StringContains
            | LoweredKernel::StringStartsWith
            | LoweredKernel::StringEndsWith
            | LoweredKernel::StringMatches
            | LoweredKernel::StringAppend
            | LoweredKernel::StringSplit => vec![string.clone(), string],
            LoweredKernel::StringJoin => vec![string, strings],
            LoweredKernel::StringConcat => vec![strings],
            LoweredKernel::StringReplace => vec![string.clone(), string.clone(), string],
            LoweredKernel::StringSlice => vec![int.clone(), int, string],
            LoweredKernel::StringRepeat => vec![int, string],
            LoweredKernel::StringPadLeft | LoweredKernel::StringPadRight => {
                vec![int, string.clone(), string]
            }
            LoweredKernel::StringToDecimal => vec![string],
            LoweredKernel::StringFromDecimal => vec![decimal],
            LoweredKernel::DecimalAdd
            | LoweredKernel::DecimalSubtract
            | LoweredKernel::DecimalMultiply
            | LoweredKernel::DecimalCompare => vec![decimal.clone(), decimal],
            LoweredKernel::DecimalFromInt => vec![int],
            LoweredKernel::DecimalToInt => vec![mode, decimal],
            LoweredKernel::DecimalRound => vec![int, mode, decimal],
            LoweredKernel::DecimalDivide => vec![decimal.clone(), decimal, int, mode],
            LoweredKernel::RationalFromInt => vec![int],
            LoweredKernel::RationalFromDecimal => vec![decimal],
            LoweredKernel::RationalToWholeNumber | LoweredKernel::RationalToFiniteDecimal => {
                vec![rational]
            }
            LoweredKernel::RationalToInt => vec![mode, rational],
            LoweredKernel::RationalToDecimal => vec![int, mode, rational],
            LoweredKernel::RationalAdd
            | LoweredKernel::RationalSubtract
            | LoweredKernel::RationalMultiply
            | LoweredKernel::RationalDivide
            | LoweredKernel::RationalCompare => vec![rational.clone(), rational],
            LoweredKernel::DateAddDays
            | LoweredKernel::DateAddMonths
            | LoweredKernel::DateAddYears => vec![int, date],
            LoweredKernel::DateTimeAddMinutes
            | LoweredKernel::DateTimeAddHours
            | LoweredKernel::DateTimeAddDays => vec![int, datetime],
            LoweredKernel::DateDaysBetween => vec![date.clone(), date],
            LoweredKernel::DateYear | LoweredKernel::DateMonth | LoweredKernel::DateDay => {
                vec![date]
            }
            LoweredKernel::DateFromParts | LoweredKernel::TimeFromParts => {
                vec![int.clone(), int.clone(), int]
            }
            LoweredKernel::TimeHour | LoweredKernel::TimeMinute | LoweredKernel::TimeSecond => {
                vec![time]
            }
            LoweredKernel::DateTimeMinutesBetween => vec![datetime.clone(), datetime],
            LoweredKernel::DateTimeToDate | LoweredKernel::DateTimeToTime => vec![datetime],
            LoweredKernel::DateTimeFromDateAndTime => vec![date, time],
            LoweredKernel::SetEmpty | LoweredKernel::MapEmpty => Vec::new(),
            LoweredKernel::SetSingleton => vec![int],
            LoweredKernel::SetInsert | LoweredKernel::SetRemove | LoweredKernel::SetContains => {
                vec![int.clone(), ints_set(int)]
            }
            LoweredKernel::SetUnion
            | LoweredKernel::SetIntersection
            | LoweredKernel::SetDifference => vec![ints_set(int.clone()), ints_set(int)],
            LoweredKernel::SetIsEmpty | LoweredKernel::SetSize | LoweredKernel::SetToList => {
                vec![ints_set(int)]
            }
            LoweredKernel::SetFromList => vec![Ty::List {
                list: Box::new(int),
            }],
            LoweredKernel::MapGet | LoweredKernel::MapContainsKey | LoweredKernel::MapRemove => {
                vec![string.clone(), counts(string, int)]
            }
            LoweredKernel::MapKeys
            | LoweredKernel::MapValues
            | LoweredKernel::MapIsEmpty
            | LoweredKernel::MapSize
            | LoweredKernel::MapToList => vec![counts(string, int)],
            LoweredKernel::MapSingleton => vec![string, int],
            LoweredKernel::MapInsert => vec![string.clone(), int.clone(), counts(string, int)],
            LoweredKernel::MapFromList => vec![Ty::List {
                list: Box::new(Ty::Tuple {
                    tuple: vec![string, int],
                }),
            }],
        }
    }

    fn ints_set(int: Ty) -> Ty {
        Ty::Set { set: Box::new(int) }
    }

    fn counts(key: Ty, value: Ty) -> Ty {
        Ty::Map {
            map: MapTy {
                key: Box::new(key),
                value: Box::new(value),
            },
        }
    }

    /// The checker writes a union's members by name, so a contract naming the members of one in
    /// another order would be read as a union the checker never wrote. Each answer of this kind was
    /// written by hand, and `Time | NotATime` is `NotATime | Time` for that reason: held here for
    /// every kernel and not remembered at each.
    #[test]
    fn a_union_a_kernel_answers_lists_its_members_by_name() {
        for (key, kernel) in LOWERED {
            let Shape::Cases(cases) = kernel.contract().answers else {
                continue;
            };
            let names: Vec<String> = cases
                .iter()
                .map(|case| match case {
                    Case::Primitive { prim } => prim.spelt().to_string(),
                    Case::Language { case } => case.spelt().to_string(),
                    Case::Declared { declared } => declared.to_string(),
                })
                .collect();
            let mut by_name = names.clone();
            by_name.sort();
            assert_eq!(names, by_name, "{key}");
        }
    }

    /// What a kernel answers is settled by what it takes: a variable in its answer that nothing it
    /// takes binds would leave the answer unknown however a call is settled. A kernel that takes
    /// nothing is the one exception, an empty collection, and what the call is typed binds the whole
    /// of what it answers (`Coherent`).
    #[test]
    fn what_a_kernel_takes_settles_what_it_answers() {
        for (key, kernel) in LOWERED {
            let contract = kernel.contract();
            let mut bound = Bound::default();
            for (shape, ty) in contract.takes.iter().zip(some_shape_of(kernel)) {
                assert!(shape.binds(&ty, &mut bound), "{key}");
            }
            if contract.takes.is_empty() {
                assert!(
                    matches!(kernel, LoweredKernel::SetEmpty | LoweredKernel::MapEmpty),
                    "{key}"
                );
                continue;
            }
            assert!(contract.answers.settled(&bound).is_some(), "{key}");
            // So is the type a fact of it has to hold, where the kind carries one.
            assert_eq!(
                contract.fact.holds(&bound).is_some(),
                matches!(contract.fact, FactContract::OrderingSubject(_)),
                "{key}"
            );
        }
    }

    /// A function a kernel takes takes what the list or the optional beside it holds: each of its
    /// parameters is a variable that is the element of one of those. So a function that never runs,
    /// one taking the type of what has no value, is handed beside a list or an optional that holds
    /// nothing, and the lowering answers for nothing without calling it (`lists`). A kernel handing
    /// its function a value of its own would have a contract this refuses.
    #[test]
    fn a_function_a_kernel_takes_takes_what_it_is_handed_beside_it() {
        for (key, kernel) in LOWERED {
            let contract = kernel.contract();
            let held: Vec<&Shape> = contract
                .takes
                .iter()
                .filter_map(|shape| match shape {
                    Shape::List(element) | Shape::Option(element) => Some(&**element),
                    _ => None,
                })
                .collect();
            for shape in &contract.takes {
                let Shape::Fn { takes, .. } = shape else {
                    continue;
                };
                for taken in takes {
                    assert!(
                        matches!(taken, Shape::Var(_)) && held.contains(&taken),
                        "{key}"
                    );
                }
            }
        }
    }

    /// A zero divisor is answered, so it is a case of what the division answers and never a reason
    /// it ends; and the cases are what the division answers whatever it is handed.
    #[test]
    fn a_zero_divisor_is_a_case_of_the_answer_and_not_an_abort() {
        let answer = Ty::Union {
            union: Cases::one_or_more(vec![
                Case::Language {
                    case: LanguageCase::DivisionByZero,
                },
                Case::Primitive { prim: Prim::Int },
            ])
            .unwrap(),
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
                union: Cases::one_or_more(vec![
                    Case::Primitive { prim: Prim::Int },
                    Case::Language {
                        case: LanguageCase::DivisionByZero,
                    },
                ])
                .unwrap(),
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
