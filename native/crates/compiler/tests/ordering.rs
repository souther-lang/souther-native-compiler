//! Which enumeration orders a value, where the value's own type is not one.
//!
//! A case is ordered by the one enumeration that lists it, and a union of cases by the one that
//! lists every member (ADR-0069). A case two enumerations list is placed differently by each, so
//! it has no order of its own; the checker never writes one, and this driver, handed one, does
//! not pick either.

use souther_native_driver::{NotLowered, object_for};

/// The units `m.A` and `m.B`, the enumeration `m.S = m.A | m.B`, and the enumeration `m.T = m.B`:
/// `m.A` is placed by `m.S` alone, and `m.B` by both. A helper orders two values of `ty` as they
/// stand.
fn ordering(ty: &str) -> String {
    let read = |at: u32| format!(r#"{{"core":"read","binding":{at},"type":{ty},"aborts":[]}}"#);
    let body = format!(
        r#"{{"core":"binary","op":"LT","reading":{{"is":"astheystand"}},"left":{},"right":{},"type":{{"prim":"BOOL"}},"aborts":[]}}"#,
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
        r#"{{"transport":25,"declarations":[{},{},{},{}],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{helper}],"values":[],"entries":[],"definitions":[],"examples":[]}}]}}"#,
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

#[test]
fn a_case_one_enumeration_lists_is_ordered_by_it() {
    is_lowered(&ordering(r#"{"ref":{"is":"declared","declared":"m.A"}}"#));
}

#[test]
fn an_enumeration_is_ordered_by_itself_though_another_lists_its_cases() {
    is_lowered(&ordering(r#"{"ref":{"is":"declared","declared":"m.S"}}"#));
    is_lowered(&ordering(r#"{"ref":{"is":"declared","declared":"m.T"}}"#));
}

/// `m.A | m.B` is placed by `m.S` and by nothing else, since `m.T` does not list `m.A`: what orders
/// a union is the enumeration every member shares, not one each member has alone.
#[test]
fn a_union_is_ordered_by_the_one_enumeration_listing_every_member() {
    let union =
        r#"{"union":[{"is":"declared","declared":"m.A"},{"is":"declared","declared":"m.B"}]}"#;
    is_lowered(&ordering(union));
}

#[test]
fn a_case_two_enumerations_list_is_not_ordered_by_either() {
    let refused = object_for(&ordering(r#"{"ref":{"is":"declared","declared":"m.B"}}"#))
        .expect_err("placed twice");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(
        refused.to_string().contains("no one enumeration"),
        "{refused}"
    );
}
