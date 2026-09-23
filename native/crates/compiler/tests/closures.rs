//! The whole path, at the width of a function value: a checked program the Java half wrote out
//! for Issue #11 (souther-lang/souther-native-compiler#11), an object this driver lowered it to,
//! the system linker, and a run that answers.
//!
//! Every behavior in the fixture is built from the one source shape that survives to a
//! `Core.Block`/`Core.Apply` pair at all — an `if` choosing between two lambdas, so which one runs
//! is not known until the run decides (see the Java test that owns this fixture,
//! `AFunctionValueCrossesWholeWithNoCaptureListTest`, for why a lambda bound and applied straight
//! away never reaches this far). What each behavior below exercises:
//!
//! - `no_capture`: a closure over nothing at all, applied.
//! - `with_struct`: a closure over one value of a declared type, applied — the field is read
//!   through the closure's own captured pointer, not through anything the caller passed again.
//! - `aborting`: a closure whose body leaves `Int`'s range, applied — the abort has to cross the
//!   indirect call the same way it crosses a direct one.
//! - `adder`: the two branches close over one `Int` each, laid out identically because both
//!   branches are one parameter list — but which of the two is meant is still a runtime question,
//!   which is what keeps this from being inlined away like the straight-line case is.
//! - `nested`: a closure inside a closure. The outer's own parameter and a `let` inside its body
//!   are both free in the inner block and nowhere else; the outer's own capture list carries the
//!   outer parameter `a` forward for the inner block's sake alone, since nothing in the outer's own
//!   body reads it except by handing it to the inner closure.
//! - `handed_over`: every behavior above builds and applies its own closure in the one generated
//!   function its own body lowers to. This is the one shape that hands a closure to a *different*
//!   generated function and applies it there: `apply_n` is a recursive helper — the one shape this
//!   language leaves un-inlined as a method of its own (a function type cannot cross a behavior's
//!   own declared boundary at all, E1311, so a helper's own parameter is how this is reached in
//!   practice) — taking `f` as an ordinary machine parameter and calling `f(x)` inside a body that
//!   never built the closure, then recursing with the same `f` handed on again.

use souther_native_driver::object_for;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

mod support;

/// The document the Java half wrote, and the one its own test holds it to.
const CLOSURES: &str = include_str!("closures.transport.json");

/// What the linker on this platform calls a symbol the object names.
const PREFIX: &str = if cfg!(target_vendor = "apple") { "_" } else { "" };

/// What generated code takes room from — needed here because every closure this fixture builds is
/// allocated through it, the same as any other compound value.
fn runtime() -> &'static std::path::Path {
    support::runtime()
}

const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern uint32_t no_capture(int8_t, int64_t, int64_t *) __asm__("PREFIXsouther2.closures.no_capture");
extern uint32_t with_struct(int8_t, int64_t *, int64_t, int64_t *) __asm__("PREFIXsouther2.closures.with_struct");
extern uint32_t aborting(int8_t, int64_t, int64_t *) __asm__("PREFIXsouther2.closures.aborting");
extern uint32_t adder(int64_t, int8_t, int64_t, int64_t *) __asm__("PREFIXsouther2.closures.adder");
extern uint32_t nested(int64_t, int8_t, int8_t, int64_t *) __asm__("PREFIXsouther2.closures.nested");
extern uint32_t handed_over(int8_t, int64_t, int64_t *) __asm__("PREFIXsouther2.closures.handed_over");

int main(int argc, char **argv) {
    if (argc != 2) {
        return 2;
    }
    const char *which = argv[1];
    int64_t out;
    uint32_t status;

    if (strcmp(which, "no_capture_true") == 0) {
        status = no_capture(1, 10, &out);
    } else if (strcmp(which, "no_capture_false") == 0) {
        status = no_capture(0, 10, &out);
    } else if (strcmp(which, "with_struct_true") == 0) {
        int64_t box[2] = {0, 7};
        status = with_struct(1, box, 10, &out);
    } else if (strcmp(which, "with_struct_false") == 0) {
        int64_t box[2] = {0, 7};
        status = with_struct(0, box, 10, &out);
    } else if (strcmp(which, "aborting_true") == 0) {
        status = aborting(1, 10, &out);
    } else if (strcmp(which, "aborting_false") == 0) {
        status = aborting(0, 10, &out);
    } else if (strcmp(which, "adder_true") == 0) {
        status = adder(5, 1, 10, &out);
    } else if (strcmp(which, "adder_false") == 0) {
        status = adder(5, 0, 10, &out);
    } else if (strcmp(which, "nested_true_true") == 0) {
        status = nested(2, 1, 1, &out);
    } else if (strcmp(which, "nested_true_false") == 0) {
        status = nested(2, 1, 0, &out);
    } else if (strcmp(which, "nested_false") == 0) {
        status = nested(2, 0, 1, &out);
    } else if (strcmp(which, "handed_over_true") == 0) {
        status = handed_over(1, 10, &out);
    } else if (strcmp(which, "handed_over_false") == 0) {
        status = handed_over(0, 10, &out);
    } else {
        return 2;
    }

    printf("%u\n", status);
    if (status == 0) {
        printf("%" PRId64 "\n", out);
    }
    return 0;
}
"#;

struct Answered {
    status: u32,
    value: Option<i64>,
}

fn answered(output: &Output) -> Answered {
    assert!(output.status.success(), "the process itself failed: {output:?}");
    let said = String::from_utf8_lossy(&output.stdout);
    let mut lines = said.lines();
    let status: u32 = lines.next().expect("a status").parse().expect("a status as a number");
    let value = lines.next().map(|it| it.parse().expect("an Int"));
    Answered { status, value }
}

/// A closure over nothing at all, applied — the smallest a `Node::Block` can be.
#[test]
fn a_closure_over_nothing_is_applied() {
    let (_swept, built) = build();
    assert_eq!(answered(&run(&built, "no_capture_true")).value, Some(11));
    assert_eq!(answered(&run(&built, "no_capture_false")).value, Some(9));
}

/// A closure over one value of a declared type. The field it reads comes through the closure's
/// own captured pointer, restored inside the lifted function — not read a second time from
/// whatever the caller of `Apply` happened to be holding.
#[test]
fn a_closure_over_a_declared_value_reads_its_field_through_the_capture() {
    let (_swept, built) = build();
    assert_eq!(answered(&run(&built, "with_struct_true")).value, Some(17));
    assert_eq!(answered(&run(&built, "with_struct_false")).value, Some(3));
}

/// Both branches close over the same `Int`, laid out at the same slot in either closure — the
/// caller of `Apply` never asks which branch it is holding, only ever `closure[0]`.
#[test]
fn both_branches_of_an_if_choosing_a_closure_answer_correctly_whichever_is_taken() {
    let (_swept, built) = build();
    assert_eq!(answered(&run(&built, "adder_true")).value, Some(15), "10 + 5");
    assert_eq!(answered(&run(&built, "adder_false")).value, Some(50), "10 * 5");
}

/// A closure whose body leaves `Int`'s range: the abort has to reach the caller through the
/// indirect call exactly the way it reaches one through a direct call — read the status back,
/// forward it, and never read `out`.
#[test]
fn an_abort_inside_a_closures_body_forwards_through_apply() {
    let (_swept, built) = build();

    let ok = answered(&run(&built, "aborting_true"));
    assert_eq!(ok.status, 0, "ANSWERED");
    assert_eq!(ok.value, Some(9223372036854775806), "MAX - 1");

    let aborted = answered(&run(&built, "aborting_false"));
    assert_eq!(aborted.status, 5, "REQUIRED_FORM_HAS_NO_PLACE");
    assert_eq!(aborted.value, None);
}

/// A closure inside a closure. The inner block's own capture list reaches the outer block's own
/// parameter and a value the outer `let` bound; the outer block's own capture list, in turn,
/// carries the outer parameter `a` forward only because the inner block needs it — nothing in the
/// outer body's own arithmetic ever reads `a` directly.
#[test]
fn a_closure_nested_inside_another_closure_carries_the_outer_scope_all_the_way_in() {
    let (_swept, built) = build();
    // b = x + 1 = 6; inner = y -> y + a + b; inner(100) = 100 + 2 + 6
    assert_eq!(answered(&run(&built, "nested_true_true")).value, Some(108));
    // inner = y -> y * a * b; inner(100) = 100 * 2 * 6
    assert_eq!(answered(&run(&built, "nested_true_false")).value, Some(1200));
    // c1 = false: outer = x -> x; outer(5) = 5, whatever a and c2 are
    assert_eq!(answered(&run(&built, "nested_false")).value, Some(5));
}

/// A closure handed over as an ordinary machine parameter to a *different* generated function
/// (`apply_n`, a recursive helper) and applied there, three times over via that helper's own
/// recursion — not built and applied inside the one function that built it, the way every closure
/// above is. `signature_over`'s `Ty::Fn -> POINTER` mapping is exercised here as a plain parameter
/// type on an otherwise-ordinary local definition, not only by the closure's own site machinery.
#[test]
fn a_closure_handed_over_as_a_parameter_is_applied_by_the_function_it_was_handed_to() {
    let (_swept, built) = build();
    // f = y -> y + 1; apply_n(f, 3, 10) = f(f(f(10))) = 13
    assert_eq!(answered(&run(&built, "handed_over_true")).value, Some(13));
    // f = y -> y * 2; apply_n(f, 3, 10) = f(f(f(10))) = 80
    assert_eq!(answered(&run(&built, "handed_over_false")).value, Some(80));
}

fn build() -> (TempDir, PathBuf) {
    let into = tempdir().expect("a directory to work in");
    let into_path = into.path().to_path_buf();

    let object = into_path.join("closures.o");
    fs::write(&object, object_for(CLOSURES).expect("an object")).expect("the object written");

    let harness = into_path.join("harness.c");
    fs::write(&harness, HARNESS.replace("PREFIX", PREFIX)).expect("the harness written");

    let executable = into_path.join("closures");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness)
        .arg(&object)
        .arg(runtime())
        .output()
        .expect("a C compiler to link with");
    assert!(
        linked.status.success(),
        "the link failed: {}",
        String::from_utf8_lossy(&linked.stderr)
    );
    (into, executable)
}

fn run(executable: &Path, which: &str) -> Output {
    Command::new(executable)
        .arg(which)
        .output()
        .expect("the executable to run")
}
