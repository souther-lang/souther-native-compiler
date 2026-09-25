//! A row stands in for what its behavior requires with what it states, and its entry runs it with
//! nothing handed: the stand-ins are in the object, as functions answering the way upstream's
//! `StandsIn.answering` does, and the row's entry calls the behavior with a capability of each.
//!
//! The first entry stating the arguments a call arrived with answers; where none does, what the row
//! states for the rest; and where it states nothing for the rest, `FAKE_NO_OUTPUT`, which is not a
//! language abort and not a host's status (upstream files it as the row's fake resolution failing).
//! The checker refuses a program whose rows ask a stand-in for what it does not state, so the last
//! is reached here from a document written by hand.

use serde_json::{Value, json};
use souther_native_driver::object_for;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

mod support;
use support::PREFIX;

/// `m.twice`, which requires `m.lookUp` and answers what it answers doubled.
const ENSURES: &str = include_str!("ensures.transport.json");

fn int(value: i64) -> Value {
    json!({"core": "int", "value": value, "type": {"prim": "INT"}, "aborts": []})
}

/// A row of `m.twice` handed `a`, standing in for `m.lookUp` with `(1) -> 21` and `otherwise`.
fn row(at: usize, a: i64, otherwise: Option<i64>) -> Value {
    json!({
        "behavior": "twice",
        "at": at,
        "body": {
            "core": "call",
            "reaches": {"is": "behavior", "declared": "m.twice"},
            "arguments": [int(a)],
            "type": {"prim": "INT"},
            "aborts": []
        },
        "standsIn": [{
            "module": "m",
            "name": "lookUp",
            "entries": [{"arguments": [int(1)], "answer": int(21)}],
            "otherwise": otherwise.map(int)
        }]
    })
}

const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>

extern uint32_t stated(int64_t *) __asm__("PREFIXsouther4.m.twice$example$0");
extern uint32_t rest(int64_t *) __asm__("PREFIXsouther4.m.twice$example$1");
extern uint32_t unstated(int64_t *) __asm__("PREFIXsouther4.m.twice$example$2");
extern int64_t souther_mark(void);
extern void souther_reset(int64_t);

static void ran(const char *what, uint32_t (*row)(int64_t *)) {
    int64_t mark = souther_mark();
    int64_t answer = -1;
    uint32_t status = row(&answer);
    printf("%s %u %" PRId64 "\n", what, status, answer);
    souther_reset(mark);
}

int main(void) {
    ran("stated", stated);
    ran("rest", rest);
    ran("unstated", unstated);
    return 0;
}
"#;

#[test]
fn a_row_stands_in_with_the_first_entry_stating_the_call_and_otherwise_with_the_rest() {
    let mut document: Value = serde_json::from_str(ENSURES).unwrap();
    document["modules"][0]["examples"] =
        json!([row(0, 1, None), row(1, 2, Some(5)), row(2, 2, None)]);

    let into = tempdir().unwrap();
    let object = into.path().join("m.o");
    fs::write(&object, object_for(&document.to_string()).unwrap()).unwrap();
    let harness = into.path().join("harness.c");
    fs::write(&harness, HARNESS.replace("PREFIX", PREFIX)).unwrap();
    let executable = into.path().join("standing");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness)
        .arg(&object)
        .arg(support::runtime())
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let run = Command::new(&executable).output().unwrap();

    assert_eq!(
        String::from_utf8(run.stdout).unwrap(),
        format!(
            "stated 0 42\nrest 0 10\nunstated {} -1\n",
            souther_native_abi::FAKE_NO_OUTPUT
        )
    );
}

/// A row standing in for other than what its behavior requires, in that order, is the two halves
/// disagreeing: its entry would call the behavior with a capability of one dependency in the place
/// of another.
#[test]
fn a_row_standing_in_for_other_than_what_its_behavior_requires_is_refused() {
    let mut document: Value = serde_json::from_str(ENSURES).unwrap();
    let mut standing = row(0, 1, None);
    standing["standsIn"][0]["name"] = json!("find");
    document["modules"][0]["examples"] = json!([standing]);

    let refused = object_for(&document.to_string()).expect_err("a stand-in out of place");

    assert!(
        refused
            .to_string()
            .contains("in the order the behavior requires them"),
        "{refused}"
    );
}

const UNANSWERED: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>

extern uint32_t looked(int64_t *) __asm__("PREFIXsouther4.m.lookUp$example$0");

int main(void) {
    int64_t answer = -1;
    uint32_t status = looked(&answer);
    printf("%u %" PRId64 "\n", status, answer);
    return 0;
}
"#;

/// A row of a behavior a host implements states what the host's implementation answers, and
/// nothing in the object answers it: the behavior has no symbol, and the row stands in with no
/// capability of the behavior it is a row of. So its entry answers `INJECTION_UNBOUND`, as a call
/// handed nothing for it does, rather than calling something no object defines.
#[test]
fn a_row_of_what_a_host_implements_answers_unbound() {
    let mut document: Value = serde_json::from_str(ENSURES).unwrap();
    document["modules"][0]["examples"] = json!([{
        "behavior": "lookUp",
        "at": 0,
        "body": {
            "core": "call",
            "reaches": {"is": "behavior", "declared": "m.lookUp"},
            "arguments": [int(1)],
            "type": {"prim": "INT"},
            "aborts": []
        },
        "standsIn": []
    }]);

    let into = tempdir().unwrap();
    let object = into.path().join("m.o");
    fs::write(&object, object_for(&document.to_string()).unwrap()).unwrap();
    let harness = into.path().join("harness.c");
    fs::write(&harness, UNANSWERED.replace("PREFIX", PREFIX)).unwrap();
    let executable = into.path().join("unanswered");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness)
        .arg(&object)
        .arg(support::runtime())
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let run = Command::new(&executable).output().unwrap();

    assert_eq!(
        String::from_utf8(run.stdout).unwrap(),
        format!("{} -1\n", souther_native_abi::INJECTION_UNBOUND)
    );
}
