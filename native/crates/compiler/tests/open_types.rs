//! A type variable no call of a helper settles, and what asks what it is.
//!
//! A copy of a helper replaces the variables a call settles and leaves the others as written. What
//! is refused is what asks of one what it is, where it asks: a list held over one is a pointer and
//! needs nothing of it, and two of them compared need its comparison.

use souther_native_driver::{NotLowered, object_for};

const INT: &str = r#"{"prim":"INT"}"#;
const BOOL: &str = r#"{"prim":"BOOL"}"#;
const OPEN_LIST: &str = r#"{"list":{"var":0}}"#;

fn node(core: &str, fields: &str, ty: &str) -> String {
    let fields = if fields.is_empty() {
        String::new()
    } else {
        format!("{fields},")
    };
    format!(r#"{{"core":"{core}",{fields}"type":{ty},"aborts":[]}}"#)
}

fn read(binding: usize, ty: &str) -> String {
    node("read", &format!(r#""binding":{binding}"#), ty)
}

/// A helper `m.h` taking an `Int`, holding the empty list over a variable no call settles as
/// binding 1, and answering `body` as `answers`; and the one value `v` calling it.
fn holding(body: &str, answers: &str) -> String {
    let bound = node(
        "let",
        &format!(
            r#""binding":1,"binds":{OPEN_LIST},"value":{},"body":{body}"#,
            node("list", r#""elements":[]"#, OPEN_LIST)
        ),
        answers,
    );
    let held = format!(
        r#"{{"reached":{{"is":"own","module":"m","name":"h"}},"parameters":[{{"name":"n","type":{INT}}}],"body":{bound}}}"#
    );
    let called = node(
        "call",
        &format!(
            r#""reaches":{{"is":"helper","reached":{{"is":"own","module":"m","name":"h"}}}},"arguments":[{}]"#,
            node("int", r#""value":1"#, INT)
        ),
        answers,
    );
    format!(
        r#"{{"transport":27,"declarations":[],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{held}],"values":[{{"module":"m","name":"v","handovers":[],"body":{called}}}],"entries":[],"definitions":[],"examples":[]}}]}}"#
    )
}

/// The list is held and never read: nothing asks what its elements are.
#[test]
fn a_list_over_a_variable_no_call_settles_is_lowered_where_nothing_asks_what_it_holds() {
    let document = holding(&node("int", r#""value":1"#, INT), INT);
    if let Err(refused) = object_for(&document) {
        panic!("refused: {refused}");
    }
}

/// Two of them compared ask for the comparison of what they hold, which is not known: refused as
/// not lowered, and not a panic.
#[test]
fn a_comparison_of_lists_over_a_variable_no_call_settles_is_not_lowered() {
    let compared = node(
        "binary",
        &format!(
            r#""op":"EQ","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
            read(1, OPEN_LIST),
            read(1, OPEN_LIST)
        ),
        BOOL,
    );
    let refused = object_for(&holding(&compared, BOOL))
        .expect_err("what the elements are is what their comparison is");
    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "the backend not writing this, and not the halves disagreeing: {refused}"
    );
    assert!(
        refused.to_string().contains("type variable 0"),
        "refused for what it is asked of the variable, and not for something else: {refused}"
    );
}
