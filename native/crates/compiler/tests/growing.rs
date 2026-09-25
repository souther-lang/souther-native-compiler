//! A walk that grows a list, run: what it answers, and how much room it takes to answer it.
//!
//! The fixture is `AWalkGrowingAListCrossesAsTheOperationsItIsTest`'s. `grown(n)` walks a list of
//! ten thousand numbers, keeps those below `n`, and adds one to each, as one walk the checker's
//! compiler joined out of a `filter` and a `map`, growing a list of `n` elements.
//!
//! Growing that list the way `++` joins two lists, a new list of everything so far at every step,
//! takes room for `n²/2` slots. Growing it in place takes room linear in `n`. Which of the two the
//! lowering does is not something a clock should be asked: the room a run takes from the arena is
//! counted by the arena itself, in slots, and is the same on every machine. So the run is made for
//! two values of `n`, the difference is what the longer walk took over the shorter, and that is held
//! to a bound linear in how many more elements it grew.

use serde_json::{Value, json};
use souther_native_driver::object_for;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{TempDir, tempdir};

mod support;
use support::PREFIX;

/// The document the Java half wrote, and the one its own test holds it to.
const GROWING: &str = include_str!("growing.transport.json");

/// Calls `grown` once with the number it is given, and says what it answered and how many slots
/// the arena handed out while it ran.
const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern uint32_t grown(const void *, int64_t, int64_t *) __asm__("PREFIXsouther4.growing.grown");
extern int64_t souther_mark(void);
extern void souther_reset(int64_t);

int main(int argc, char **argv) {
    if (argc != 2) {
        return 2;
    }
    int64_t n = strtoll(argv[1], NULL, 10);
    int64_t out;
    int64_t before = souther_mark();
    uint32_t status = grown(NULL, n, &out);
    int64_t after = souther_mark();
    souther_reset(before);
    printf("%u\n%" PRId64 "\n%" PRId64 "\n", status, out, after - before);
    return 0;
}
"#;

/// What one run said: the status, what `grown` answered, and the slots it took.
struct Ran {
    status: u32,
    answered: i64,
    taken: i64,
}

fn build() -> (TempDir, PathBuf) {
    let into = tempdir().expect("a directory to work in");
    let object = into.path().join("growing.o");
    fs::write(&object, object_for(GROWING).expect("an object")).expect("the object written");
    let harness = into.path().join("harness.c");
    fs::write(&harness, HARNESS.replace("PREFIX", PREFIX)).expect("the harness written");
    let executable = into.path().join("growing");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness)
        .arg(&object)
        .arg(support::runtime())
        .output()
        .expect("a C compiler to link with");
    assert!(
        linked.status.success(),
        "the link failed: {}",
        String::from_utf8_lossy(&linked.stderr)
    );
    (into, executable)
}

fn run(executable: &Path, n: i64) -> Ran {
    let output = Command::new(executable)
        .arg(n.to_string())
        .output()
        .expect("the executable to run");
    assert!(
        output.status.success(),
        "the process itself failed: {output:?}"
    );
    let said = String::from_utf8_lossy(&output.stdout);
    let mut lines = said.lines().map(|it| it.parse::<i64>().expect("a number"));
    let mut next = || lines.next().expect("three lines");
    Ran {
        status: u32::try_from(next()).expect("a status"),
        answered: next(),
        taken: next(),
    }
}

#[test]
fn a_walk_answers_the_list_it_grew() {
    let (_swept, built) = build();
    for n in [0, 1, 9, 10, 17, 10_000, 20_000] {
        let ran = run(&built, n);
        assert_eq!(ran.status, 0, "ANSWERED for {n}");
        assert_eq!(ran.answered, n.min(10_000), "the elements below {n}");
    }
}

/// Every element grown is copied a bounded number of times, however long the walk. Doubling the
/// room each time it runs out takes at most about four slots an element over the walk's length,
/// the old rooms included; a bound of eight is well clear of that and some three hundred times
/// below what a new list at every step would take for this many elements.
#[test]
fn growing_a_list_takes_room_linear_in_what_it_grows() {
    let (_swept, built) = build();
    let (short, long) = (10, 10_000);
    let more = run(&built, long).taken - run(&built, short).taken;
    assert!(
        more <= 8 * (long - short),
        "growing {} more elements took {more} more slots",
        long - short
    );
}

/// The fixture with the first node `found` answers for handed to `change`, walked in the order the
/// document is written.
fn changed(found: impl Fn(&Value) -> bool, change: impl FnOnce(&mut Value)) -> String {
    fn first<'v>(value: &'v mut Value, found: &dyn Fn(&Value) -> bool) -> Option<&'v mut Value> {
        if found(value) {
            return Some(value);
        }
        match value {
            Value::Object(fields) => fields.values_mut().find_map(|it| first(it, found)),
            Value::Array(items) => items.iter_mut().find_map(|it| first(it, found)),
            _ => None,
        }
    }
    let mut document: Value = serde_json::from_str(GROWING).expect("the fixture is JSON");
    change(first(&mut document, &found).expect("the fixture holds what is changed"));
    document.to_string()
}

fn emitted(operation: &str) -> impl Fn(&Value) -> bool {
    move |node| node["core"] == "call" && node["reaches"]["operation"] == operation
}

fn refused(document: &str) -> String {
    object_for(document)
        .expect_err("a document where the list a walk grows could be read as a list")
        .to_string()
}

/// `acc ++ acc`: what is added is the list being grown, read as a list.
#[test]
fn a_step_reading_what_it_grows_as_a_list_is_refused() {
    let document = changed(emitted("GROW_LIST"), |grow| {
        grow["arguments"][1] = grow["arguments"][0].clone();
    });
    let refused = refused(&document);
    assert!(refused.contains("is read as a list"), "{refused}");
    assert!(refused.contains("disagree"), "{refused}");
}

/// A growth where no step answers: here, the list a walk walks.
#[test]
fn a_growth_outside_a_step_is_refused() {
    let document = changed(emitted("BUILD_LIST"), |build| {
        let walked = build["arguments"][1].clone();
        build["arguments"][1] = json!({
            "core": "call",
            "reaches": { "is": "emitted", "operation": "GROW_LIST" },
            "arguments": [walked.clone(), walked],
            "type": { "list": { "prim": "INT" } },
            "aborts": []
        });
    });
    let refused = refused(&document);
    assert!(refused.contains("where no step answers"), "{refused}");
}

/// A step answering a list of its own making, which the walk would go on growing as though it
/// were the one it grows.
#[test]
fn a_step_answering_another_list_is_refused() {
    let document = changed(emitted("BUILD_LIST"), |build| {
        build["arguments"][0]["body"] = json!({
            "core": "list",
            "elements": [],
            "type": { "list": { "prim": "INT" } },
            "aborts": []
        });
    });
    let refused = refused(&document);
    assert!(
        refused.contains("other than the list it grows"),
        "{refused}"
    );
}

/// A walk whose step is a function value it was handed rather than a block written there.
#[test]
fn a_walk_whose_step_is_not_a_block_is_refused() {
    let document = changed(emitted("BUILD_LIST"), |build| {
        let ty = build["arguments"][0]["type"].clone();
        let step = build["arguments"][0].take();
        build["arguments"][0] = json!({
            "core": "let", "binding": 900, "binds": ty.clone(), "value": step,
            "body": { "core": "read", "binding": 900, "type": ty.clone(), "aborts": [] },
            "type": ty, "aborts": []
        });
    });
    let refused = refused(&document);
    assert!(refused.contains("not a block"), "{refused}");
}
