//! What this driver says no to, and which no it is.
//!
//! Two of them and they are not the same. A program the language admits and this backend does not
//! write yet is the backend's shortcoming; a document this driver cannot read is the two halves
//! disagreeing about what they are saying to each other. The half that started the driver reports
//! them to different people, so what is checked here is that they arrive apart.

use souther_native_driver::{NotLowered, object_for};

/// One behavior over one primitive, joined by one operator — the smallest document that reaches
/// the lowering, with the two things under test as its only variables.
fn document(op: &str, ty: &str) -> String {
    let prim = format!(r#"{{"prim":"{ty}"}}"#);
    let read = |at: u32| format!(r#"{{"core":"read","binding":{at},"type":{prim}}}"#);
    let body =
        format!(r#"{{"core":"binary","op":"{op}","left":{},"right":{},"type":{prim}}}"#, read(0), read(1));
    let behavior = format!(
        r#"{{"name":"f","parameters":["a","b"],"takes":[{prim},{prim}],"answers":{prim},"body":{body}}}"#
    );
    format!(r#"{{"transport":1,"modules":[{{"name":"calculation","behaviors":[{behavior}]}}]}}"#)
}

/// An operator crosses whether or not there is a lowering for it. What it means is the language's
/// and the writer's business; whether this can write it is this driver's, and it is answered here.
#[test]
fn an_operator_with_no_lowering_is_not_lowered_rather_than_unreadable() {
    let refused = object_for(&document("SUB", "INT")).expect_err("no lowering for it");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "read as something other than a lowering this driver has not got: {refused}"
    );
    assert!(refused.to_string().contains('-'), "{refused}");
}

/// The same for a primitive. A type with no machine representation yet is a lowering this driver
/// has not got, not a document it failed to understand.
#[test]
fn a_primitive_with_no_representation_is_not_lowered() {
    let refused = object_for(&document("ADD", "STRING")).expect_err("no representation for it");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "read as something other than a lowering this driver has not got: {refused}"
    );
    assert!(refused.to_string().contains("String"), "{refused}");
}

/// A field nothing here names is the writer saying something this driver has no idea it was told.
/// Reading past it would be compiling a program that means more than what was understood of it.
#[test]
fn a_field_this_driver_does_not_know_is_refused_rather_than_skipped() {
    let said_more = document("ADD", "INT").replace(
        r#""core":"binary""#,
        r#""core":"binary","somethingNewlySettled":true"#,
    );

    let refused = object_for(&said_more).expect_err("a document saying more than this reads");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
}

/// A transport this driver does not read is refused whole. Reading the parts that happen to parse
/// would be reading a document written to mean something else.
#[test]
fn a_transport_from_another_version_is_refused() {
    let later = document("ADD", "INT").replace(r#""transport":1"#, r#""transport":2"#);

    let refused = object_for(&later).expect_err("a version this does not read");

    assert!(refused.to_string().contains('2'), "{refused}");
}
