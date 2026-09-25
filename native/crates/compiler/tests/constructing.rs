//! What a body reaches through a capability is what the behavior was constructed with, and nothing
//! else: a body calling a behavior a host implements without being constructed with it, or a
//! composition holding a stage whose requirements it was not handed, is the two halves disagreeing
//! about what constructing the behavior takes, and lowered it would be a call through a capability
//! nothing holds.

use serde_json::{Value, json};
use souther_native_driver::object_for;

/// `m.twice`, which requires `m.lookUp`, and `m.looked`, a composition of `m.lookUp` and
/// `m.doubled` requiring `m.lookUp`.
const ENSURES: &str = include_str!("ensures.transport.json");

/// The document with what `declared` requires replaced by nothing.
fn requiring_nothing(declared: &str) -> String {
    let mut document: Value = serde_json::from_str(ENSURES).unwrap();
    let definitions = document["modules"][0]["definitions"]
        .as_array_mut()
        .unwrap();
    let definition = definitions
        .iter_mut()
        .find(|it| it["declared"] == declared)
        .unwrap();
    definition["requirements"] = json!([]);
    document.to_string()
}

#[test]
fn a_body_calling_what_a_host_implements_without_being_constructed_with_it_is_refused() {
    let refused =
        object_for(&requiring_nothing("m.twice")).expect_err("a call nothing was handed for");

    assert!(
        refused
            .to_string()
            .contains("a call of m.lookUp, which a host implements, from a body not constructed"),
        "{refused}"
    );
}

#[test]
fn a_composition_not_handed_what_its_stage_is_is_refused() {
    let refused =
        object_for(&requiring_nothing("m.looked")).expect_err("a stage nothing was handed for");

    assert!(
        refused
            .to_string()
            .contains("m.looked's stage m.lookUp is implemented by a host and is not among"),
        "{refused}"
    );
}
