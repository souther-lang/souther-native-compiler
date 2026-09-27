//! What the carrier holds is spent as text is built, so the boundary is where the arithmetic says
//! it is for every operation that builds text: a capacity one unit short of what an operation needs
//! ends it, and one that is enough answers what an unbounded one does.
//!
//! The oracle is written here from the operation's definition and shares nothing with what it
//! checks: what has to be held is the text as it is handed over, before NFC (spec
//! §what-a-string-holds: "the text it is defined as canonicalizing"), and what it comes to after.

use souther_text::{
    Capacity, Text, append, join, lowercase, pad_left, pad_right, repeat, replace, reverse,
    uppercase,
};

const PLENTY: Capacity = Capacity::of_code_points(1 << 30);

fn code_points(text: &str) -> i64 {
    text.chars().count() as i64
}

fn held(text: &str) -> Text<'_> {
    Text::held(text)
}

/// Text that composes at a seam, is outside the basic plane, and is plain.
const CORPUS: [&str; 8] = [
    "",
    "a",
    "e",
    "\u{301}",
    "\u{e9}\u{301}",
    "\u{10000}\u{10428}",
    "straße",
    "\u{1ea1}b",
];

/// Every capacity, from none to past what is needed, answers as the oracle says: the operation
/// holds where the capacity is at least what it hands over and at least what it comes to.
fn every_capacity_answers_as_the_oracle_says(
    what: &str,
    handed_over: i64,
    build: impl Fn(Capacity) -> Option<String>,
) {
    let whole = build(PLENTY).expect("a capacity of a billion units holds every text here");
    let needs = handed_over.max(code_points(&whole));
    for capacity in 0..=needs + 2 {
        let built = build(Capacity::of_code_points(capacity));
        if capacity >= needs {
            assert_eq!(
                built.as_deref(),
                Some(whole.as_str()),
                "{what} at {capacity}"
            );
        } else {
            assert_eq!(built, None, "{what} at {capacity} of {needs}");
        }
    }
}

#[test]
fn a_join_holds_what_is_handed_over_and_what_it_comes_to() {
    for one in CORPUS {
        for other in CORPUS {
            every_capacity_answers_as_the_oracle_says(
                &format!("append {one:?} {other:?}"),
                code_points(one) + code_points(other),
                |capacity| append(held(one), held(other), capacity),
            );
            for third in CORPUS {
                let pieces = [held(one), held(other), held(third)];
                every_capacity_answers_as_the_oracle_says(
                    &format!("join {one:?} {other:?} {third:?}"),
                    code_points(one) + code_points(other) + code_points(third) + 2 * code_points("-"),
                    |capacity| join(held("-"), pieces, capacity),
                );
            }
        }
    }
}

#[test]
fn a_replace_holds_what_it_writes() {
    for text in CORPUS {
        for replacement in CORPUS {
            let target = "a";
            let pieces = text.split(target).count() as i64;
            every_capacity_answers_as_the_oracle_says(
                &format!("replace {text:?} {replacement:?}"),
                code_points(text) - (pieces - 1) * code_points(target) + (pieces - 1) * code_points(replacement),
                |capacity| replace(held(target), held(replacement), held(text), capacity),
            );
        }
    }
}

#[test]
fn a_reverse_and_a_case_mapping_hold_what_they_come_to() {
    for text in CORPUS {
        every_capacity_answers_as_the_oracle_says(
            &format!("reverse {text:?}"),
            code_points(text),
            |capacity| reverse(held(text), capacity),
        );
        every_capacity_answers_as_the_oracle_says(
            &format!("lowercase {text:?}"),
            code_points(text),
            |capacity| lowercase(held(text), capacity),
        );
    }
    // Uppercasing writes more than it was handed: `ß` is `SS`.
    every_capacity_answers_as_the_oracle_says("uppercase", code_points("STRASSE"), |capacity| {
        uppercase(held("straße"), capacity)
    });
}

#[test]
fn a_repeat_holds_the_copies() {
    for text in CORPUS {
        for copies in 0..5 {
            every_capacity_answers_as_the_oracle_says(
                &format!("repeat {copies} {text:?}"),
                code_points(text) * copies,
                |capacity| repeat(copies, held(text), capacity),
            );
        }
    }
}

/// The fill is measured with the text it widens, in units and not in the code points a width
/// counts, and the copies of the pad that cover the fill are measured too, before the fill is cut.
#[test]
fn a_pad_holds_the_fill_and_the_text_together() {
    for text in CORPUS {
        for pad in ["a", "xy", "\u{10000}"] {
            for width in 0..7 {
                for (name, build) in [
                    (
                        "left",
                        pad_left as fn(i64, Text, Text, Capacity) -> Option<String>,
                    ),
                    ("right", pad_right),
                ] {
                    let whole = build(width, held(pad), held(text), PLENTY).unwrap();
                    let needs = code_points(&whole);
                    for capacity in 0..=needs + code_points(pad) * 2 + 2 {
                        let built =
                            build(width, held(pad), held(text), Capacity::of_code_points(capacity));
                        if capacity >= needs + code_points(pad) * 2 {
                            assert_eq!(built.as_deref(), Some(whole.as_str()));
                        }
                        // Never a text no capacity holds that the operation made: what it hands back
                        // as it was given is text already held, and is not made.
                        let widens = width > text.chars().count() as i64;
                        if let (true, Some(built)) = (widens, built) {
                            assert!(
                                code_points(&built) <= capacity,
                                "pad {name} {width} {pad:?} {text:?} at {capacity}"
                            );
                        }
                    }
                    // One unit short of what the answer is written in is never an answer.
                    if needs > 0 && width > text.chars().count() as i64 {
                        assert_eq!(
                            build(width, held(pad), held(text), Capacity::of_code_points(needs - 1)),
                            None,
                            "pad {name} {width} {pad:?} {text:?}"
                        );
                    }
                }
            }
        }
    }
}

/// A function that builds text says what the carrier holds. Text semantics is the same for every
/// target and a bound is one target's, so an operation that builds a string and takes no
/// `Capacity` is one that can build what no carrier holds. The two that do take none build from
/// what is already held: a number's digits, and text put in NFC where it comes in.
#[test]
fn every_function_that_builds_text_takes_a_capacity() {
    let allowed = ["written"];
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut refused = Vec::new();
    let mut seen = 0;
    for entry in std::fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        // Only what is public, and only what comes before the tests.
        let text = text.split("#[cfg(test)]").next().unwrap();
        let mut rest = text;
        while let Some(at) = rest.find("pub fn ") {
            let from = &rest[at + "pub fn ".len()..];
            let name: String = from
                .chars()
                .take_while(|it| it.is_alphanumeric() || *it == '_')
                .collect();
            let signature = &from[..from.find('{').unwrap()];
            rest = from;
            let builds = signature
                .split("->")
                .nth(1)
                .is_some_and(|returned| returned.contains("String") || returned.contains("Cow"));
            if !builds {
                continue;
            }
            seen += 1;
            if !signature.contains("Capacity") && !allowed.contains(&name.as_str()) {
                refused.push(format!("{}: {name}", path.display()));
            }
        }
    }
    assert!(
        seen >= 10,
        "the scan found only {seen} builders: it reads nothing"
    );
    assert!(
        refused.is_empty(),
        "builds text and holds no capacity: {refused:?}"
    );
}

/// What is worked out from a width stays within what an `i64` counts: a width no capacity holds is
/// refused before the copies of the pad that would cover it are worked out from it, and the sum
/// that counts them would leave the range for the widest.
#[test]
fn a_width_at_the_end_of_the_range_is_refused_and_not_worked_out() {
    for width in [i64::MAX, i64::MAX - 1, i64::MAX - 2] {
        for pad in ["a", "xy", "\u{10000}"] {
            assert_eq!(pad_left(width, held(pad), held("b"), PLENTY), None);
            assert_eq!(pad_right(width, held(pad), held("b"), PLENTY), None);
        }
    }
}
