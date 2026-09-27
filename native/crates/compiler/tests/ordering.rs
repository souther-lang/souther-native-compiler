//! Which enumeration orders a value, where the value's own type is not one.
//!
//! A case is ordered by the one enumeration that lists it, and a union of cases by the one that
//! lists every member (ADR-0069). A unit may be a case of two enumerations that place it
//! differently, so it has no order of its own unless one is named for it; which one is the
//! checker's own answer, carried on the node (`ordering`) and not something this driver works out
//! again from the declarations it happens to carry. So `m.B`,
//! which `m.S` and `m.T` both list, orders by whichever the checker names — this driver never
//! finds two where the checker found one, because it is never asked to find one at all.

use souther_native_driver::transport::TRANSPORT_VERSION;
use souther_native_driver::{NotLowered, object_for};

/// The units `m.A` and `m.B`, the enumeration `m.S = m.A | m.B`, and the enumeration `m.T = m.B`:
/// `m.A` is placed by `m.S` alone, and `m.B` by both. A helper orders two values of `ty` as they
/// stand, by the enumeration `basis` names.
fn ordering(ty: &str, basis: &str) -> String {
    let read = |at: u32| format!(r#"{{"core":"read","binding":{at},"type":{ty},"aborts":[]}}"#);
    let body = format!(
        r#"{{"core":"binary","op":"LT","reading":{{"is":"astheystand"}},"ordering":{basis},"left":{},"right":{},"type":{{"prim":"BOOL"}},"aborts":[]}}"#,
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
        r#"{{"transport":{TRANSPORT_VERSION},"declarations":[{},{},{},{}],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{helper}],"values":[],"entries":[],"definitions":[],"examples":[]}}]}}"#,
        unit("A"),
        unit("B"),
        sum("S", &["A", "B"]),
        sum("T", &["B"]),
    )
}

/// `ty`, ordered by the declared enumeration `basis`.
fn as_enum(basis: &str) -> String {
    format!(r#"{{"ref":{{"is":"declared","declared":"m.{basis}"}}}}"#)
}

fn is_lowered(document: &str) {
    if let Err(refused) = object_for(document) {
        panic!("an order the checker could have written is refused: {refused}");
    }
}

#[test]
fn a_case_one_enumeration_lists_is_ordered_by_it() {
    is_lowered(&ordering(
        r#"{"ref":{"is":"declared","declared":"m.A"}}"#,
        &as_enum("S"),
    ));
}

#[test]
fn an_enumeration_is_ordered_by_itself_though_another_lists_its_cases() {
    is_lowered(&ordering(
        r#"{"ref":{"is":"declared","declared":"m.S"}}"#,
        &as_enum("S"),
    ));
    is_lowered(&ordering(
        r#"{"ref":{"is":"declared","declared":"m.T"}}"#,
        &as_enum("T"),
    ));
}

/// `m.A | m.B` is placed by `m.S`, and by nothing else since `m.T` does not list `m.A` — but that
/// is a fact about what the checker could have written, not one this driver rediscovers: it takes
/// whichever enumeration the node names.
#[test]
fn a_union_is_ordered_by_the_enumeration_the_checker_names() {
    let union =
        r#"{"union":[{"is":"declared","declared":"m.A"},{"is":"declared","declared":"m.B"}]}"#;
    is_lowered(&ordering(union, &as_enum("S")));
}

/// `m.B` is a case of both `m.S` and `m.T`, so it has no order of its own — but the checker, and
/// not this driver, is the one that would have had to choose between them, and it names whichever
/// one a program ordered `m.B` by. This driver lowers either.
#[test]
fn a_case_two_enumerations_list_is_ordered_by_whichever_the_checker_names() {
    let b = r#"{"ref":{"is":"declared","declared":"m.B"}}"#;
    is_lowered(&ordering(b, &as_enum("S")));
    is_lowered(&ordering(b, &as_enum("T")));
}

/// A comparison over a case or a union with no basis named for it is a document the checker never
/// writes (`Core.Binary`'s own constructor holds a written comparison to naming one), and this
/// driver refuses it as the two halves disagreeing rather than guessing one.
#[test]
fn a_case_ordered_with_no_basis_named_is_the_halves_disagreeing() {
    let refused = object_for(&ordering(
        r#"{"ref":{"is":"declared","declared":"m.A"}}"#,
        "null",
    ))
    .expect_err("a comparison with no ordering basis");
    assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
    assert!(
        refused.to_string().contains("settles no basis"),
        "{refused}"
    );
}
