//! A function a kernel is handed and never runs, run: what is worked out to make it is worked out.
//!
//! The fixture is `AFunctionAKernelNeverRunsIsStillHandedOverTest`'s. `sorted(a)` sorts an empty
//! list by a key over what has no value and `mapped(a)` maps an absent value, each by a function
//! that reads `a`. Such a function never runs, and upstream hands every kernel its function all
//! the same (`Core.Call.functionArgument` answers `HANDED_OVER` for a kernel): the JVM works out
//! what the function is made of, a `let` around its block included, before it makes the lambda.
//! So a document whose function is a `let` binding what ends the run around the block ends the
//! run, here as there, and the empty answer is not answered. `byFold`'s key is a block whose body
//! hands a fold a step it never applies, which is not lowered wherever it runs; the body never
//! runs, so nothing in it is asked to be.

use serde_json::{Value, json};
use souther_native_driver::object_for;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{TempDir, tempdir};

mod support;

/// The document the Java half wrote, and the one its own test holds it to.
const UNRAN: &str = include_str!("unran.transport.json");

/// Calls the behavior named on the command line with the number after it, and says the status and
/// what it answered.
const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern uint32_t sorted(const void *, int64_t, int64_t *) __asm__("PREFIXsouther@.unran.sorted");
extern uint32_t mapped(const void *, int64_t, int64_t *) __asm__("PREFIXsouther@.unran.mapped");
extern uint32_t by_fold(const void *, int64_t, int64_t *) __asm__("PREFIXsouther@.unran.byFold");

int main(int argc, char **argv) {
    if (argc != 3) {
        return 2;
    }
    int64_t a = strtoll(argv[2], NULL, 10);
    int64_t out = 0;
    uint32_t status = strcmp(argv[1], "sorted") == 0   ? sorted(NULL, a, &out)
                      : strcmp(argv[1], "mapped") == 0 ? mapped(NULL, a, &out)
                                                       : by_fold(NULL, a, &out);
    printf("%u\n%" PRId64 "\n", status, out);
    return 0;
}
"#;

/// The status a run that leaves an `Int`'s range ends with (`abort-status-abi4.json`).
const REQUIRED_FORM_HAS_NO_PLACE: u32 = 5;

fn build(document: &str) -> (TempDir, PathBuf) {
    let into = tempdir().expect("a directory to work in");
    let object = into.path().join("unran.o");
    fs::write(&object, object_for(document).expect("an object")).expect("the object written");
    let harness = into.path().join("harness.c");
    fs::write(&harness, support::harness(HARNESS)).expect("the harness written");
    let executable = into.path().join("unran");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness)
        .arg(&object)
        .args(support::runtime_arguments())
        .output()
        .expect("a C compiler to link with");
    assert!(
        linked.status.success(),
        "the link failed: {}",
        String::from_utf8_lossy(&linked.stderr)
    );
    (into, executable)
}

/// The status and the answer of one run.
fn run(executable: &Path, behavior: &str, a: i64) -> (u32, i64) {
    let output = Command::new(executable)
        .args([behavior, &a.to_string()])
        .output()
        .expect("the executable to run");
    assert!(
        output.status.success(),
        "the process itself failed: {output:?}"
    );
    let said = String::from_utf8_lossy(&output.stdout);
    let mut lines = said.lines();
    let status = lines.next().expect("a status").parse().expect("a number");
    let answered = lines.next().expect("an answer").parse().expect("a number");
    (status, answered)
}

/// The fixture with the function each kernel is handed made a `let` around its block, binding
/// the sum of the largest `Int` and one, which no `Int` holds.
fn computed_around() -> String {
    /// Whether a `let` was put somewhere under `value`.
    fn wrap(value: &mut Value) -> bool {
        let mut put = false;
        if let Value::Object(fields) = value
            && fields.get("core") == Some(&json!("call"))
            && fields["reaches"]["is"] == "kernel"
            && ["list.sortBy", "option.map"]
                .contains(&fields["reaches"]["kernel"].as_str().unwrap())
        {
            let int = json!({ "prim": "INT" });
            let block = fields["arguments"][0].take();
            let ty = block["type"].clone();
            let past = json!({
                "core": "call",
                "reaches": { "is": "kernel", "kernel": "int.add", "takes": [int, int],
                             "fact": { "is": "none" } },
                "arguments": [
                    { "core": "int", "value": i64::MAX, "type": int, "aborts": [] },
                    { "core": "int", "value": 1, "type": int, "aborts": [] }
                ],
                "type": int, "aborts": ["REQUIRED_FORM_HAS_NO_PLACE"]
            });
            fields["arguments"][0] = json!({
                "core": "let", "binding": 900, "binds": int, "value": past, "body": block,
                "type": ty, "aborts": []
            });
            put = true;
        }
        match value {
            Value::Object(fields) => fields.values_mut().for_each(|it| put |= wrap(it)),
            Value::Array(items) => items.iter_mut().for_each(|it| put |= wrap(it)),
            _ => {}
        }
        put
    }
    let mut document: Value = serde_json::from_str(UNRAN).expect("the fixture is JSON");
    assert!(wrap(&mut document), "the fixture calls both kernels");
    document.to_string()
}

#[test]
fn a_function_a_kernel_never_runs_answers_what_the_kernel_answers_for_nothing() {
    let (_swept, built) = build(UNRAN);
    assert_eq!(run(&built, "sorted", 5), (0, 5));
    assert_eq!(run(&built, "mapped", 5), (0, 5));
    assert_eq!(run(&built, "byFold", 5), (0, 5));
}

#[test]
fn what_a_function_a_kernel_never_runs_is_made_of_is_worked_out() {
    let (_swept, built) = build(&computed_around());
    assert_eq!(run(&built, "sorted", 5).0, REQUIRED_FORM_HAS_NO_PLACE);
    assert_eq!(run(&built, "mapped", 5).0, REQUIRED_FORM_HAS_NO_PLACE);
}
