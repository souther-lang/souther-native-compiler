//! What `souther_native_abi` says every function here takes and answers, held to the functions.
//!
//! Two tables say it: [`HOST_RUNTIME`], what a header declares and a library exports, and
//! [`GENERATED_RUNTIME`], what the driver declares each function as where generated code calls it.
//! Nothing else checks either against the functions: a C compiler trusts a header, Cranelift trusts
//! a signature, and a linker resolves a name whatever it was declared as. So each function is
//! written here as the pointer type it is, which compiles only while that is what the function is,
//! and every parameter and answer is read off that type as the word it stands for.
//!
//! One Rust type is one word. An address of text is a [`Text`], of a value a [`Value`], and a count
//! a [`Count`], and not all of them an address or an `i64`, so a table saying one where the function
//! takes the other is caught here and not by a host reading the wrong thing.

use crate::decimal::*;
use crate::decoding::*;
use crate::document::Node;
use crate::external::*;
use crate::rational::*;
use crate::temporal::*;
use crate::*;
use souther_native_abi::{
    BUILT_IN_CASES, GENERATED_RUNTIME, HOST_CASES, HOST_RUNTIME, HostWord, Parameter,
    RuntimeFunction, Word,
};
use std::collections::BTreeSet;

/// A type a function here takes, as the parameter it is.
trait Taken {
    const PARAMETER: Parameter;
}

/// A type a function here answers, as the word it is.
trait Answered {
    const WORD: Word;
}

/// Types that are one word, whether handed over or answered.
macro_rules! words {
    ($($ty:ty => $word:expr),* $(,)?) => {
        $(
            impl Taken for $ty {
                const PARAMETER: Parameter = Parameter::Given($word);
            }
            impl Answered for $ty {
                const WORD: Word = $word;
            }
        )*
    };
}

/// Types that are room for a word, written by the function.
macro_rules! rooms {
    ($($ty:ty => $word:expr),* $(,)?) => {
        $(
            impl Taken for $ty {
                const PARAMETER: Parameter = Parameter::Room($word);
            }
        )*
    };
}

words! {
    i8 => Word::Host(HostWord::Bool),
    i32 => Word::Host(HostWord::Outcome),
    i64 => Word::Host(HostWord::Int),
    Count => Word::Host(HostWord::Count),
    Scope => Word::Host(HostWord::Scope),
    Comparison => Word::Comparison,
    *const u8 => Word::Host(HostWord::Bytes),
    *mut u8 => Word::Memory,
    *const Text => Word::Host(HostWord::String),
    *const Decimal => Word::Host(HostWord::Decimal),
    *mut Decimal => Word::Host(HostWord::Decimal),
    *const Rational => Word::Rational,
    *mut Rational => Word::Rational,
    *const Date => Word::Host(HostWord::Date),
    *mut Date => Word::Host(HostWord::Date),
    *const Time => Word::Host(HostWord::Time),
    *mut Time => Word::Host(HostWord::Time),
    *const DateTime => Word::Host(HostWord::DateTime),
    *mut DateTime => Word::Host(HostWord::DateTime),
    *const Instant => Word::Host(HostWord::Instant),
    *mut Instant => Word::Host(HostWord::Instant),
    *mut Text => Word::Host(HostWord::String),
    *const Value => Word::Host(HostWord::Value),
    *const List => Word::Host(HostWord::List),
    *mut List => Word::Host(HostWord::List),
    *const u32 => Word::Machine,
    *const Decoding => Word::Host(HostWord::Decoded),
    *mut Decoding => Word::Host(HostWord::Decoded),
    *const Issue => Word::Host(HostWord::Issue),
    *mut Form => Word::Form,
    *const Node => Word::Node,
    *const Path => Word::Path,
    *const Set => Word::Set,
    *mut Set => Word::Set,
    *const Map => Word::Map,
    *mut Map => Word::Map,
    *const HeldAt => Word::Held,
    Hash => Word::Hash,
    Hasher => Word::Hasher,
    Equality => Word::Equality,
}

rooms! {
    *mut i64 => Word::Host(HostWord::Int),
    *mut i8 => Word::Host(HostWord::Bool),
    *mut *mut Text => Word::Host(HostWord::String),
    *mut *mut Decimal => Word::Host(HostWord::Decimal),
    *mut *mut Rational => Word::Rational,
    *mut *mut Date => Word::Host(HostWord::Date),
    *mut *mut Time => Word::Host(HostWord::Time),
    *mut *mut DateTime => Word::Host(HostWord::DateTime),
    *mut *mut Instant => Word::Host(HostWord::Instant),
    *mut *const Set => Word::Set,
    *mut *const Map => Word::Map,
}

/// What a function takes and answers.
type Shape = (Vec<Parameter>, Option<Word>);

/// A function pointer as what it takes and answers.
trait Shaped {
    fn shape() -> Shape;
}

macro_rules! shaped {
    ($($taken:ident),*) => {
        impl<$($taken: Taken,)* R: Answered> Shaped for unsafe extern "C" fn($($taken),*) -> R {
            fn shape() -> Shape {
                (vec![$($taken::PARAMETER),*], Some(R::WORD))
            }
        }
        impl<$($taken: Taken,)* R: Answered> Shaped for extern "C" fn($($taken),*) -> R {
            fn shape() -> Shape {
                (vec![$($taken::PARAMETER),*], Some(R::WORD))
            }
        }
        impl<$($taken: Taken),*> Shaped for unsafe extern "C" fn($($taken),*) {
            fn shape() -> Shape {
                (vec![$($taken::PARAMETER),*], None)
            }
        }
        impl<$($taken: Taken),*> Shaped for extern "C" fn($($taken),*) {
            fn shape() -> Shape {
                (vec![$($taken::PARAMETER),*], None)
            }
        }
    };
}

shaped!();
shaped!(A);
shaped!(A, B);
shaped!(A, B, C);
shaped!(A, B, C, D);
shaped!(A, B, C, D, E);
shaped!(A, B, C, D, E, F);
shaped!(A, B, C, D, E, F, G);

fn shape_of<F: Shaped>(_: F) -> Shape {
    F::shape()
}

/// Every function here, as the type it is. A cast from a function to a pointer of another type does
/// not compile, so what is written here is what each function is.
fn functions() -> Vec<(&'static str, Shape)> {
    type T = *const Text;
    type M = *mut Text;
    type D = *mut Decoding;
    type C = *const Decoding;
    vec![
        (
            "souther_alloc",
            shape_of(souther_alloc as extern "C" fn(Count) -> *mut u8),
        ),
        (
            "souther_scope_open",
            shape_of(souther_scope_open as extern "C" fn() -> Scope),
        ),
        (
            "souther_scope_close",
            shape_of(souther_scope_close as extern "C" fn(Scope) -> i8),
        ),
        (
            "souther_string_compare",
            shape_of(souther_string_compare as unsafe extern "C" fn(T, T) -> Comparison),
        ),
        (
            "souther_string_concat",
            shape_of(souther_string_concat as unsafe extern "C" fn(T, T, *mut M) -> i8),
        ),
        (
            "souther_string_code_points",
            shape_of(souther_string_code_points as unsafe extern "C" fn(T) -> i64),
        ),
        (
            "souther_string_trim",
            shape_of(souther_string_trim as unsafe extern "C" fn(T) -> M),
        ),
        (
            "souther_string_lowercase",
            shape_of(souther_string_lowercase as unsafe extern "C" fn(T, *mut M) -> i8),
        ),
        (
            "souther_string_uppercase",
            shape_of(souther_string_uppercase as unsafe extern "C" fn(T, *mut M) -> i8),
        ),
        (
            "souther_string_contains",
            shape_of(souther_string_contains as unsafe extern "C" fn(T, T) -> i8),
        ),
        (
            "souther_string_starts_with",
            shape_of(souther_string_starts_with as unsafe extern "C" fn(T, T) -> i8),
        ),
        (
            "souther_string_ends_with",
            shape_of(souther_string_ends_with as unsafe extern "C" fn(T, T) -> i8),
        ),
        (
            "souther_string_matches",
            shape_of(souther_string_matches as unsafe extern "C" fn(*const u32, T) -> i8),
        ),
        (
            "souther_string_slice",
            shape_of(
                souther_string_slice as unsafe extern "C" fn(i64, i64, T, *mut *mut Text) -> i8,
            ),
        ),
        (
            "souther_string_split",
            shape_of(souther_string_split as unsafe extern "C" fn(T, T) -> *mut List),
        ),
        (
            "souther_string_join",
            shape_of(souther_string_join as unsafe extern "C" fn(T, *const List, *mut M) -> i8),
        ),
        (
            "souther_string_concat_all",
            shape_of(souther_string_concat_all as unsafe extern "C" fn(*const List, *mut M) -> i8),
        ),
        (
            "souther_string_replace",
            shape_of(souther_string_replace as unsafe extern "C" fn(T, T, T, *mut M) -> i8),
        ),
        (
            "souther_string_words",
            shape_of(souther_string_words as unsafe extern "C" fn(T) -> *mut List),
        ),
        (
            "souther_string_lines",
            shape_of(souther_string_lines as unsafe extern "C" fn(T) -> *mut List),
        ),
        (
            "souther_string_from_int",
            shape_of(souther_string_from_int as extern "C" fn(i64) -> M),
        ),
        (
            "souther_string_to_int",
            shape_of(souther_string_to_int as unsafe extern "C" fn(T, *mut i64) -> i8),
        ),
        (
            "souther_string_reverse",
            shape_of(souther_string_reverse as unsafe extern "C" fn(T, *mut M) -> i8),
        ),
        (
            "souther_string_repeat",
            shape_of(souther_string_repeat as unsafe extern "C" fn(i64, T, *mut *mut Text) -> i8),
        ),
        (
            "souther_string_pad_left",
            shape_of(
                souther_string_pad_left as unsafe extern "C" fn(i64, T, T, *mut *mut Text) -> i8,
            ),
        ),
        (
            "souther_string_pad_right",
            shape_of(
                souther_string_pad_right as unsafe extern "C" fn(i64, T, T, *mut *mut Text) -> i8,
            ),
        ),
        (
            "souther_string_characters",
            shape_of(souther_string_characters as unsafe extern "C" fn(T) -> *mut List),
        ),
        (
            "souther_string_code_point_values",
            shape_of(souther_string_code_point_values as unsafe extern "C" fn(T) -> *mut List),
        ),
        (
            "souther_string_of_utf8",
            shape_of(
                souther_string_of_utf8 as unsafe extern "C" fn(*const u8, Count, *mut M) -> i8,
            ),
        ),
        (
            "souther_string_length",
            shape_of(souther_string_length as unsafe extern "C" fn(T) -> Count),
        ),
        (
            "souther_string_bytes",
            shape_of(souther_string_bytes as unsafe extern "C" fn(T) -> *const u8),
        ),
        (
            "souther_external_null",
            shape_of(souther_external_null as extern "C" fn() -> *mut Form),
        ),
        (
            "souther_external_bool",
            shape_of(souther_external_bool as extern "C" fn(i8) -> *mut Form),
        ),
        (
            "souther_external_int",
            shape_of(souther_external_int as extern "C" fn(i64) -> *mut Form),
        ),
        (
            "souther_external_string",
            shape_of(souther_external_string as unsafe extern "C" fn(T) -> *mut Form),
        ),
        (
            "souther_external_array",
            shape_of(souther_external_array as extern "C" fn() -> *mut Form),
        ),
        (
            "souther_external_append",
            shape_of(souther_external_append as unsafe extern "C" fn(*mut Form, *mut Form)),
        ),
        (
            "souther_external_object",
            shape_of(souther_external_object as extern "C" fn() -> *mut Form),
        ),
        (
            "souther_external_put",
            shape_of(souther_external_put as unsafe extern "C" fn(*mut Form, T, *mut Form)),
        ),
        (
            "souther_external_json",
            shape_of(souther_external_json as unsafe extern "C" fn(*mut Form) -> M),
        ),
        (
            "souther_decode_begin",
            shape_of(souther_decode_begin as unsafe extern "C" fn(*const u8, Count) -> D),
        ),
        (
            "souther_decode_host_begin",
            shape_of(souther_decode_host_begin as unsafe extern "C" fn(*const u8, Count) -> D),
        ),
        (
            "souther_decode_root",
            shape_of(souther_decode_root as unsafe extern "C" fn(C) -> *const Node),
        ),
        (
            "souther_decode_end",
            shape_of(souther_decode_end as unsafe extern "C" fn(D, *const Value)),
        ),
        (
            "souther_decode_abandon",
            shape_of(souther_decode_abandon as unsafe extern "C" fn(D)),
        ),
        (
            "souther_path_below",
            shape_of(souther_path_below as unsafe extern "C" fn(*const Path, T) -> *const Path),
        ),
        (
            "souther_path_at",
            shape_of(souther_path_at as unsafe extern "C" fn(*const Path, Count) -> *const Path),
        ),
        (
            "souther_read_array",
            shape_of(souther_read_array as unsafe extern "C" fn(*const Node, *const Path, D) -> i8),
        ),
        (
            "souther_read_array_length",
            shape_of(souther_read_array_length as unsafe extern "C" fn(*const Node) -> Count),
        ),
        (
            "souther_read_element",
            shape_of(
                souther_read_element as unsafe extern "C" fn(*const Node, Count) -> *const Node,
            ),
        ),
        (
            "souther_read_object",
            shape_of(
                souther_read_object as unsafe extern "C" fn(*const Node, *const Path, D) -> i8,
            ),
        ),
        (
            "souther_read_member",
            shape_of(souther_read_member as unsafe extern "C" fn(*const Node, T) -> *const Node),
        ),
        (
            "souther_read_missing",
            shape_of(souther_read_missing as unsafe extern "C" fn(*const Path, D)),
        ),
        (
            "souther_read_null",
            shape_of(souther_read_null as unsafe extern "C" fn(*const Node) -> i8),
        ),
        (
            "souther_read_int",
            shape_of(
                souther_read_int
                    as unsafe extern "C" fn(*const Node, *const Path, D, *mut i64) -> i8,
            ),
        ),
        (
            "souther_read_bool",
            shape_of(
                souther_read_bool
                    as unsafe extern "C" fn(*const Node, *const Path, D, *mut i8) -> i8,
            ),
        ),
        (
            "souther_read_string",
            shape_of(
                souther_read_string
                    as unsafe extern "C" fn(*const Node, *const Path, D, *mut M) -> i8,
            ),
        ),
        (
            "souther_read_case",
            shape_of(souther_read_case as unsafe extern "C" fn(*const Node, *const Path, D) -> i8),
        ),
        (
            "souther_read_tag",
            shape_of(
                souther_read_tag
                    as unsafe extern "C" fn(*const Node, T, *const Path, D) -> *const Node,
            ),
        ),
        (
            "souther_read_is",
            shape_of(souther_read_is as unsafe extern "C" fn(*const Node, T) -> i8),
        ),
        (
            "souther_read_not_a_case",
            shape_of(souther_read_not_a_case as unsafe extern "C" fn(*const Node, *const Path, D)),
        ),
        (
            "souther_read_invariant",
            shape_of(souther_read_invariant as unsafe extern "C" fn(*const Path, D, T, T, T)),
        ),
        (
            "souther_read_min_length",
            shape_of(souther_read_min_length as unsafe extern "C" fn(*const Path, D, T, i64) -> i8),
        ),
        (
            "souther_read_max_length",
            shape_of(souther_read_max_length as unsafe extern "C" fn(*const Path, D, T, i64) -> i8),
        ),
        (
            "souther_read_fixed_length",
            shape_of(
                souther_read_fixed_length as unsafe extern "C" fn(*const Path, D, T, i64) -> i8,
            ),
        ),
        (
            "souther_read_pattern",
            shape_of(
                souther_read_pattern
                    as unsafe extern "C" fn(*const Path, D, T, *const u32, T) -> i8,
            ),
        ),
        (
            "souther_read_int_min",
            shape_of(souther_read_int_min as unsafe extern "C" fn(*const Path, D, i64, i64) -> i8),
        ),
        (
            "souther_read_int_max",
            shape_of(souther_read_int_max as unsafe extern "C" fn(*const Path, D, i64, i64) -> i8),
        ),
        (
            "souther_read_int_positive",
            shape_of(souther_read_int_positive as unsafe extern "C" fn(*const Path, D, i64) -> i8),
        ),
        (
            "souther_read_int_non_negative",
            shape_of(
                souther_read_int_non_negative as unsafe extern "C" fn(*const Path, D, i64) -> i8,
            ),
        ),
        (
            "souther_read_decimal_min",
            shape_of(
                souther_read_decimal_min
                    as unsafe extern "C" fn(*const Path, D, *const Decimal, *const Decimal) -> i8,
            ),
        ),
        (
            "souther_read_decimal_max",
            shape_of(
                souther_read_decimal_max
                    as unsafe extern "C" fn(*const Path, D, *const Decimal, *const Decimal) -> i8,
            ),
        ),
        (
            "souther_read_decimal_positive",
            shape_of(
                souther_read_decimal_positive
                    as unsafe extern "C" fn(*const Path, D, *const Decimal) -> i8,
            ),
        ),
        (
            "souther_read_decimal_non_negative",
            shape_of(
                souther_read_decimal_non_negative
                    as unsafe extern "C" fn(*const Path, D, *const Decimal) -> i8,
            ),
        ),
        (
            "souther_read_list_non_empty",
            shape_of(
                souther_read_list_non_empty
                    as unsafe extern "C" fn(*const Path, D, *const List) -> i8,
            ),
        ),
        (
            "souther_read_list_min_size",
            shape_of(
                souther_read_list_min_size
                    as unsafe extern "C" fn(*const Path, D, *const List, i64) -> i8,
            ),
        ),
        (
            "souther_read_list_max_size",
            shape_of(
                souther_read_list_max_size
                    as unsafe extern "C" fn(*const Path, D, *const List, i64) -> i8,
            ),
        ),
        (
            "souther_read_list_fixed_size",
            shape_of(
                souther_read_list_fixed_size
                    as unsafe extern "C" fn(*const Path, D, *const List, i64) -> i8,
            ),
        ),
        (
            "souther_read_map_non_empty",
            shape_of(
                souther_read_map_non_empty
                    as unsafe extern "C" fn(*const Path, D, *const Map) -> i8,
            ),
        ),
        (
            "souther_read_map_min_size",
            shape_of(
                souther_read_map_min_size
                    as unsafe extern "C" fn(*const Path, D, *const Map, i64) -> i8,
            ),
        ),
        (
            "souther_read_map_max_size",
            shape_of(
                souther_read_map_max_size
                    as unsafe extern "C" fn(*const Path, D, *const Map, i64) -> i8,
            ),
        ),
        (
            "souther_read_duplicates",
            shape_of(souther_read_duplicates as unsafe extern "C" fn(*const Path, D, *mut Form)),
        ),
        (
            "souther_list_duplicates",
            shape_of(
                souther_list_duplicates
                    as unsafe extern "C" fn(*const List, Hasher, Equality) -> *const List,
            ),
        ),
        (
            "souther_decoded_outcome",
            shape_of(souther_decoded_outcome as unsafe extern "C" fn(C) -> i32),
        ),
        (
            "souther_decoded_value",
            shape_of(souther_decoded_value as unsafe extern "C" fn(C) -> *const Value),
        ),
        (
            "souther_decoded_malformed_at",
            shape_of(souther_decoded_malformed_at as unsafe extern "C" fn(C) -> Count),
        ),
        (
            "souther_decoded_issue_count",
            shape_of(souther_decoded_issue_count as unsafe extern "C" fn(C) -> Count),
        ),
        (
            "souther_decoded_issue",
            shape_of(souther_decoded_issue as unsafe extern "C" fn(C, Count) -> *const Issue),
        ),
        (
            "souther_case_int_make",
            shape_of(souther_case_int_make as extern "C" fn(i64) -> *const Value),
        ),
        (
            "souther_case_int_read",
            shape_of(souther_case_int_read as unsafe extern "C" fn(*const Value, *mut i64) -> i8),
        ),
        (
            "souther_case_bool_make",
            shape_of(souther_case_bool_make as extern "C" fn(i8) -> *const Value),
        ),
        (
            "souther_case_bool_read",
            shape_of(souther_case_bool_read as unsafe extern "C" fn(*const Value, *mut i8) -> i8),
        ),
        (
            "souther_case_string_make",
            shape_of(souther_case_string_make as extern "C" fn(T) -> *const Value),
        ),
        (
            "souther_case_string_read",
            shape_of(
                souther_case_string_read
                    as unsafe extern "C" fn(*const Value, *mut *mut Text) -> i8,
            ),
        ),
        (
            "souther_case_decimal_make",
            shape_of(souther_case_decimal_make as extern "C" fn(*const Decimal) -> *const Value),
        ),
        (
            "souther_case_decimal_read",
            shape_of(
                souther_case_decimal_read
                    as unsafe extern "C" fn(*const Value, *mut *mut Decimal) -> i8,
            ),
        ),
        (
            "souther_decimal_of_parts",
            shape_of(
                souther_decimal_of_parts
                    as unsafe extern "C" fn(*const u8, Count, i64, *mut *mut Decimal) -> i8,
            ),
        ),
        (
            "souther_decimal_literal",
            shape_of(souther_decimal_literal as unsafe extern "C" fn(T, i64) -> *mut Decimal),
        ),
        (
            "souther_decimal_unscaled",
            shape_of(souther_decimal_unscaled as unsafe extern "C" fn(*const Decimal) -> M),
        ),
        (
            "souther_decimal_scale",
            shape_of(souther_decimal_scale as unsafe extern "C" fn(*const Decimal) -> i64),
        ),
        (
            "souther_decimal_compare",
            shape_of(
                souther_decimal_compare
                    as unsafe extern "C" fn(*const Decimal, *const Decimal) -> Comparison,
            ),
        ),
        (
            "souther_rational_from_int",
            shape_of(souther_rational_from_int as extern "C" fn(i64) -> *mut Rational),
        ),
        (
            "souther_rational_from_decimal",
            shape_of(
                souther_rational_from_decimal
                    as unsafe extern "C" fn(*const Decimal) -> *mut Rational,
            ),
        ),
        (
            "souther_rational_negate",
            shape_of(
                souther_rational_negate as unsafe extern "C" fn(*const Rational) -> *mut Rational,
            ),
        ),
        (
            "souther_rational_is_zero",
            shape_of(souther_rational_is_zero as unsafe extern "C" fn(*const Rational) -> i8),
        ),
        (
            "souther_rational_is_whole",
            shape_of(souther_rational_is_whole as unsafe extern "C" fn(*const Rational) -> i8),
        ),
        (
            "souther_rational_has_finite_decimal",
            shape_of(
                souther_rational_has_finite_decimal as unsafe extern "C" fn(*const Rational) -> i8,
            ),
        ),
        (
            "souther_rational_compare",
            shape_of(
                souther_rational_compare
                    as unsafe extern "C" fn(*const Rational, *const Rational) -> Comparison,
            ),
        ),
        (
            "souther_rational_add",
            shape_of(
                souther_rational_add
                    as unsafe extern "C" fn(
                        *const Rational,
                        *const Rational,
                        *mut *mut Rational,
                    ) -> i8,
            ),
        ),
        (
            "souther_rational_subtract",
            shape_of(
                souther_rational_subtract
                    as unsafe extern "C" fn(
                        *const Rational,
                        *const Rational,
                        *mut *mut Rational,
                    ) -> i8,
            ),
        ),
        (
            "souther_rational_multiply",
            shape_of(
                souther_rational_multiply
                    as unsafe extern "C" fn(
                        *const Rational,
                        *const Rational,
                        *mut *mut Rational,
                    ) -> i8,
            ),
        ),
        (
            "souther_rational_divide",
            shape_of(
                souther_rational_divide
                    as unsafe extern "C" fn(
                        *const Rational,
                        *const Rational,
                        *mut *mut Rational,
                    ) -> i8,
            ),
        ),
        (
            "souther_rational_to_whole",
            shape_of(
                souther_rational_to_whole as unsafe extern "C" fn(*const Rational, *mut i64) -> i8,
            ),
        ),
        (
            "souther_rational_to_finite_decimal",
            shape_of(
                souther_rational_to_finite_decimal
                    as unsafe extern "C" fn(*const Rational, *mut *mut Decimal) -> i8,
            ),
        ),
        (
            "souther_rational_to_int",
            shape_of(
                souther_rational_to_int
                    as unsafe extern "C" fn(*const Value, *const Rational, *mut i64) -> i8,
            ),
        ),
        (
            "souther_rational_to_decimal",
            shape_of(
                souther_rational_to_decimal
                    as unsafe extern "C" fn(
                        i64,
                        *const Value,
                        *const Rational,
                        *mut *mut Decimal,
                    ) -> i8,
            ),
        ),
        (
            "souther_decimal_is_zero",
            shape_of(souther_decimal_is_zero as unsafe extern "C" fn(*const Decimal) -> i8),
        ),
        (
            "souther_decimal_negate",
            shape_of(
                souther_decimal_negate as unsafe extern "C" fn(*const Decimal) -> *mut Decimal,
            ),
        ),
        (
            "souther_decimal_add",
            shape_of(
                souther_decimal_add
                    as unsafe extern "C" fn(
                        *const Decimal,
                        *const Decimal,
                        *mut *mut Decimal,
                    ) -> i8,
            ),
        ),
        (
            "souther_decimal_subtract",
            shape_of(
                souther_decimal_subtract
                    as unsafe extern "C" fn(
                        *const Decimal,
                        *const Decimal,
                        *mut *mut Decimal,
                    ) -> i8,
            ),
        ),
        (
            "souther_decimal_multiply",
            shape_of(
                souther_decimal_multiply
                    as unsafe extern "C" fn(
                        *const Decimal,
                        *const Decimal,
                        *mut *mut Decimal,
                    ) -> i8,
            ),
        ),
        (
            "souther_decimal_from_int",
            shape_of(souther_decimal_from_int as extern "C" fn(i64) -> *mut Decimal),
        ),
        (
            "souther_decimal_to_int",
            shape_of(
                souther_decimal_to_int
                    as unsafe extern "C" fn(*const Value, *const Decimal, *mut i64) -> i8,
            ),
        ),
        (
            "souther_decimal_round",
            shape_of(
                souther_decimal_round
                    as unsafe extern "C" fn(
                        i64,
                        *const Value,
                        *const Decimal,
                        *mut *mut Decimal,
                    ) -> i8,
            ),
        ),
        (
            "souther_decimal_divide",
            shape_of(
                souther_decimal_divide
                    as unsafe extern "C" fn(
                        *const Decimal,
                        *const Decimal,
                        i64,
                        *const Value,
                        *mut *mut Decimal,
                    ) -> i8,
            ),
        ),
        (
            "souther_string_to_decimal",
            shape_of(souther_string_to_decimal as unsafe extern "C" fn(T, *mut *mut Decimal) -> i8),
        ),
        (
            "souther_string_from_decimal",
            shape_of(
                souther_string_from_decimal as unsafe extern "C" fn(*const Decimal, *mut M) -> i8,
            ),
        ),
        (
            "souther_external_decimal",
            shape_of(souther_external_decimal as unsafe extern "C" fn(*const Decimal) -> *mut Form),
        ),
        (
            "souther_read_decimal",
            shape_of(
                souther_read_decimal
                    as unsafe extern "C" fn(*const Node, *const Path, D, *mut *mut Decimal) -> i8,
            ),
        ),
        (
            "souther_case_date_make",
            shape_of(souther_case_date_make as extern "C" fn(*const Date) -> *const Value),
        ),
        (
            "souther_case_date_read",
            shape_of(
                souther_case_date_read as unsafe extern "C" fn(*const Value, *mut *mut Date) -> i8,
            ),
        ),
        (
            "souther_date_of_parts",
            shape_of(
                souther_date_of_parts as unsafe extern "C" fn(i64, i64, i64, *mut *mut Date) -> i8,
            ),
        ),
        (
            "souther_date_parts",
            shape_of(
                souther_date_parts
                    as unsafe extern "C" fn(*const Date, *mut i64, *mut i64, *mut i64),
            ),
        ),
        (
            "souther_time_of_parts",
            shape_of(
                souther_time_of_parts as unsafe extern "C" fn(i64, i64, i64, *mut *mut Time) -> i8,
            ),
        ),
        (
            "souther_time_parts",
            shape_of(
                souther_time_parts
                    as unsafe extern "C" fn(*const Time, *mut i64, *mut i64, *mut i64),
            ),
        ),
        (
            "souther_datetime_of_parts",
            shape_of(
                souther_datetime_of_parts
                    as unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, *mut *mut DateTime) -> i8,
            ),
        ),
        (
            "souther_datetime_parts",
            shape_of(
                souther_datetime_parts
                    as unsafe extern "C" fn(
                        *const DateTime,
                        *mut i64,
                        *mut i64,
                        *mut i64,
                        *mut i64,
                        *mut i64,
                        *mut i64,
                    ),
            ),
        ),
        (
            "souther_instant_of_parts",
            shape_of(
                souther_instant_of_parts as unsafe extern "C" fn(i64, i64, *mut *mut Instant) -> i8,
            ),
        ),
        (
            "souther_instant_parts",
            shape_of(
                souther_instant_parts as unsafe extern "C" fn(*const Instant, *mut i64, *mut i64),
            ),
        ),
        (
            "souther_date_literal",
            shape_of(souther_date_literal as extern "C" fn(i64) -> *mut Date),
        ),
        (
            "souther_date_compare",
            shape_of(
                souther_date_compare
                    as unsafe extern "C" fn(*const Date, *const Date) -> Comparison,
            ),
        ),
        (
            "souther_external_date",
            shape_of(souther_external_date as unsafe extern "C" fn(*const Date) -> *mut Form),
        ),
        (
            "souther_read_date",
            shape_of(
                souther_read_date
                    as unsafe extern "C" fn(*const Node, *const Path, D, *mut *mut Date) -> i8,
            ),
        ),
        (
            "souther_case_time_make",
            shape_of(souther_case_time_make as extern "C" fn(*const Time) -> *const Value),
        ),
        (
            "souther_case_time_read",
            shape_of(
                souther_case_time_read as unsafe extern "C" fn(*const Value, *mut *mut Time) -> i8,
            ),
        ),
        (
            "souther_time_literal",
            shape_of(souther_time_literal as extern "C" fn(i64) -> *mut Time),
        ),
        (
            "souther_time_compare",
            shape_of(
                souther_time_compare
                    as unsafe extern "C" fn(*const Time, *const Time) -> Comparison,
            ),
        ),
        (
            "souther_external_time",
            shape_of(souther_external_time as unsafe extern "C" fn(*const Time) -> *mut Form),
        ),
        (
            "souther_read_time",
            shape_of(
                souther_read_time
                    as unsafe extern "C" fn(*const Node, *const Path, D, *mut *mut Time) -> i8,
            ),
        ),
        (
            "souther_case_datetime_make",
            shape_of(souther_case_datetime_make as extern "C" fn(*const DateTime) -> *const Value),
        ),
        (
            "souther_case_datetime_read",
            shape_of(
                souther_case_datetime_read
                    as unsafe extern "C" fn(*const Value, *mut *mut DateTime) -> i8,
            ),
        ),
        (
            "souther_datetime_literal",
            shape_of(souther_datetime_literal as extern "C" fn(i64) -> *mut DateTime),
        ),
        (
            "souther_datetime_compare",
            shape_of(
                souther_datetime_compare
                    as unsafe extern "C" fn(*const DateTime, *const DateTime) -> Comparison,
            ),
        ),
        (
            "souther_external_datetime",
            shape_of(
                souther_external_datetime as unsafe extern "C" fn(*const DateTime) -> *mut Form,
            ),
        ),
        (
            "souther_read_datetime",
            shape_of(
                souther_read_datetime
                    as unsafe extern "C" fn(*const Node, *const Path, D, *mut *mut DateTime) -> i8,
            ),
        ),
        (
            "souther_case_instant_make",
            shape_of(souther_case_instant_make as extern "C" fn(*const Instant) -> *const Value),
        ),
        (
            "souther_case_instant_read",
            shape_of(
                souther_case_instant_read
                    as unsafe extern "C" fn(*const Value, *mut *mut Instant) -> i8,
            ),
        ),
        (
            "souther_instant_literal",
            shape_of(souther_instant_literal as extern "C" fn(i64, i64) -> *mut Instant),
        ),
        (
            "souther_instant_compare",
            shape_of(
                souther_instant_compare
                    as unsafe extern "C" fn(*const Instant, *const Instant) -> Comparison,
            ),
        ),
        (
            "souther_external_instant",
            shape_of(souther_external_instant as unsafe extern "C" fn(*const Instant) -> *mut Form),
        ),
        (
            "souther_read_instant",
            shape_of(
                souther_read_instant
                    as unsafe extern "C" fn(*const Node, *const Path, D, *mut *mut Instant) -> i8,
            ),
        ),
        (
            "souther_date_add_days",
            shape_of(
                souther_date_add_days
                    as unsafe extern "C" fn(i64, *const Date, *mut *mut Date) -> i8,
            ),
        ),
        (
            "souther_date_add_months",
            shape_of(
                souther_date_add_months
                    as unsafe extern "C" fn(i64, *const Date, *mut *mut Date) -> i8,
            ),
        ),
        (
            "souther_date_add_years",
            shape_of(
                souther_date_add_years
                    as unsafe extern "C" fn(i64, *const Date, *mut *mut Date) -> i8,
            ),
        ),
        (
            "souther_date_days_between",
            shape_of(
                souther_date_days_between as unsafe extern "C" fn(*const Date, *const Date) -> i64,
            ),
        ),
        (
            "souther_date_year",
            shape_of(souther_date_year as unsafe extern "C" fn(*const Date) -> i64),
        ),
        (
            "souther_date_month",
            shape_of(souther_date_month as unsafe extern "C" fn(*const Date) -> i64),
        ),
        (
            "souther_date_day",
            shape_of(souther_date_day as unsafe extern "C" fn(*const Date) -> i64),
        ),
        (
            "souther_date_from_parts",
            shape_of(
                souther_date_from_parts
                    as unsafe extern "C" fn(i64, i64, i64, *mut *mut Date) -> i8,
            ),
        ),
        (
            "souther_time_from_parts",
            shape_of(
                souther_time_from_parts
                    as unsafe extern "C" fn(i64, i64, i64, *mut *mut Time) -> i8,
            ),
        ),
        (
            "souther_time_hour",
            shape_of(souther_time_hour as unsafe extern "C" fn(*const Time) -> i64),
        ),
        (
            "souther_time_minute",
            shape_of(souther_time_minute as unsafe extern "C" fn(*const Time) -> i64),
        ),
        (
            "souther_time_second",
            shape_of(souther_time_second as unsafe extern "C" fn(*const Time) -> i64),
        ),
        (
            "souther_datetime_add_minutes",
            shape_of(
                souther_datetime_add_minutes
                    as unsafe extern "C" fn(i64, *const DateTime, *mut *mut DateTime) -> i8,
            ),
        ),
        (
            "souther_datetime_add_hours",
            shape_of(
                souther_datetime_add_hours
                    as unsafe extern "C" fn(i64, *const DateTime, *mut *mut DateTime) -> i8,
            ),
        ),
        (
            "souther_datetime_add_days",
            shape_of(
                souther_datetime_add_days
                    as unsafe extern "C" fn(i64, *const DateTime, *mut *mut DateTime) -> i8,
            ),
        ),
        (
            "souther_datetime_minutes_between",
            shape_of(
                souther_datetime_minutes_between
                    as unsafe extern "C" fn(*const DateTime, *const DateTime) -> i64,
            ),
        ),
        (
            "souther_datetime_to_date",
            shape_of(
                souther_datetime_to_date as unsafe extern "C" fn(*const DateTime) -> *mut Date,
            ),
        ),
        (
            "souther_datetime_to_time",
            shape_of(
                souther_datetime_to_time as unsafe extern "C" fn(*const DateTime) -> *mut Time,
            ),
        ),
        (
            "souther_datetime_from_date_and_time",
            shape_of(
                souther_datetime_from_date_and_time
                    as unsafe extern "C" fn(*const Date, *const Time) -> *mut DateTime,
            ),
        ),
        (
            "souther_case_some_make",
            shape_of(souther_case_some_make as extern "C" fn() -> *const Value),
        ),
        (
            "souther_case_none_make",
            shape_of(souther_case_none_make as extern "C" fn() -> *const Value),
        ),
        (
            "souther_case_division_by_zero_make",
            shape_of(souther_case_division_by_zero_make as extern "C" fn() -> *const Value),
        ),
        (
            "souther_case_not_a_number_make",
            shape_of(souther_case_not_a_number_make as extern "C" fn() -> *const Value),
        ),
        (
            "souther_case_not_a_date_make",
            shape_of(souther_case_not_a_date_make as extern "C" fn() -> *const Value),
        ),
        (
            "souther_case_not_a_time_make",
            shape_of(souther_case_not_a_time_make as extern "C" fn() -> *const Value),
        ),
        (
            "souther_case_not_whole_make",
            shape_of(souther_case_not_whole_make as extern "C" fn() -> *const Value),
        ),
        (
            "souther_case_not_a_finite_decimal_make",
            shape_of(souther_case_not_a_finite_decimal_make as extern "C" fn() -> *const Value),
        ),
        (
            "souther_issue_code",
            shape_of(souther_issue_code as unsafe extern "C" fn(*const Issue) -> T),
        ),
        (
            "souther_issue_path",
            shape_of(souther_issue_path as unsafe extern "C" fn(*const Issue) -> T),
        ),
        (
            "souther_issue_message_key",
            shape_of(souther_issue_message_key as unsafe extern "C" fn(*const Issue) -> T),
        ),
        (
            "souther_issue_meta",
            shape_of(souther_issue_meta as unsafe extern "C" fn(*const Issue) -> T),
        ),
        (
            "souther_external_order",
            shape_of(souther_external_order as unsafe extern "C" fn(*mut Form)),
        ),
        (
            "souther_external_entries",
            shape_of(souther_external_entries as unsafe extern "C" fn(*mut Form)),
        ),
        (
            "souther_read_members",
            shape_of(souther_read_members as unsafe extern "C" fn(*const Node) -> Count),
        ),
        (
            "souther_read_member_key",
            shape_of(
                souther_read_member_key as unsafe extern "C" fn(*const Node, Count) -> *const Node,
            ),
        ),
        (
            "souther_read_member_value",
            shape_of(
                souther_read_member_value
                    as unsafe extern "C" fn(*const Node, Count) -> *const Node,
            ),
        ),
        (
            "souther_path_below_member",
            shape_of(
                souther_path_below_member
                    as unsafe extern "C" fn(*const Path, *const Node, Count) -> *const Path,
            ),
        ),
        (
            "souther_read_duplicate_key",
            shape_of(souther_read_duplicate_key as unsafe extern "C" fn(*const Path, D)),
        ),
        (
            "souther_set_empty",
            shape_of(souther_set_empty as extern "C" fn() -> *mut Set),
        ),
        (
            "souther_set_insert",
            shape_of(
                souther_set_insert
                    as unsafe extern "C" fn(
                        *const Set,
                        i64,
                        Hasher,
                        Equality,
                        *mut *const Set,
                    ) -> i8,
            ),
        ),
        (
            "souther_set_remove",
            shape_of(
                souther_set_remove
                    as unsafe extern "C" fn(*const Set, i64, Hasher, Equality) -> *const Set,
            ),
        ),
        (
            "souther_set_contains",
            shape_of(
                souther_set_contains
                    as unsafe extern "C" fn(*const Set, i64, Hasher, Equality) -> i8,
            ),
        ),
        (
            "souther_set_union",
            shape_of(
                souther_set_union
                    as unsafe extern "C" fn(
                        *const Set,
                        *const Set,
                        Equality,
                        *mut *const Set,
                    ) -> i8,
            ),
        ),
        (
            "souther_set_intersection",
            shape_of(
                souther_set_intersection
                    as unsafe extern "C" fn(*const Set, *const Set, Equality) -> *const Set,
            ),
        ),
        (
            "souther_set_difference",
            shape_of(
                souther_set_difference
                    as unsafe extern "C" fn(*const Set, *const Set, Equality) -> *const Set,
            ),
        ),
        (
            "souther_set_size",
            shape_of(souther_set_size as unsafe extern "C" fn(*const Set) -> i64),
        ),
        (
            "souther_set_to_list",
            shape_of(souther_set_to_list as unsafe extern "C" fn(*const Set) -> *mut List),
        ),
        (
            "souther_set_from_list",
            shape_of(
                souther_set_from_list
                    as unsafe extern "C" fn(*const List, Hasher, Equality) -> *const Set,
            ),
        ),
        (
            "souther_set_equal",
            shape_of(
                souther_set_equal as unsafe extern "C" fn(*const Set, *const Set, Equality) -> i8,
            ),
        ),
        (
            "souther_set_hash",
            shape_of(souther_set_hash as unsafe extern "C" fn(*const Set) -> Hash),
        ),
        (
            "souther_map_empty",
            shape_of(souther_map_empty as extern "C" fn() -> *mut Map),
        ),
        (
            "souther_map_get",
            shape_of(
                souther_map_get
                    as unsafe extern "C" fn(*const Map, i64, Hasher, Equality) -> *const HeldAt,
            ),
        ),
        (
            "souther_map_contains_key",
            shape_of(
                souther_map_contains_key
                    as unsafe extern "C" fn(*const Map, i64, Hasher, Equality) -> i8,
            ),
        ),
        (
            "souther_map_keys",
            shape_of(souther_map_keys as unsafe extern "C" fn(*const Map) -> *mut List),
        ),
        (
            "souther_map_values",
            shape_of(souther_map_values as unsafe extern "C" fn(*const Map) -> *mut List),
        ),
        (
            "souther_map_insert",
            shape_of(
                souther_map_insert
                    as unsafe extern "C" fn(
                        *const Map,
                        i64,
                        i64,
                        Hasher,
                        Equality,
                        *mut *const Map,
                    ) -> i8,
            ),
        ),
        (
            "souther_map_remove",
            shape_of(
                souther_map_remove
                    as unsafe extern "C" fn(*const Map, i64, Hasher, Equality) -> *const Map,
            ),
        ),
        (
            "souther_map_size",
            shape_of(souther_map_size as unsafe extern "C" fn(*const Map) -> i64),
        ),
        (
            "souther_map_to_list",
            shape_of(souther_map_to_list as unsafe extern "C" fn(*const Map) -> *mut List),
        ),
        (
            "souther_map_from_list",
            shape_of(
                souther_map_from_list
                    as unsafe extern "C" fn(*const List, Hasher, Equality) -> *mut Map,
            ),
        ),
        (
            "souther_map_equal",
            shape_of(
                souther_map_equal
                    as unsafe extern "C" fn(*const Map, *const Map, Equality, Equality) -> i8,
            ),
        ),
        (
            "souther_map_hash",
            shape_of(souther_map_hash as unsafe extern "C" fn(*const Map, Hasher) -> Hash),
        ),
        (
            "souther_hash_combine",
            shape_of(souther_hash_combine as extern "C" fn(Hash, i64) -> Hash),
        ),
        (
            "souther_string_hash",
            shape_of(souther_string_hash as unsafe extern "C" fn(T) -> Hash),
        ),
        (
            "souther_decimal_hash",
            shape_of(souther_decimal_hash as unsafe extern "C" fn(*const Decimal) -> Hash),
        ),
        (
            "souther_rational_hash",
            shape_of(souther_rational_hash as unsafe extern "C" fn(*const Rational) -> Hash),
        ),
        (
            "souther_date_hash",
            shape_of(souther_date_hash as unsafe extern "C" fn(*const Date) -> Hash),
        ),
        (
            "souther_time_hash",
            shape_of(souther_time_hash as unsafe extern "C" fn(*const Time) -> Hash),
        ),
        (
            "souther_datetime_hash",
            shape_of(souther_datetime_hash as unsafe extern "C" fn(*const DateTime) -> Hash),
        ),
        (
            "souther_instant_hash",
            shape_of(souther_instant_hash as unsafe extern "C" fn(*const Instant) -> Hash),
        ),
    ]
}

/// Every function a host calls: the runtime's own, and what it makes and reads each case no
/// declaration names through.
fn host_functions() -> Vec<&'static RuntimeFunction> {
    HOST_RUNTIME
        .iter()
        .chain(
            HOST_CASES
                .iter()
                .flat_map(|it| std::iter::once(&it.make).chain(&it.read)),
        )
        .collect()
}

/// What the tables say of every function, by its name.
fn said() -> Vec<(&'static str, Shape)> {
    let host = host_functions().into_iter().map(|function| {
        (
            function.name,
            (
                function
                    .takes
                    .iter()
                    .copied()
                    .map(Parameter::from)
                    .collect(),
                function.answers.map(Word::Host),
            ),
        )
    });
    let generated = GENERATED_RUNTIME
        .iter()
        .map(|call| (call.name, (call.takes.to_vec(), call.answers)));
    host.chain(generated).collect()
}

#[test]
fn every_function_is_what_the_table_naming_it_says() {
    let functions = functions();
    for (name, said) in said() {
        let Some((_, shape)) = functions.iter().find(|(it, _)| *it == name) else {
            panic!("{name} is in a table and not written here as the function it is");
        };
        assert_eq!(&said, shape, "{name}");
    }
}

/// What the runtime defines for its own tests and the driver's to count with, and for no other
/// party: in neither table, so no header declares it and no library exports it.
const INSTRUMENTS: &[&str] = &["souther_arena_taken"];

/// The generation query is in neither table: it is outside every generation, the same in each,
/// and held to what it was made as by the ABI's own test. Here it is held to being that function.
#[test]
fn the_generation_query_is_the_function_it_is_declared_as() {
    let query: extern "C" fn() -> u32 = souther_abi_generation;
    assert_eq!(query(), souther_native_abi::ABI_GENERATION);
    assert_eq!(
        souther_native_abi::GENERATION_QUERY,
        "souther_abi_generation"
    );
}

/// Every function the runtime defines for another party is in exactly one of the two tables, and
/// is written above: read off the source, so a function added here and to neither table is caught
/// rather than called by someone the tables say nothing to.
#[test]
fn every_function_the_runtime_defines_is_in_one_table() {
    let sources = [
        ("lib", include_str!("lib.rs")),
        ("amount", include_str!("amount.rs")),
        ("collection", include_str!("collection.rs")),
        ("contract", include_str!("contract.rs")),
        ("decimal", include_str!("decimal.rs")),
        ("decoding", include_str!("decoding.rs")),
        ("document", include_str!("document.rs")),
        ("enclosure", include_str!("enclosure.rs")),
        ("external", include_str!("external.rs")),
        ("kernels", include_str!("kernels.rs")),
        ("magnitude", include_str!("magnitude.rs")),
        ("rational", include_str!("rational.rs")),
        ("temporal", include_str!("temporal.rs")),
    ];
    // Every module the crate declares is read, so a function defined in a file added later is not
    // one this test never saw.
    let declared: BTreeSet<&str> = include_str!("lib.rs")
        .lines()
        .filter_map(|line| line.strip_prefix("mod ")?.strip_suffix(';'))
        .chain(["lib"])
        .collect();
    let read: BTreeSet<&str> = sources.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        read, declared,
        "every module of the crate is read for what it defines"
    );
    let marker = "extern \"C\" fn ";
    let mut defined = BTreeSet::new();
    for (_, source) in sources {
        for (at, _) in source.match_indices(marker) {
            let rest = &source[at + marker.len()..];
            let name: String = rest
                .chars()
                .take_while(|it| it.is_ascii_alphanumeric() || *it == '_')
                .collect();
            if name.starts_with("souther_")
                && !INSTRUMENTS.contains(&name.as_str())
                && name != souther_native_abi::GENERATION_QUERY
            {
                defined.insert(name);
            }
        }
    }
    let host: Vec<&str> = host_functions().iter().map(|it| it.name).collect();
    let generated: Vec<&str> = GENERATED_RUNTIME.iter().map(|it| it.name).collect();
    for name in &host {
        assert!(!generated.contains(name), "{name} is in both tables");
    }
    let tabled: BTreeSet<String> = host
        .iter()
        .chain(&generated)
        .map(|it| it.to_string())
        .collect();
    assert_eq!(tabled, defined);
    let written: BTreeSet<String> = functions()
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect();
    assert_eq!(written, defined);
}

/// A case a host makes a value of is a case the runtime defines a token for. The other way about
/// is not held: a case can have a token and no external form, and `Rational` is one.
#[test]
fn a_host_makes_only_cases_the_runtime_has_a_token_for() {
    let built_in: BTreeSet<&str> = BUILT_IN_CASES.iter().copied().collect();
    for crossing in HOST_CASES {
        assert!(
            built_in.contains(crossing.case),
            "{} is made by a host and has no token",
            crossing.case
        );
    }
}

/// A rational is something objects share and a host never sees: it has a token, and no function a
/// host calls makes or reads one. That none takes or answers one is the type's to say: there is
/// no `HostWord` for it.
#[test]
fn a_rational_is_a_case_and_no_host_crosses_it() {
    assert!(BUILT_IN_CASES.contains(&"Rational"));
    assert!(HOST_CASES.iter().all(|it| it.case != "Rational"));
}

/// What a host makes of a case and what it reads back are what went in, and the value says it is
/// the case: the token at its front is the one the runtime defines for it.
#[test]
fn a_case_a_host_makes_reads_back_as_what_it_holds() {
    let scope = souther_scope_open();
    let which = |value: *const Value| unsafe { value.cast::<*const u8>().read() };
    let int = souther_case_int_make(-42);
    assert_eq!(which(int), CASE_INT.as_ptr());
    let mut number = 0;
    assert_eq!(unsafe { souther_case_int_read(int, &mut number) }, 1);
    assert_eq!(number, -42);
    for truth in [0, 1] {
        let bool = souther_case_bool_make(truth);
        assert_eq!(which(bool), CASE_BOOL.as_ptr());
        let mut read = 9;
        assert_eq!(unsafe { souther_case_bool_read(bool, &mut read) }, 1);
        assert_eq!(read, truth);
    }
    // SAFETY: three bytes of UTF-8 at the address handed over.
    let mut text = std::ptr::null_mut();
    let admitted = unsafe { souther_string_of_utf8("hé".as_ptr(), Count(3), &mut text) };
    assert_eq!(admitted, 1, "test text has a place");
    let string = souther_case_string_make(text);
    assert_eq!(which(string), CASE_STRING.as_ptr());
    let mut read = std::ptr::null_mut();
    assert_eq!(unsafe { souther_case_string_read(string, &mut read) }, 1);
    assert_eq!(read, text);
    assert_eq!(which(souther_case_none_make()), CASE_NONE.as_ptr());
    souther_scope_close(scope);
}

/// What each function a host calls is held to of what a host hands it
/// ([`souther_native_abi::HOST_INPUT_CONTRACT`]).
enum Hostile {
    /// It takes nothing but handles, and what a handle is, is the host's to hold to.
    HandlesOnly,
    /// It is called here with what a host can have wrong, and answers each as a refusal.
    Refuses(fn()),
}

/// Every function a host calls, and what it answers a host that hands it what it does not take.
/// A function a host calls that is missing here, or said to take only handles while it takes a
/// datum, fails [`every_function_a_host_calls_answers_what_it_is_handed`].
fn hostile() -> Vec<(&'static str, Hostile)> {
    use Hostile::{HandlesOnly, Refuses};
    vec![
        (
            "souther_scope_open",
            Refuses(|| {
                // Takes nothing, and every token it answers is closed by the one close for it.
                let scope = souther_scope_open();
                assert_eq!(souther_scope_close(scope), 1);
            }),
        ),
        (
            "souther_scope_close",
            Refuses(|| {
                for token in [i64::MIN, -1, 0, i64::MAX] {
                    assert_eq!(souther_scope_close(Scope(token)), 0, "{token}");
                }
            }),
        ),
        (
            "souther_string_of_utf8",
            Refuses(|| {
                let scope = souther_scope_open();
                let mut out = std::ptr::null_mut();
                let not_text = [0xff_u8, 0xfe, b'a'];
                for (bytes, count) in [
                    (&not_text[..], 3),
                    (&b"abc"[..], -1),
                    (&b"abc"[..], i64::MIN),
                ] {
                    let made =
                        unsafe { souther_string_of_utf8(bytes.as_ptr(), Count(count), &mut out) };
                    assert_eq!(made, 0, "{bytes:?} counted {count}");
                }
                souther_scope_close(scope);
            }),
        ),
        ("souther_string_length", HandlesOnly),
        ("souther_string_bytes", HandlesOnly),
        (
            "souther_decimal_of_parts",
            Refuses(|| {
                let scope = souther_scope_open();
                let mut out = std::ptr::null_mut();
                for (digits, count, scale) in [
                    (&b"1.5"[..], 3, 0),
                    (&[0xff_u8][..], 1, 0),
                    (&b"15"[..], -1, 0),
                    (&b"15"[..], 2, i64::MAX),
                    (&b"15"[..], 2, i64::MIN),
                ] {
                    let made = unsafe {
                        souther_decimal_of_parts(digits.as_ptr(), Count(count), scale, &mut out)
                    };
                    assert_eq!(made, 0, "{digits:?} counted {count} at {scale}");
                }
                souther_scope_close(scope);
            }),
        ),
        ("souther_decimal_unscaled", HandlesOnly),
        ("souther_decimal_scale", HandlesOnly),
        (
            "souther_date_of_parts",
            Refuses(|| {
                let mut out = std::ptr::null_mut();
                for [y, m, d] in [
                    [i64::MIN, 1, 1],
                    [2026, -1, 1],
                    [2026, 2, 30],
                    [i64::MAX; 3],
                ] {
                    assert_eq!(unsafe { souther_date_of_parts(y, m, d, &mut out) }, 0);
                }
            }),
        ),
        ("souther_date_parts", HandlesOnly),
        (
            "souther_time_of_parts",
            Refuses(|| {
                let mut out = std::ptr::null_mut();
                for [h, m, s] in [[-1, 0, 0], [24, 0, 0], [0, 60, 0], [i64::MAX; 3]] {
                    assert_eq!(unsafe { souther_time_of_parts(h, m, s, &mut out) }, 0);
                }
            }),
        ),
        ("souther_time_parts", HandlesOnly),
        (
            "souther_datetime_of_parts",
            Refuses(|| {
                let mut out = std::ptr::null_mut();
                for [y, mo, d, h, mi, s] in [[i64::MIN, 1, 1, 0, 0, 0], [2026, 1, 1, 0, 0, -1]] {
                    assert_eq!(
                        unsafe { souther_datetime_of_parts(y, mo, d, h, mi, s, &mut out) },
                        0
                    );
                }
            }),
        ),
        ("souther_datetime_parts", HandlesOnly),
        (
            "souther_instant_of_parts",
            Refuses(|| {
                let mut out = std::ptr::null_mut();
                for [s, n] in [[i64::MIN, 0], [i64::MAX, 0], [0, -1], [0, i64::MAX]] {
                    assert_eq!(unsafe { souther_instant_of_parts(s, n, &mut out) }, 0);
                }
            }),
        ),
        ("souther_instant_parts", HandlesOnly),
        ("souther_decoded_outcome", HandlesOnly),
        ("souther_decoded_value", HandlesOnly),
        ("souther_decoded_malformed_at", HandlesOnly),
        ("souther_decoded_issue_count", HandlesOnly),
        (
            "souther_decoded_issue",
            Refuses(|| {
                let scope = souther_scope_open();
                let document = b"[]";
                let decoded = unsafe { souther_decode_begin(document.as_ptr(), Count(2)) };
                for at in [i64::MIN, -1, 0, i64::MAX] {
                    assert!(unsafe { souther_decoded_issue(decoded, Count(at)) }.is_null());
                }
                // And a count below nought is a document that is none, and ends nothing.
                let none = unsafe { souther_decode_begin(document.as_ptr(), Count(-1)) };
                assert_eq!(unsafe { souther_decoded_malformed_at(none) }, Count(0));
                souther_scope_close(scope);
            }),
        ),
        ("souther_issue_code", HandlesOnly),
        ("souther_issue_message_key", HandlesOnly),
        ("souther_issue_path", HandlesOnly),
        ("souther_issue_meta", HandlesOnly),
        (
            "souther_case_int_make",
            Refuses(|| {
                // Every Int is one.
                for number in [i64::MIN, -1, i64::MAX] {
                    let scope = souther_scope_open();
                    let mut read = 0;
                    let made = souther_case_int_make(number);
                    assert_eq!(unsafe { souther_case_int_read(made, &mut read) }, 1);
                    assert_eq!(read, number);
                    souther_scope_close(scope);
                }
            }),
        ),
        (
            "souther_case_bool_make",
            Refuses(|| {
                let scope = souther_scope_open();
                for (byte, is) in [(0, 0), (1, 1), (2, 1), (-1, 1)] {
                    let mut read = 9;
                    let made = souther_case_bool_make(byte);
                    assert_eq!(unsafe { souther_case_bool_read(made, &mut read) }, 1);
                    assert_eq!(read, is, "{byte}");
                }
                souther_scope_close(scope);
            }),
        ),
        ("souther_case_string_make", HandlesOnly),
        ("souther_case_decimal_make", HandlesOnly),
        ("souther_case_date_make", HandlesOnly),
        ("souther_case_time_make", HandlesOnly),
        ("souther_case_datetime_make", HandlesOnly),
        ("souther_case_instant_make", HandlesOnly),
        ("souther_case_some_make", HandlesOnly),
        ("souther_case_none_make", HandlesOnly),
        ("souther_case_division_by_zero_make", HandlesOnly),
        ("souther_case_not_a_number_make", HandlesOnly),
        ("souther_case_not_a_date_make", HandlesOnly),
        ("souther_case_not_a_time_make", HandlesOnly),
        ("souther_case_not_whole_make", HandlesOnly),
        ("souther_case_not_a_finite_decimal_make", HandlesOnly),
        ("souther_case_int_read", Refuses(read_as_another_case)),
        ("souther_case_bool_read", Refuses(read_as_another_case)),
        ("souther_case_string_read", Refuses(read_as_another_case)),
        ("souther_case_decimal_read", Refuses(read_as_another_case)),
        ("souther_case_date_read", Refuses(read_as_another_case)),
        ("souther_case_time_read", Refuses(read_as_another_case)),
        ("souther_case_datetime_read", Refuses(read_as_another_case)),
        ("souther_case_instant_read", Refuses(read_as_another_case)),
    ]
}

/// Which case a value is, is read off the value: a value of one case read as every other is
/// answered as that, and read as its own is not.
fn read_as_another_case() {
    let scope = souther_scope_open();
    let none = souther_case_none_make();
    let int = souther_case_int_make(7);
    unsafe {
        let mut i = 0;
        let mut b = 0;
        let mut s = std::ptr::null_mut();
        let mut d = std::ptr::null_mut();
        let mut date = std::ptr::null_mut();
        let mut time = std::ptr::null_mut();
        let mut dt = std::ptr::null_mut();
        let mut instant = std::ptr::null_mut();
        for value in [none, int] {
            assert_eq!(souther_case_bool_read(value, &mut b), 0);
            assert_eq!(souther_case_string_read(value, &mut s), 0);
            assert_eq!(souther_case_decimal_read(value, &mut d), 0);
            assert_eq!(souther_case_date_read(value, &mut date), 0);
            assert_eq!(souther_case_time_read(value, &mut time), 0);
            assert_eq!(souther_case_datetime_read(value, &mut dt), 0);
            assert_eq!(souther_case_instant_read(value, &mut instant), 0);
        }
        assert_eq!(souther_case_int_read(none, &mut i), 0);
        assert_eq!(souther_case_int_read(int, &mut i), 1);
        assert_eq!(i, 7);
    }
    souther_scope_close(scope);
}

/// Every function a host calls answers every datum it takes, as
/// [`souther_native_abi::HOST_INPUT_CONTRACT`] says; and
/// each is held here: one said to take only handles takes no datum, and one that takes a datum is
/// called with what a host can have wrong. A function a host calls that this does not name fails
/// it, so the next one is held too. A call that ends the process fails the run that makes it.
#[test]
fn every_function_a_host_calls_answers_what_it_is_handed() {
    let hostile = hostile();
    let named: BTreeSet<&str> = hostile.iter().map(|(name, _)| *name).collect();
    let called: BTreeSet<&str> = host_functions().iter().map(|it| it.name).collect();
    assert_eq!(named, called, "every function a host calls, and no other");
    for (name, held) in hostile {
        let function = host_functions()
            .into_iter()
            .find(|it| it.name == name)
            .expect("named above");
        let takes_a_datum = function.takes.iter().any(
            |it| matches!(it, souther_native_abi::HostParameter::Given(word) if word.is_datum()),
        );
        match held {
            Hostile::HandlesOnly => assert!(!takes_a_datum, "{name} takes a datum"),
            Hostile::Refuses(call) => call(),
        }
    }
}
