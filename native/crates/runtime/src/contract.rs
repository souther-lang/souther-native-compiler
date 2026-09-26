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
    Mark => Word::Host(HostWord::Mark),
    Comparison => Word::Comparison,
    *const u8 => Word::Host(HostWord::Bytes),
    *mut u8 => Word::Memory,
    *const Text => Word::Host(HostWord::String),
    *const Decimal => Word::Host(HostWord::Decimal),
    *mut Decimal => Word::Host(HostWord::Decimal),
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
}

rooms! {
    *mut i64 => Word::Host(HostWord::Int),
    *mut i8 => Word::Host(HostWord::Bool),
    *mut *mut Text => Word::Host(HostWord::String),
    *mut *mut Decimal => Word::Host(HostWord::Decimal),
    *mut *mut Date => Word::Host(HostWord::Date),
    *mut *mut Time => Word::Host(HostWord::Time),
    *mut *mut DateTime => Word::Host(HostWord::DateTime),
    *mut *mut Instant => Word::Host(HostWord::Instant),
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
            "souther_mark",
            shape_of(souther_mark as extern "C" fn() -> Mark),
        ),
        (
            "souther_reset",
            shape_of(souther_reset as extern "C" fn(Mark)),
        ),
        (
            "souther_string_compare",
            shape_of(souther_string_compare as unsafe extern "C" fn(T, T) -> Comparison),
        ),
        (
            "souther_string_concat",
            shape_of(souther_string_concat as unsafe extern "C" fn(T, T) -> M),
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
            shape_of(souther_string_lowercase as unsafe extern "C" fn(T) -> M),
        ),
        (
            "souther_string_uppercase",
            shape_of(souther_string_uppercase as unsafe extern "C" fn(T) -> M),
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
            shape_of(souther_string_join as unsafe extern "C" fn(T, *const List) -> M),
        ),
        (
            "souther_string_concat_all",
            shape_of(souther_string_concat_all as unsafe extern "C" fn(*const List) -> M),
        ),
        (
            "souther_string_replace",
            shape_of(souther_string_replace as unsafe extern "C" fn(T, T, T) -> M),
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
            shape_of(souther_string_reverse as unsafe extern "C" fn(T) -> M),
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
            shape_of(souther_string_of_utf8 as unsafe extern "C" fn(*const u8, Count) -> M),
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
            shape_of(souther_case_int_read as unsafe extern "C" fn(*const Value) -> i64),
        ),
        (
            "souther_case_bool_make",
            shape_of(souther_case_bool_make as extern "C" fn(i8) -> *const Value),
        ),
        (
            "souther_case_bool_read",
            shape_of(souther_case_bool_read as unsafe extern "C" fn(*const Value) -> i8),
        ),
        (
            "souther_case_string_make",
            shape_of(souther_case_string_make as extern "C" fn(T) -> *const Value),
        ),
        (
            "souther_case_string_read",
            shape_of(souther_case_string_read as unsafe extern "C" fn(*const Value) -> T),
        ),
        (
            "souther_case_decimal_make",
            shape_of(souther_case_decimal_make as extern "C" fn(*const Decimal) -> *const Value),
        ),
        (
            "souther_case_decimal_read",
            shape_of(
                souther_case_decimal_read as unsafe extern "C" fn(*const Value) -> *const Decimal,
            ),
        ),
        (
            "souther_decimal_of_parts",
            shape_of(souther_decimal_of_parts as unsafe extern "C" fn(T, i64) -> *mut Decimal),
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
            shape_of(souther_string_from_decimal as unsafe extern "C" fn(*const Decimal) -> M),
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
            shape_of(souther_case_date_read as unsafe extern "C" fn(*const Value) -> *const Date),
        ),
        (
            "souther_date_of_iso",
            shape_of(souther_date_of_iso as unsafe extern "C" fn(T) -> *mut Date),
        ),
        (
            "souther_date_literal",
            shape_of(souther_date_literal as unsafe extern "C" fn(T) -> *mut Date),
        ),
        (
            "souther_date_iso",
            shape_of(souther_date_iso as unsafe extern "C" fn(*const Date) -> M),
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
            shape_of(souther_case_time_read as unsafe extern "C" fn(*const Value) -> *const Time),
        ),
        (
            "souther_time_of_iso",
            shape_of(souther_time_of_iso as unsafe extern "C" fn(T) -> *mut Time),
        ),
        (
            "souther_time_literal",
            shape_of(souther_time_literal as unsafe extern "C" fn(T) -> *mut Time),
        ),
        (
            "souther_time_iso",
            shape_of(souther_time_iso as unsafe extern "C" fn(*const Time) -> M),
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
                souther_case_datetime_read as unsafe extern "C" fn(*const Value) -> *const DateTime,
            ),
        ),
        (
            "souther_datetime_of_iso",
            shape_of(souther_datetime_of_iso as unsafe extern "C" fn(T) -> *mut DateTime),
        ),
        (
            "souther_datetime_literal",
            shape_of(souther_datetime_literal as unsafe extern "C" fn(T) -> *mut DateTime),
        ),
        (
            "souther_datetime_iso",
            shape_of(souther_datetime_iso as unsafe extern "C" fn(*const DateTime) -> M),
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
                souther_case_instant_read as unsafe extern "C" fn(*const Value) -> *const Instant,
            ),
        ),
        (
            "souther_instant_of_iso",
            shape_of(souther_instant_of_iso as unsafe extern "C" fn(T) -> *mut Instant),
        ),
        (
            "souther_instant_literal",
            shape_of(souther_instant_literal as unsafe extern "C" fn(T) -> *mut Instant),
        ),
        (
            "souther_instant_iso",
            shape_of(souther_instant_iso as unsafe extern "C" fn(*const Instant) -> M),
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
            "souther_issue_meta_count",
            shape_of(souther_issue_meta_count as unsafe extern "C" fn(*const Issue) -> Count),
        ),
        (
            "souther_issue_meta_key",
            shape_of(souther_issue_meta_key as unsafe extern "C" fn(*const Issue, Count) -> T),
        ),
        (
            "souther_issue_meta_value",
            shape_of(souther_issue_meta_value as unsafe extern "C" fn(*const Issue, Count) -> T),
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

/// Every function the runtime defines for another party is in exactly one of the two tables, and
/// is written above: read off the source, so a function added here and to neither table is caught
/// rather than called by someone the tables say nothing to.
#[test]
fn every_function_the_runtime_defines_is_in_one_table() {
    let sources = [
        include_str!("lib.rs"),
        include_str!("decoding.rs"),
        include_str!("external.rs"),
        include_str!("document.rs"),
        include_str!("kernels.rs"),
        include_str!("decimal.rs"),
        include_str!("temporal.rs"),
    ];
    let marker = "extern \"C\" fn ";
    let mut defined = BTreeSet::new();
    for source in sources {
        for (at, _) in source.match_indices(marker) {
            let rest = &source[at + marker.len()..];
            let name: String = rest
                .chars()
                .take_while(|it| it.is_ascii_alphanumeric() || *it == '_')
                .collect();
            if name.starts_with("souther_") {
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

/// A case a host makes a value of is a case the runtime defines a token for, and every one is:
/// the two tables name the same cases in the same order.
#[test]
fn a_host_makes_every_case_the_runtime_has_a_token_for() {
    let crossed: Vec<&str> = HOST_CASES.iter().map(|it| it.case).collect();
    assert_eq!(crossed, BUILT_IN_CASES);
}

/// What a host makes of a case and what it reads back are what went in, and the value says it is
/// the case: the token at its front is the one the runtime defines for it.
#[test]
fn a_case_a_host_makes_reads_back_as_what_it_holds() {
    let mark = souther_mark();
    let which = |value: *const Value| unsafe { value.cast::<*const u8>().read() };
    let int = souther_case_int_make(-42);
    assert_eq!(which(int), CASE_INT.as_ptr());
    assert_eq!(unsafe { souther_case_int_read(int) }, -42);
    for truth in [0, 1] {
        let bool = souther_case_bool_make(truth);
        assert_eq!(which(bool), CASE_BOOL.as_ptr());
        assert_eq!(unsafe { souther_case_bool_read(bool) }, truth);
    }
    // SAFETY: three bytes of UTF-8 at the address handed over.
    let text = unsafe { souther_string_of_utf8("hé".as_ptr(), Count(3)) };
    let string = souther_case_string_make(text);
    assert_eq!(which(string), CASE_STRING.as_ptr());
    assert_eq!(
        unsafe { souther_case_string_read(string) },
        text.cast_const()
    );
    assert_eq!(which(souther_case_none_make()), CASE_NONE.as_ptr());
    souther_reset(mark);
}
