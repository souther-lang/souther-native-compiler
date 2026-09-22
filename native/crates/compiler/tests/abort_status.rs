//! `native_status`'s table, held to a fixture a Java test reads too.
//!
//! `AbortKind -> Status` is written once, in `native_status`, and read a second time by
//! `Running`'s Java test harness — which has to turn a run's own status back into the `AbortKind`
//! it came from to assert anything about which one it was, and cannot reach a private Rust
//! function to ask. That second reading is a hand-written copy of this table, and a member spelt
//! a different number on either side would read without complaint and mean something other than
//! what either side thinks it does — the same failure mode `vocabularies.rs` closes for `Op` and
//! `Prim`, closed here the same way: this side writes the fixture out from what `native_status`
//! actually answers today, and the other side's test reads it back and is held to it.

use souther_native_driver::native_status;
use souther_native_driver::transport::AbortKind;

/// How a member is spelt on the wire, for the same reason `ProgramWriter.abort` (Java) and
/// `transport::AbortKind`'s own `#[serde(rename = ...)]` are: exhaustive, so a member added
/// upstream stops this compiling before it can be left out of the fixture below.
fn spelt(kind: AbortKind) -> &'static str {
    match kind {
        AbortKind::InvariantNotHeld => "INVARIANT_NOT_HELD",
        AbortKind::EnsuresNotHeld => "ENSURES_NOT_HELD",
        AbortKind::UnreachableReached => "UNREACHABLE_REACHED",
        AbortKind::DivisionByZero => "DIVISION_BY_ZERO",
        AbortKind::RequiredFormHasNoPlace => "REQUIRED_FORM_HAS_NO_PLACE",
        AbortKind::InvalidBounds => "INVALID_BOUNDS",
    }
}

/// Every member, in the order the fixture lists them. Kept beside `spelt` rather than derived
/// from it — Rust has no reflection over an enum's own members, the same reason `vocabularies.rs`
/// lists `Op`'s and `Prim`'s members by hand rather than asking the type for them. Not itself
/// exhaustiveness-checked: a member `AbortKind` gains stops both `spelt` above and `native_status`
/// from compiling until it is answered for, and whoever is already in this file fixing that is
/// the one place this list is asked to keep up — the same margin `vocabularies.rs` accepts for
/// the lists it writes by hand.
const ALL: [AbortKind; 6] = [
    AbortKind::InvariantNotHeld,
    AbortKind::EnsuresNotHeld,
    AbortKind::UnreachableReached,
    AbortKind::DivisionByZero,
    AbortKind::RequiredFormHasNoPlace,
    AbortKind::InvalidBounds,
];

/// The fixture both this test and the Java harness read.
const FIXTURE: &str = include_str!("abort-status-abi2.json");

#[test]
fn the_fixture_the_java_harness_is_held_to_is_what_native_status_answers_today() {
    let mut written = String::from("{");
    for (at, kind) in ALL.iter().enumerate() {
        if at > 0 {
            written.push(',');
        }
        written.push_str(&format!("\"{}\":{}", spelt(*kind), native_status(*kind)));
    }
    written.push('}');
    assert_eq!(FIXTURE.trim(), written);
}
