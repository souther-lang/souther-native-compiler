//! The whole path, at the width of a composition: a checked program the Java half wrote out as a
//! `Composition`, an object this driver lowered it to, the system linker, and a run that answers.
//!
//! `f : X -> A | B`, `g : A -> B`, `h : B -> C`, composed as `f >-> g >-> h`. What this exists to
//! catch is the ambiguity Issue #13's design calls out by name: there are two kinds of `B` a run
//! can be holding after `f` answers. One is `g`'s — the mainline case, offered to `h` because it
//! is one of the cases `h`'s stage accepts. The other is `f`'s own, retired at `g`'s stage because
//! it was never one of the cases *`g`* accepted; it does not come back, and `h` never sees it —
//! even though its tag is a `B`, which is also every case `h`'s own stage accepts. A lowering that
//! tested a retired value against a later stage's cases rather than answering with it where it
//! retired would call `h` on it anyway, since nothing about the value itself says which stage
//! left it standing. Souther calls this `#unmarked-sum`: a value never carries a mark saying it
//! once left the main line, so what has to get this right is structural — which block Cranelift is
//! in — and not a test of the value.

use souther_native_driver::object_for;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

mod support;

/// The document the Java half wrote, and the one its own test holds it to.
const COMPOSING: &str = include_str!("composing.transport.json");

/// What generated code takes room from, needed here because `g` and `h` each construct a value.
fn runtime() -> &'static std::path::Path {
    support::runtime()
}

/// Reads back what `pipeline` answered: its tag, by comparing the address every value of a
/// declared type carries against each declaration's own token, and the one field both `B` and `C`
/// happen to share.
const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

extern uint32_t pipeline(const void *, int64_t, int64_t *) __asm__("PREFIXsouther@.routing.pipeline");
extern uint8_t tokenOfB __asm__("PREFIXsouther$type$routing$B");
extern uint8_t tokenOfC __asm__("PREFIXsouther$type$routing$C");

int main(int argc, char **argv) {
    if (argc != 2) {
        return 2;
    }
    int64_t n = strtoll(argv[1], NULL, 10);
    int64_t out;
    uint32_t status = pipeline(NULL, n, &out);
    if (status != 0) {
        printf("aborted %u\n", status);
        return 0;
    }
    int64_t *answered = (int64_t *) out;
    int64_t which = answered[0];
    int64_t field = answered[1];
    const char *tag = "?";
    if (which == (int64_t) &tokenOfB) { tag = "B"; }
    if (which == (int64_t) &tokenOfC) { tag = "C"; }
    printf("%s %" PRId64 "\n", tag, field);
    return 0;
}
"#;

/// `f` answers `A` and the running value stays on the main line the whole way: `g` accepts `A`,
/// answers `B`, `h` accepts that `B`, and answers `C`. What comes back is `h`'s, not `f`'s.
#[test]
fn a_running_value_on_the_main_line_is_offered_to_every_stage_that_accepts_it() {
    let (_swept, built) = build();
    let answered = run(&built, "50");

    assert!(answered.status.success(), "the run ended: {answered:?}");
    assert_eq!(
        String::from_utf8_lossy(&answered.stdout).trim(),
        "C 101",
        "50 <= 100, so `f` answers `A {{ n = 50 }}`; `g` accepts it and answers `B {{ n = 100 }}`; \
         `h` accepts that and answers `C {{ n = 101 }}`"
    );
}

/// `f` answers `B` directly. `g`'s stage does not accept a `B` — it accepts only `A` — so this `B`
/// retires there and the composition answers with it exactly as it stood. It is never offered to
/// `h`, even though `h`'s own stage would have accepted it: a value that left the main line at
/// `g`'s stage is not tested against `h`'s cases, because it was never offered to `h` at all.
///
/// This is the case a lowering that carried a retired value forward to be tested against the next
/// stage gets wrong: `h` accepts `B`, so it would call `h` on this one too, answering `C { n = 501
/// }` — a tag and a value neither `f` nor a correct reading of the pipeline ever produced.
#[test]
fn a_value_that_retires_at_one_stage_is_not_offered_to_the_next() {
    let (_swept, built) = build();
    let answered = run(&built, "500");

    assert!(answered.status.success(), "the run ended: {answered:?}");
    assert_eq!(
        String::from_utf8_lossy(&answered.stdout).trim(),
        "B 500",
        "500 > 100, so `f` answers `B {{ n = 500 }}` directly; it is not one of the cases `g`'s \
         stage accepts, so it retires there and is never offered to `g` or to `h`"
    );
}

fn build() -> (TempDir, PathBuf) {
    let into = tempdir().expect("a directory to work in");
    let into_path = into.path().to_path_buf();

    let object = into_path.join("routing.o");
    fs::write(&object, object_for(COMPOSING).expect("an object")).expect("the object written");

    let harness = into_path.join("harness.c");
    fs::write(&harness, support::harness(HARNESS)).expect("the harness written");

    let executable = into_path.join("routing");
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

fn run(executable: &Path, n: &str) -> Output {
    Command::new(executable)
        .arg(n)
        .output()
        .expect("the executable to run")
}
