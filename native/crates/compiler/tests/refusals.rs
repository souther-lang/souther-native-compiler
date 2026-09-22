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
    over(op, &format!(r#"{{"prim":"{ty}"}}"#))
}

/// The same over any type the writer can name, for the ones no Souther source reaches this way.
fn over(op: &str, prim: &str) -> String {
    let read = |at: u32| format!(r#"{{"core":"read","binding":{at},"type":{prim}}}"#);
    let body =
        format!(r#"{{"core":"binary","op":"{op}","left":{},"right":{},"type":{prim}}}"#, read(0), read(1));
    let target = format!(
        r#"{{"module":"calculation","name":"f","is":"body","takes":[{prim},{prim}],"answers":{prim}}}"#
    );
    let held = format!(
        r#"{{"declared":"calculation.f","parameters":["a","b"],"publication":"published","body":{body}}}"#
    );
    format!(
        r#"{{"transport":3,"declarations":[],"behaviors":[{target}],"modules":[{{"name":"calculation","helpers":[],"bodies":[{held}],"examples":[]}}]}}"#
    )
}

/// An operator crosses whether or not there is a lowering for it. What it means is the language's
/// and the writer's business; whether this can write it is this driver's, and it is answered here.
#[test]
fn an_operator_with_no_lowering_is_not_lowered_rather_than_unreadable() {
    let refused = object_for(&document("DIV", "INT")).expect_err("no lowering for it");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "read as something other than a lowering this driver has not got: {refused}"
    );
    assert!(refused.to_string().contains('/'), "{refused}");
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

/// A sum of two values of a declared type, which is the address each of them is held as.
///
/// No Souther source produces this: what crosses for `a + b` over a newtype is a construction of
/// the newtype over the sum of the two wrapped numbers, so the operands arrive as `Int`. Written
/// by hand for that reason — the arm that answers it is what keeps this driver from depending on
/// an arrangement made on the other side of the transport, and an `iadd` over two addresses would
/// have answered an address that points at neither.
#[test]
fn a_sum_of_two_addresses_is_the_halves_disagreeing() {
    let document = over("ADD", r#"{"declared":"counting.Amount"}"#);

    let refused = object_for(&document).expect_err("nothing here adds two addresses");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("counting.Amount"), "{refused}");
}

/// An ordering over two truths, which is not a program the language admits: `Bool` is not one of
/// the ordered types. So it is the two halves disagreeing about what they are saying to each other
/// rather than this backend being behind, and the author of the program is not the one who can do
/// anything about it.
///
/// There is no Souther source that produces this, which is why it is written as a document by
/// hand. What it holds is the arm that answers it — a comparison decided by the machine width
/// would have run it as an `icmp` over two bytes and answered something.
#[test]
fn an_ordering_over_two_truths_is_the_halves_disagreeing_and_not_a_lowering_this_has_not_got() {
    let refused = object_for(&document("LT", "BOOL")).expect_err("nothing orders two truths");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("Bool"), "{refused}");
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
    let later = document("ADD", "INT").replace(r#""transport":3"#, r#""transport":4"#);

    let refused = object_for(&later).expect_err("a version this does not read");

    assert!(refused.to_string().contains('4'), "{refused}");
}
