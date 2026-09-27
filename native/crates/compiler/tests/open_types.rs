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
        r#"{{"transport":28,"declarations":[],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{held}],"values":[{{"module":"m","name":"v","handovers":[],"body":{called}}}],"entries":[],"definitions":[],"examples":[]}}]}}"#
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

const STRING: &str = r#"{"prim":"STRING"}"#;

/// A function value written in a helper over a variable is a closure in each copy of it, with the
/// layout and the signature that copy has: the one block, written once, is two functions.
#[test]
fn a_function_value_written_in_a_helper_over_a_variable_is_one_closure_in_each_copy() {
    let made = r#"{"fn":{"takes":[],"answers":{"var":0}}}"#;
    let held_var = node("read", r#""binding":0"#, r#"{"var":0}"#);
    let block = node(
        "block",
        &format!(r#""site":0,"parameters":[],"body":{held_var}"#),
        made,
    );
    let held = format!(
        r#"{{"reached":{{"is":"own","module":"m","name":"k"}},"parameters":[{{"name":"x","type":{{"var":0}}}}],"body":{block}}}"#
    );
    let at = |ty: &str| format!(r#"{{"fn":{{"takes":[],"answers":{ty}}}}}"#);
    let call = |argument: String, ty: &str| {
        node(
            "call",
            &format!(
                r#""reaches":{{"is":"helper","reached":{{"is":"own","module":"m","name":"k"}}}},"arguments":[{argument}]"#
            ),
            &at(ty),
        )
    };
    let both = node(
        "tuple",
        &format!(
            r#""members":[{},{}]"#,
            call(node("int", r#""value":1"#, INT), INT),
            call(node("string", r#""value":"a""#, STRING), STRING)
        ),
        &format!(r#"{{"tuple":[{},{}]}}"#, at(INT), at(STRING)),
    );
    let document = format!(
        r#"{{"transport":28,"declarations":[],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{held}],"values":[{{"module":"m","name":"v","handovers":[],"body":{both}}}],"entries":[],"definitions":[],"examples":[]}}]}}"#
    );
    let object = object_for(&document).unwrap_or_else(|refused| panic!("refused: {refused}"));
    let has = |symbol: &str| {
        object
            .windows(symbol.len())
            .any(|window| window == symbol.as_bytes())
    };
    assert!(
        has("$closure$0") && has("$closure$1"),
        "one lifted function for each copy"
    );
}

/// `h<'a, 'b>` calling `h<'b, 'a>` is two functions calling one another, which is what the two
/// copies are once each is made for the types a call reaches it at.
#[test]
fn a_helper_calling_itself_with_its_types_swapped_is_lowered_as_two_functions() {
    let (a, b) = (r#"{"var":0}"#, r#"{"var":1}"#);
    let swapped = node(
        "call",
        &format!(
            r#""reaches":{{"is":"helper","reached":{{"is":"own","module":"m","name":"h"}}}},"arguments":[{},{}]"#,
            read(1, b),
            read(0, a)
        ),
        INT,
    );
    let held = format!(
        r#"{{"reached":{{"is":"own","module":"m","name":"h"}},"parameters":[{{"name":"x","type":{a}}},{{"name":"y","type":{b}}}],"body":{swapped}}}"#
    );
    let called = node(
        "call",
        &format!(
            r#""reaches":{{"is":"helper","reached":{{"is":"own","module":"m","name":"h"}}}},"arguments":[{},{}]"#,
            node("int", r#""value":1"#, INT),
            node("string", r#""value":"a""#, STRING)
        ),
        INT,
    );
    let document = format!(
        r#"{{"transport":28,"declarations":[],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{held}],"values":[{{"module":"m","name":"v","handovers":[],"body":{called}}}],"entries":[],"definitions":[],"examples":[]}}]}}"#
    );
    if let Err(refused) = object_for(&document) {
        panic!("refused: {refused}");
    }
}
