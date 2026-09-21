//! The whole path, at the width of one addition: a checked program the Java half wrote out, an
//! object this driver emitted, the system linker, and a run that answers.
//!
//! What it answers is checked against what the same program answers elsewhere, which is the
//! harness this is the first row of. Until that arrives, the two answers here are ones no reading
//! of this code could have produced by agreeing with itself: the sum of two numbers, and the run
//! that ends instead of wrapping.

use souther_native_driver::object_for;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

/// The document the Java half wrote, and the one its own test holds it to.
const ADDING: &str = include_str!("adding.transport.json");

/// What the linker on this platform calls a symbol the object names.
///
/// Mach-O writes an underscore before every one and ELF writes none. The object carries whichever
/// its format takes, so what needs saying here is only what a C declaration has to be written with
/// to reach it.
const PREFIX: &str = if cfg!(target_vendor = "apple") { "_" } else { "" };

/// Written in the width the object actually answers in. `long` is that width on the platforms this
/// builds on today and is not the same thing: what the behavior takes and answers is an `Int`, and
/// an `Int` is sixty-four bits wherever it is.
const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern int64_t adding(int64_t, int64_t) __asm__("PREFIXsouther.calculation.add");

int main(int argc, char **argv) {
    if (argc != 3) {
        return 2;
    }
    printf("%" PRId64 "\n", adding(strtoll(argv[1], NULL, 10), strtoll(argv[2], NULL, 10)));
    return 0;
}
"#;

#[test]
fn an_addition_of_two_numbers_answers_their_sum() {
    let (_swept, built) = build();
    let answered = run(&built, "2", "40");

    assert!(answered.status.success(), "the run ended: {answered:?}");
    assert_eq!(String::from_utf8_lossy(&answered.stdout).trim(), "42");
}

/// An `Int` that leaves the range it holds is an abort and not an answer. Which abort it was is
/// not asked here: nothing carries a reason out of a native run yet.
#[test]
fn a_sum_past_what_an_int_holds_ends_the_run() {
    let (_swept, built) = build();
    let answered = run(&built, "9223372036854775807", "1");

    assert!(!answered.status.success(), "it answered: {answered:?}");
    assert_eq!(String::from_utf8_lossy(&answered.stdout).trim(), "");
}

/// Compiles the document, links what came out, and answers where the executable is.
///
/// Under a directory of this build's own, and not of this process's. Tests in one process run at
/// the same time by default, so a name that varies by process varies by less than what is using
/// it: two of them would write one object while the other's linker was reading it.
///
/// The directory is handed back with the executable so that it outlives the run and is swept up
/// after it.
fn build() -> (TempDir, PathBuf) {
    let into = tempdir().expect("a directory to work in");
    let into_path = into.path().to_path_buf();

    let object = into_path.join("calculation.o");
    fs::write(&object, object_for(ADDING).expect("an object")).expect("the object written");

    let harness = into_path.join("harness.c");
    fs::write(&harness, HARNESS.replace("PREFIX", PREFIX)).expect("the harness written");

    let executable = into_path.join("adding");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness)
        .arg(&object)
        .output()
        .expect("a C compiler to link with");
    assert!(
        linked.status.success(),
        "the link failed: {}",
        String::from_utf8_lossy(&linked.stderr)
    );
    (into, executable)
}

fn run(executable: &Path, a: &str, b: &str) -> Output {
    Command::new(executable)
        .args([a, b])
        .output()
        .expect("the executable to run")
}
