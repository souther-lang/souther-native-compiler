//! What a run's lifetime lets a program say and what it refuses, held to the compiler: each file
//! under `lifetimes/pass` compiles, and each under `lifetimes/fail` is refused with what its
//! `.stderr` beside it says. Not read off the types' definitions, which say what a lifetime is
//! meant to do and not what the compiler makes of it.

#[test]
fn lifetimes() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/lifetimes/pass/*.rs");
    cases.compile_fail("tests/lifetimes/fail/*.rs");
}
