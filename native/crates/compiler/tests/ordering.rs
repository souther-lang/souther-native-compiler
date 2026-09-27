//! What orders a value of a case, a union of cases or an enumeration: the order the checker says
//! it is placed on (`Core.OrderingBasis`), which the comparison carries.
//!
//! A case two enumerations list is placed differently by each (ADR-0069), so which one orders it
//! is the checker's answer and not the case's. This driver reads that answer and does not look for
//! one; what it holds is that the order it is handed places the value at all.

use souther_native_driver::{NotLowered, object_for};

/// The units `m.A` and `m.B`, the enumeration `m.S = m.A | m.B`, and the enumeration `m.T = m.B`:
/// `m.A` is listed by `m.S` alone, and `m.B` by both. A helper orders two values of `ty` as they
/// stand, on the order of `by`.
fn ordering(ty: &str, by: &str) -> String {
    let by = format!(r#"{{"ref":{{"is":"declared","declared":"m.{by}"}}}}"#);
    let read = |at: u32| format!(r#"{{"core":"read","binding":{at},"type":{ty},"aborts":[]}}"#);
    let body = format!(
        r#"{{"core":"binary","op":"LT","reading":{{"is":"astheystand"}},"ordering":{by},"left":{},"right":{},"type":{{"prim":"BOOL"}},"aborts":[]}}"#,
        read(0),
        read(1)
    );
    let helper = format!(
        r#"{{"reached":{{"is":"own","module":"m","name":"h"}},"parameters":[{{"name":"a","type":{ty}}},{{"name":"b","type":{ty}}}],"body":{body}}}"#
    );
    let unit =
        |name: &str| format!(r#"{{"module":"m","name":"{name}","by":"amodule","is":"unit"}}"#);
    let case = |name: &str| format!(r#"{{"is":"declared","declared":"m.{name}"}}"#);
    let sum = |name: &str, cases: &[&str]| {
        let cases: Vec<String> = cases.iter().map(|it| case(it)).collect();
        format!(
            r#"{{"module":"m","name":"{name}","by":"amodule","is":"sum","cases":[{}],"form":{{"is":"enumeration"}}}}"#,
            cases.join(",")
        )
    };
    format!(
        r#"{{"transport":28,"declarations":[{},{},{},{}],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{helper}],"values":[],"entries":[],"definitions":[],"examples":[]}}]}}"#,
        unit("A"),
        unit("B"),
        sum("S", &["A", "B"]),
        sum("T", &["B"]),
    )
}

fn is_lowered(document: &str) {
    if let Err(refused) = object_for(document) {
        panic!("an order the checker could have written is refused: {refused}");
    }
}

fn is_the_halves_disagreeing(document: &str, naming: &str) {
    let refused = object_for(document).expect_err("an order that does not place the value");
    assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
    assert!(refused.to_string().contains(naming), "{refused}");
}

const A: &str = r#"{"ref":{"is":"declared","declared":"m.A"}}"#;
const B: &str = r#"{"ref":{"is":"declared","declared":"m.B"}}"#;

#[test]
fn a_case_is_ordered_by_the_enumeration_the_checker_says() {
    is_lowered(&ordering(A, "S"));
}

/// `m.B` is listed by both, and is placed on whichever order the comparison carries: the choice is
/// the checker's, and nothing here makes it again.
#[test]
fn a_case_two_enumerations_list_is_ordered_by_the_one_carried() {
    is_lowered(&ordering(B, "S"));
    is_lowered(&ordering(B, "T"));
}

#[test]
fn an_enumeration_is_ordered_by_itself() {
    is_lowered(&ordering(
        r#"{"ref":{"is":"declared","declared":"m.S"}}"#,
        "S",
    ));
    is_lowered(&ordering(
        r#"{"ref":{"is":"declared","declared":"m.T"}}"#,
        "T",
    ));
}

#[test]
fn a_union_is_ordered_by_an_enumeration_listing_every_member() {
    let union =
        r#"{"union":[{"is":"declared","declared":"m.A"},{"is":"declared","declared":"m.B"}]}"#;
    is_lowered(&ordering(union, "S"));
    is_the_halves_disagreeing(&ordering(union, "T"), "does not list m.A");
}

/// An order that does not list the case, and a declaration that is no enumeration, place nothing.
#[test]
fn an_order_that_does_not_place_the_value_is_the_halves_disagreeing() {
    is_the_halves_disagreeing(&ordering(A, "T"), "does not list m.A");
    is_the_halves_disagreeing(&ordering(A, "A"), "no order the language has");
}
