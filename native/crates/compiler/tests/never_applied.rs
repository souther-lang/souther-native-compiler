//! A function a helper is handed and never applies, run.
//!
//! `kept(a)` calls `h(a, f)` where `f` is a function over what has no value and `h` answers its
//! first argument without applying its second. Nothing of `f` is lowered, and `h`, which holds a
//! pointer where it takes one, is handed a function no call reaches: one with no code, carrying
//! nothing.

use souther_native_driver::object_for;
use souther_native_driver::transport::TRANSPORT_VERSION;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

mod support;

const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern uint32_t kept(const void *, int64_t, int64_t *) __asm__("PREFIXsouther@.unran.kept");

int main(int argc, char **argv) {
    int64_t out = 0;
    uint32_t status = kept(NULL, strtoll(argv[1], NULL, 10), &out);
    printf("%u\n%" PRId64 "\n", status, out);
    return 0;
}
"#;

fn document() -> String {
    let int = r#"{"prim":"INT"}"#;
    let nothing_to_int = r#"{"fn":{"takes":[{"nothing":{}}],"answers":{"prim":"INT"}}}"#;
    let read = |binding: usize| {
        format!(r#"{{"core":"read","binding":{binding},"type":{int},"aborts":[]}}"#)
    };
    let block = format!(
        r#"{{"core":"block","site":0,"parameters":[{{"binding":1,"name":"x"}}],"body":{},"type":{nothing_to_int},"aborts":[]}}"#,
        read(0)
    );
    let held = format!(
        r#"{{"reached":{{"is":"own","module":"unran","name":"h"}},"parameters":[{{"name":"n","type":{int}}},{{"name":"f","type":{nothing_to_int}}}],"body":{}}}"#,
        read(0)
    );
    let called = format!(
        r#"{{"core":"call","reaches":{{"is":"helper","reached":{{"is":"own","module":"unran","name":"h"}}}},"arguments":[{},{block}],"type":{int},"aborts":[]}}"#,
        read(0)
    );
    format!(
        r#"{{"transport":{TRANSPORT_VERSION},"declarations":[],"behaviors":[{{"module":"unran","name":"kept","is":"body","parameters":{{"named":[{{"name":"a","input":{{"is":"scalar","scalar":"INT"}}}}]}},"output":{{"is":"scalar","scalar":"INT"}},"ensures":{{"at":"none"}},"requirements":[]}}],"modules":[{{"name":"unran","publishes":[],"helpers":[{held}],"values":[],"entries":[],"definitions":[{{"is":"body","declared":"unran.kept","parameters":["a"],"publication":"published","body":{called}}}],"examples":[]}}]}}"#
    )
}

#[test]
fn a_function_a_helper_never_applies_is_handed_a_function_no_call_reaches() {
    let into = tempdir().expect("a directory to work in");
    let object = into.path().join("never_applied.o");
    fs::write(
        &object,
        object_for(&document()).expect("an object: nothing of the function is lowered"),
    )
    .expect("the object written");
    let harness = into.path().join("harness.c");
    fs::write(&harness, support::harness(HARNESS)).expect("the harness written");
    let executable = into.path().join("never_applied");
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
    let output = Command::new(&executable)
        .arg("41")
        .output()
        .expect("the executable to run");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n41\n");
}

/// The document the Java half wrote for `List.fold((acc, x) -> acc, a, [])`, and the one its own
/// test holds it to: the fold is a helper over two variables, called at `Int` and what has no
/// value, so its copy reads an optional of what has no value, and never takes the arm for one
/// being there.
const FOLDING_EMPTY: &str = include_str!("folding_empty.transport.json");

const FOLDING_HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern uint32_t kept(const void *, int64_t, int64_t *) __asm__("PREFIXsouther@.folding.kept");

int main(int argc, char **argv) {
    int64_t out = 0;
    uint32_t status = kept(NULL, strtoll(argv[1], NULL, 10), &out);
    printf("%u\n%" PRId64 "\n", status, out);
    return 0;
}
"#;

#[test]
fn a_fold_of_an_empty_list_answers_its_seed() {
    let into = tempdir().expect("a directory to work in");
    let object = into.path().join("folding.o");
    fs::write(
        &object,
        object_for(FOLDING_EMPTY).expect("an object: the arm for a present element is not lowered"),
    )
    .expect("the object written");
    let harness = into.path().join("harness.c");
    fs::write(&harness, support::harness(FOLDING_HARNESS)).expect("the harness written");
    let executable = into.path().join("folding");
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
    let output = Command::new(&executable)
        .arg("41")
        .output()
        .expect("the executable to run");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n41\n");
}
