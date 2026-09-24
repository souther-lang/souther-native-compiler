//! `HOST_RUNTIME`, held to the functions it names.
//!
//! The table is what a header declares and a shared library exports, and the functions are Rust.
//! Nothing between the two checks them against each other: a C compiler reading the header trusts
//! it, and a linker resolves a name whatever it was declared as. So each function is written here
//! as the pointer type it is, which only compiles while that is what the function is, and what the
//! table says is held to that.

use crate::decoding::*;
use crate::*;
use souther_native_abi::{HOST_RUNTIME, HostParameter, HostWord};

/// What a word is on the machine, which is all a call across C agrees about.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Width {
    Byte,
    Four,
    Eight,
    Address,
}

fn width_of(word: HostWord) -> Width {
    match word {
        HostWord::Bool => Width::Byte,
        HostWord::Status | HostWord::Case | HostWord::Outcome => Width::Four,
        HostWord::Int | HostWord::Count | HostWord::Mark => Width::Eight,
        HostWord::Bytes
        | HostWord::Value
        | HostWord::String
        | HostWord::Decoded
        | HostWord::Issue => Width::Address,
    }
}

fn parameter_width(parameter: &HostParameter) -> Width {
    match parameter {
        HostParameter::Given(word) => width_of(*word),
        HostParameter::Room(_) => Width::Address,
    }
}

/// A Rust type as the width it crosses C at.
trait Crosses {
    const WIDTH: Width;
}

impl Crosses for i8 {
    const WIDTH: Width = Width::Byte;
}
impl Crosses for i32 {
    const WIDTH: Width = Width::Four;
}
impl Crosses for u32 {
    const WIDTH: Width = Width::Four;
}
impl Crosses for i64 {
    const WIDTH: Width = Width::Eight;
}
impl<T> Crosses for *const T {
    const WIDTH: Width = Width::Address;
}
impl<T> Crosses for *mut T {
    const WIDTH: Width = Width::Address;
}

/// What a function takes and what it answers, where it answers anything.
type Shape = (Vec<Width>, Option<Width>);

/// A function pointer as what it takes and answers.
trait Shaped {
    fn shape() -> Shape;
}

macro_rules! shaped {
    ($($taken:ident),*) => {
        impl<$($taken: Crosses,)* R: Crosses> Shaped for unsafe extern "C" fn($($taken),*) -> R {
            fn shape() -> Shape {
                (vec![$($taken::WIDTH),*], Some(R::WIDTH))
            }
        }
        impl<$($taken: Crosses,)* R: Crosses> Shaped for extern "C" fn($($taken),*) -> R {
            fn shape() -> Shape {
                (vec![$($taken::WIDTH),*], Some(R::WIDTH))
            }
        }
        impl<$($taken: Crosses),*> Shaped for unsafe extern "C" fn($($taken),*) {
            fn shape() -> Shape {
                (vec![$($taken::WIDTH),*], None)
            }
        }
        impl<$($taken: Crosses),*> Shaped for extern "C" fn($($taken),*) {
            fn shape() -> Shape {
                (vec![$($taken::WIDTH),*], None)
            }
        }
    };
}

shaped!();
shaped!(A);
shaped!(A, B);

fn shape_of<F: Shaped>(_: F) -> Shape {
    F::shape()
}

/// Every function a host calls, as the type it is. A cast from a function to a pointer of another
/// type does not compile, so what is written here is what each function is.
fn functions() -> Vec<(&'static str, Shape)> {
    vec![
        (
            "souther_mark",
            shape_of(souther_mark as extern "C" fn() -> i64),
        ),
        (
            "souther_reset",
            shape_of(souther_reset as extern "C" fn(i64)),
        ),
        (
            "souther_string_of_utf8",
            shape_of(souther_string_of_utf8 as unsafe extern "C" fn(*const u8, i64) -> *mut u8),
        ),
        (
            "souther_string_length",
            shape_of(souther_string_length as unsafe extern "C" fn(*const u8) -> i64),
        ),
        (
            "souther_string_bytes",
            shape_of(souther_string_bytes as unsafe extern "C" fn(*const u8) -> *const u8),
        ),
        (
            "souther_decoded_outcome",
            shape_of(souther_decoded_outcome as unsafe extern "C" fn(*const Decoding) -> i32),
        ),
        (
            "souther_decoded_value",
            shape_of(souther_decoded_value as unsafe extern "C" fn(*const Decoding) -> *const u8),
        ),
        (
            "souther_decoded_malformed_at",
            shape_of(souther_decoded_malformed_at as unsafe extern "C" fn(*const Decoding) -> i64),
        ),
        (
            "souther_decoded_issue_count",
            shape_of(souther_decoded_issue_count as unsafe extern "C" fn(*const Decoding) -> i64),
        ),
        (
            "souther_decoded_issue",
            shape_of(
                souther_decoded_issue as unsafe extern "C" fn(*const Decoding, i64) -> *const Issue,
            ),
        ),
        (
            "souther_issue_code",
            shape_of(souther_issue_code as unsafe extern "C" fn(*const Issue) -> *const u8),
        ),
        (
            "souther_issue_path",
            shape_of(souther_issue_path as unsafe extern "C" fn(*const Issue) -> *const u8),
        ),
        (
            "souther_issue_meta_count",
            shape_of(souther_issue_meta_count as unsafe extern "C" fn(*const Issue) -> i64),
        ),
        (
            "souther_issue_meta_key",
            shape_of(
                souther_issue_meta_key as unsafe extern "C" fn(*const Issue, i64) -> *const u8,
            ),
        ),
        (
            "souther_issue_meta_value",
            shape_of(
                souther_issue_meta_value as unsafe extern "C" fn(*const Issue, i64) -> *const u8,
            ),
        ),
    ]
}

#[test]
fn every_function_a_host_calls_is_what_the_table_says() {
    let functions = functions();
    let named: Vec<&str> = functions.iter().map(|(name, _)| *name).collect();
    let listed: Vec<&str> = HOST_RUNTIME.iter().map(|it| it.name).collect();
    assert_eq!(
        listed, named,
        "the table and the functions name the same ones"
    );
    for (function, (name, shape)) in HOST_RUNTIME.iter().zip(&functions) {
        let said = (
            function
                .takes
                .iter()
                .map(parameter_width)
                .collect::<Vec<_>>(),
            function.answers.map(width_of),
        );
        assert_eq!(&said, shape, "{name}");
    }
}
