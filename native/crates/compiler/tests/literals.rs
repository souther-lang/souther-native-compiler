//! A string literal is one piece of data in the object, however many places spell it.

use souther_native_driver::object_for;

/// `m.twice` joins a literal to itself, and its boundary writes a key it shares with nothing: the
/// literal's bytes are in the object once, not once per site that spells them.
#[test]
fn a_literal_spelt_twice_is_held_once() {
    let text = r#"{"core":"string","value":"zebra-crossing","type":{"prim":"STRING"},"aborts":[]}"#;
    let document = format!(
        concat!(
            r#"{{"transport":19,"declarations":[],"#,
            r#""behaviors":[{{"module":"m","name":"twice","is":"body","parameters":{{"named":[]}},"#,
            r#""output":{{"is":"scalar","scalar":"STRING"}},"ensures":{{"at":"none"}}}}],"#,
            r#""modules":[{{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":["#,
            r#"{{"is":"body","declared":"m.twice","parameters":[],"publication":"published","requirements":[],"#,
            r#""body":{{"core":"binary","op":"CONCAT","reading":{{"is":"astheystand"}},"left":{text},"right":{text},"#,
            r#""type":{{"prim":"STRING"}},"aborts":[]}}}}"#,
            r#"],"examples":[]}}]}}"#
        ),
        text = text
    );

    let object = object_for(&document).expect("an object");
    let held = object
        .windows(b"zebra-crossing".len())
        .filter(|at| *at == b"zebra-crossing")
        .count();

    assert_eq!(held, 1, "the literal's bytes, counted in the object");
}
