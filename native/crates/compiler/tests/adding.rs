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

/// What the object calls that is not its own code. A published behavior is also an entry a host
/// reaches for its answer as the language writes it, and writing that is the runtime's.
const RUNTIME: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/debug/libsouther_native_runtime.a");

/// Written in the width the object actually answers in. `long` is that width on the platforms this
/// builds on today and is not the same thing: what the behavior takes and answers is an `Int`, and
/// an `Int` is sixty-four bits wherever it is.
///
/// Every generated function answers `status + out` (see `signature_over`'s own doc in the crate
/// this is a test of): a status this harness prints on its own line, and — only where that status
/// is `ANSWERED` (zero) — the value written through the pointer this hands over, on the line under
/// it. A caller that read the second line without checking the first would be reading whatever was
/// last in the room the pointer names, answer or not.
const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern uint32_t adding(int64_t, int64_t, int64_t *) __asm__("PREFIXsouther2.calculation.add");

int main(int argc, char **argv) {
    if (argc != 3) {
        return 2;
    }
    int64_t answer;
    uint32_t status = adding(strtoll(argv[1], NULL, 10), strtoll(argv[2], NULL, 10), &answer);
    printf("%u\n", status);
    if (status == 0) {
        printf("%" PRId64 "\n", answer);
    }
    return 0;
}
"#;

/// The same behavior reached at its boundary: the answer comes back as the JSON the language
/// writes an `Int` as, in a string of the runtime's layout, and the harness prints its bytes as
/// they are.
const BOUNDARY_HARNESS: &str = r#"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern uint32_t adding(int64_t, int64_t, const uint8_t **) __asm__("PREFIXsouther2.calculation.add$boundary");
extern int64_t souther_string_length(const uint8_t *);
extern const uint8_t *souther_string_bytes(const uint8_t *);

int main(int argc, char **argv) {
    if (argc != 3) {
        return 2;
    }
    const uint8_t *written;
    uint32_t status = adding(strtoll(argv[1], NULL, 10), strtoll(argv[2], NULL, 10), &written);
    printf("%u\n", status);
    if (status == 0) {
        fwrite(souther_string_bytes(written), 1, (size_t) souther_string_length(written), stdout);
        printf("\n");
    }
    return 0;
}
"#;

/// What issue #9 adds: not just whether a run answered, but which of a fixed set of reasons it did
/// not, read straight off the process's own output rather than guessed from whether it crashed.
struct Answered {
    status: u32,
    value: Option<i64>,
}

fn answered(output: &Output) -> Answered {
    assert!(output.status.success(), "the process itself failed: {output:?}");
    let said = String::from_utf8_lossy(&output.stdout);
    let mut lines = said.lines();
    let status: u32 = lines
        .next()
        .expect("a status on the first line")
        .parse()
        .expect("a status this harness wrote as a number");
    let value = lines.next().map(|it| it.parse().expect("an Int on the second line"));
    Answered { status, value }
}

/// A published behavior is an entry at its boundary as well: what it answers leaves as the JSON
/// the language writes it as, and a run that ends without a value still ends with its status and
/// nothing written.
#[test]
fn an_addition_reached_at_its_boundary_answers_the_json_of_its_sum() {
    let (_swept, built) = build_with(BOUNDARY_HARNESS);

    let said = run(&built, "-2", "-40");
    assert!(said.status.success(), "the process itself failed: {said:?}");
    assert_eq!(String::from_utf8_lossy(&said.stdout), "0\n-42\n");

    let ended = run(&built, "9223372036854775807", "1");
    assert_eq!(String::from_utf8_lossy(&ended.stdout), "5\n");
}

#[test]
fn an_addition_of_two_numbers_answers_their_sum() {
    let (_swept, built) = build();
    let answered = answered(&run(&built, "2", "40"));

    assert_eq!(answered.status, 0, "ANSWERED");
    assert_eq!(answered.value, Some(42));
}

/// An `Int` that leaves the range it holds is a language abort and not an answer — and, since
/// issue #9, one this harness can read the reason for rather than only that the run did not
/// answer. `5` is `REQUIRED_FORM_HAS_NO_PLACE`'s wire number (`native_status` in the crate this
/// tests), the one member of `AbortSet` the checker gives an `Int` `+` over — a fact this test
/// pins down rather than leaves to a comment, since a status this backend answers and a status
/// upstream meant are two different claims and only a running program checks that they agree.
#[test]
fn a_sum_past_what_an_int_holds_answers_required_form_has_no_place() {
    let (_swept, built) = build();
    let answered = answered(&run(&built, "9223372036854775807", "1"));

    assert_eq!(answered.status, 5, "REQUIRED_FORM_HAS_NO_PLACE");
    assert_eq!(answered.value, None);
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
    build_with(HARNESS)
}

fn build_with(harness_source: &str) -> (TempDir, PathBuf) {
    let into = tempdir().expect("a directory to work in");
    let into_path = into.path().to_path_buf();

    let object = into_path.join("calculation.o");
    fs::write(&object, object_for(ADDING).expect("an object")).expect("the object written");

    let harness = into_path.join("harness.c");
    fs::write(&harness, harness_source.replace("PREFIX", PREFIX)).expect("the harness written");

    let executable = into_path.join("adding");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness)
        .arg(&object)
        .arg(RUNTIME)
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
