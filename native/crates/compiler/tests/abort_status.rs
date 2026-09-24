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

/// The fixture both this test and the Java harness read.
const FIXTURE: &str = include_str!("abort-status-abi2.json");

#[test]
fn the_fixture_the_java_harness_is_held_to_is_what_native_status_answers_today() {
    let mut written = String::from("{");
    for (at, kind) in AbortKind::ALL.iter().enumerate() {
        if at > 0 {
            written.push(',');
        }
        written.push_str(&format!("\"{}\":{}", kind.spelt(), native_status(*kind)));
    }
    written.push('}');
    assert_eq!(FIXTURE.trim(), written);
}

/// Every reason a computation ends is its own number, and none is `ANSWERED`: a caller tells them
/// apart by the number alone, and the Java harness, the header and the manifest all read it back
/// that way. The fixture above would carry two names for one number without complaint.
#[test]
fn every_abort_answers_a_number_of_its_own() {
    let mut seen = vec![souther_native_abi::ANSWERED];
    for kind in AbortKind::ALL {
        let number = native_status(kind);
        assert!(
            !seen.contains(&number),
            "{} answers {number}, which is already answered",
            kind.spelt()
        );
        seen.push(number);
    }
}
