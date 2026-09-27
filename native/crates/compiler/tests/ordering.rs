//! Which enumeration orders a value, where the value's own type is not one.
//!
//! A case is ordered by the one enumeration that lists it, and a union of cases by the one that
//! lists every member (ADR-0069). A unit may be a case of two enumerations that place it
//! differently — `m.B`, which both `m.S` and `m.T` list here — and then it has no order of its
//! own: the checker never writes `m.B < m.B` on its own account, only a comparison some wider
//! context settles one enumeration for (`m.S < m.B`, where `m.S` is a union naming the pair). What
//! this driver reads is the checker's own answer, carried on the node (`ordering`) and not
//! rediscovered from the declarations it happens to carry, so most of what is checked below asks
//! whether a basis this side is handed is trusted correctly — not whether `m.B` orders against
//! itself, which no program does.

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
    // `m.P`, a product, and `m.D`, a sum of `m.A` and `m.P` that travels discriminated because one
    // of its cases is not a unit: neither is an enumeration, and no basis this side trusts without
    // asking.
    let product = r#"{"module":"m","name":"P","by":"amodule","is":"product","fields":[{"name":"n","binding":0,"codec":{"is":"scalar","scalar":"INT"}}],"invariants":[]}"#;
    let discriminated = format!(
        r#"{{"module":"m","name":"D","by":"amodule","is":"sum","cases":[{},{}],"form":{{"is":"discriminated","tag":"type","contents":"value"}}}}"#,
        case("A"),
        case("P"),
    );
    format!(
        r#"{{"transport":{TRANSPORT_VERSION},"declarations":[{},{},{},{},{},{}],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{helper}],"values":[],"entries":[],"definitions":[],"examples":[]}}]}}"#,
        unit("A"),
        unit("B"),
        sum("S", &["A", "B"]),
        sum("T", &["B"]),
        product,
        discriminated,
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

/// `m.B` is a case of both `m.S` and `m.T`, so `m.B < m.B` is not one the checker writes on its
/// own account — a robustness test of this driver and not of a document the checker states, since
/// which of the two would place it is exactly what has no answer for a comparison with no wider
/// context to settle it. What this asks is narrower: this driver trusts whichever basis a node
/// names, S or T, and does not go looking for the other one instead once it has one that fits.
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

/// A basis is trusted once it is checked to be one that could place every value compared, not
/// before: `m.A | m.B` ordered by `m.T`, which lists `m.B` alone, is a basis with no place for
/// `m.A` in it. Read on trust, `place` would fall `m.A` to whatever position `m.T`'s leaves leave
/// for a tag none of them names — comparing wrongly and not refusing at all — so this is refused as
/// the two halves disagreeing before a value is ever placed.
#[test]
fn a_basis_that_does_not_place_every_case_compared_is_the_halves_disagreeing() {
    let union =
        r#"{"union":[{"is":"declared","declared":"m.A"},{"is":"declared","declared":"m.B"}]}"#;
    let refused =
        object_for(&ordering(union, &as_enum("T"))).expect_err("a basis with no place for m.A");
    assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
    assert!(refused.to_string().contains("does not place"), "{refused}");
}

/// `leaves_of` answers a declaration whole where it is not a sum, so a product named as its own
/// basis would place every value of it alike and pass the coverage check above with nothing left
/// to name: `m.P < m.P`, ordered by `m.P`, has `[m.P]` on both sides. A product has no order of its
/// own (Souther never writes this), and `ordering::ordered` reads any declared case as an
/// enumeration once it is trusted, so untrusted this would place every `m.P` at the one ordinal a
/// sum of one leaf gives it — comparing wrongly rather than refusing. A basis has to be an
/// enumeration before it is asked what it places.
#[test]
fn a_basis_that_is_not_a_sum_is_the_halves_disagreeing() {
    let p = r#"{"ref":{"is":"declared","declared":"m.P"}}"#;
    let refused = object_for(&ordering(p, p)).expect_err("a product is no enumeration");
    assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
    assert!(refused.to_string().contains("no enumeration"), "{refused}");
}

/// A discriminated sum's cases carry more than which one they are, so a token alone does not place
/// them the way an enumeration's leaves are placed; `m.D`, whose case `m.P` is not a unit, travels
/// discriminated and is refused as a basis the same way a product is.
#[test]
fn a_basis_that_is_a_discriminated_sum_is_the_halves_disagreeing() {
    let refused = object_for(&ordering(
        r#"{"ref":{"is":"declared","declared":"m.A"}}"#,
        &as_enum("D"),
    ))
    .expect_err("a discriminated sum is no enumeration");
    assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
    assert!(refused.to_string().contains("no enumeration"), "{refused}");
}
