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

/// The document the Java half wrote, and the one its own test holds it to.
const ADDING: &str = include_str!("adding.transport.json");

/// What the linker on this platform calls a symbol the object names.
///
/// Mach-O writes an underscore before every one and ELF writes none. The object carries whichever
/// its format takes, so what needs saying here is only what a C declaration has to be written with
/// to reach it.
const PREFIX: &str = if cfg!(target_vendor = "apple") { "_" } else { "" };

const HARNESS: &str = r#"
#include <stdio.h>
#include <stdlib.h>

extern long adding(long, long) __asm__("PREFIXsouther.calculation.add");

int main(int argc, char **argv) {
    if (argc != 3) {
        return 2;
    }
    printf("%ld\n", adding(atol(argv[1]), atol(argv[2])));
    return 0;
}
"#;

#[test]
fn an_addition_of_two_numbers_answers_their_sum() {
    let built = build();
    let answered = run(&built, "2", "40");

    assert!(answered.status.success(), "the run ended: {answered:?}");
    assert_eq!(String::from_utf8_lossy(&answered.stdout).trim(), "42");
}

/// An `Int` that leaves the range it holds is an abort and not an answer. Which abort it was is
/// not asked here: nothing carries a reason out of a native run yet.
#[test]
fn a_sum_past_what_an_int_holds_ends_the_run() {
    let built = build();
    let answered = run(&built, "9223372036854775807", "1");

    assert!(!answered.status.success(), "it answered: {answered:?}");
    assert_eq!(String::from_utf8_lossy(&answered.stdout).trim(), "");
}

/// Compiles the document, links what came out, and answers where the executable is.
///
/// Under a directory of this process's own: a fixed name in a shared place is a file somebody
/// else's run is in the middle of writing.
fn build() -> PathBuf {
    let into = std::env::temp_dir().join(format!("souther-native-{}", std::process::id()));
    fs::create_dir_all(&into).expect("a directory to work in");

    let object = into.join("calculation.o");
    fs::write(&object, object_for(ADDING).expect("an object")).expect("the object written");

    let harness = into.join("harness.c");
    fs::write(&harness, HARNESS.replace("PREFIX", PREFIX)).expect("the harness written");

    let executable = into.join("adding");
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
    executable
}

fn run(executable: &Path, a: &str, b: &str) -> Output {
    Command::new(executable)
        .args([a, b])
        .output()
        .expect("the executable to run")
}
