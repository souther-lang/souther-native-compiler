//! What "no value of this type is ever made" means is stated once, on the type.
//!
//! `Nothing` and `Never` are both types no value of which is made, and a pass that asks whether a
//! value could be made of a type has to give the same answer for both. A pass that names one of
//! them where it means that answers for it and forgets the other, and a new pass does the same
//! again unless the naming is not available to it: `List<Never>` was compared through an element
//! comparator, and an arm for an element of `Option<Never>` was lowered, while `List<Nothing>` and
//! `Option<Nothing>` were not. `Ty::has_no_value` and `Ty::holds_no_value` are the statement, and
//! `FnSignature::never_runs` the one place upstream's own, narrower question (`Core.neverRuns`) is
//! asked. All three are in `transport.rs`, so nothing else names either type in a `matches!`.

use std::fs;
use std::path::Path;

/// Every `matches!(...)` in `text` that names `Ty::Nothing` or `Ty::Never`, by the text of it.
fn naming_a_bottom(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("matches!(") {
        let start = from + at;
        let mut depth = 0usize;
        let mut end = text.len();
        for (place, byte) in text[start..].bytes().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + place + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let expression = &text[start..end];
        if expression.contains("Ty::Nothing") || expression.contains("Ty::Never") {
            found.push(expression.to_string());
        }
        from = end;
    }
    found
}

/// The scan finds what it is for, in one line and across several, and nothing else.
#[test]
fn the_scan_finds_a_bottom_named_in_a_matches() {
    let one = "if matches!(ty, Ty::Nothing { .. }) {}";
    let several =
        "matches!(\n    ty,\n    Ty::Option { option } if matches!(**option, Ty::Never { .. })\n)";
    let other = "matches!(ty, Ty::Fn { .. }) && Ty::Nothing { nothing: Bottom {} }.is_open()";
    assert_eq!(naming_a_bottom(one).len(), 1);
    assert_eq!(naming_a_bottom(several).len(), 1);
    assert!(naming_a_bottom(other).is_empty());
}

#[test]
fn only_the_type_names_what_has_no_value() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut named = Vec::new();
    for entry in fs::read_dir(&source).expect("the source directory") {
        let path = entry.expect("an entry").path();
        if path.extension().is_none_or(|it| it != "rs")
            || path.file_name().is_some_and(|it| it == "transport.rs")
        {
            continue;
        }
        let text = fs::read_to_string(&path).expect("a source file");
        for expression in naming_a_bottom(&text) {
            named.push(format!("{}: {expression}", path.display()));
        }
    }
    assert!(
        named.is_empty(),
        "ask `Ty::has_no_value`, `Ty::holds_no_value` or `FnSignature::never_runs` instead of \
         naming the type: {named:#?}"
    );
}
