//! What the Java half writes for a value, its handovers, and the entry it publishes for one —
//! read back, and run end to end: a value's home defined the way a helper's is, its published
//! entry exported under `value_symbol`, and a call across a module boundary resolved to that same
//! entry rather than to a copy of the value's own body (#10).

use souther_native_driver::object_for;
use souther_native_driver::transport::{Program, Reaches};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

mod support;
use support::PREFIX;

/// A value that names another value at its root: `ks`, kept and handed nothing, and `ys`,
/// published and handed one `ks`.
const VALUES: &str = include_str!("values.transport.json");

/// A behavior in one module answering with a value another module publishes.
const PUBLISHED_VALUE: &str = include_str!("published_value.transport.json");

/// What generated code takes room from, needed here because both documents construct a value.
fn runtime() -> &'static std::path::Path {
    support::runtime()
}

#[test]
fn a_value_and_its_handover_read_back_as_the_checker_wrote_them() {
    let program = Program::read(VALUES).expect("every field this carries reads");
    let module = &program.modules[0];

    assert_eq!(module.values.len(), 2, "ks and ys");
    let ks = module
        .values
        .iter()
        .find(|it| it.name == "ks")
        .expect("ks is a value");
    let ys = module
        .values
        .iter()
        .find(|it| it.name == "ys")
        .expect("ys is a value");

    assert!(ks.handovers.is_empty(), "ks is handed nothing");
    assert_eq!(ys.handovers.len(), 1, "ys names ks at its root");
    assert_eq!(ys.handovers[0].carries.declared(), "m.ks");

    assert_eq!(module.entries.len(), 1, "only ys is published");
    assert_eq!(module.entries[0].value.declared(), "m.ys");
}

/// `ys`'s published entry, called the way anything outside its module has to reach it: `ks` is
/// built first inside the entry's own body, handed to `ys`'s home as the one handover it takes,
/// and the entry answers with what `ys` names — `P { n = 42 }`, unchanged, since `ys` is nothing
/// but a reference to `ks`.
#[test]
fn a_published_values_entry_answers_with_what_it_names() {
    let harness: &str = r#"
        #include <inttypes.h>
        #include <stdint.h>
        #include <stdio.h>

        extern uint32_t entry(int64_t *) __asm__("PREFIXsouther5.m$value$ys");

        int main(void) {
            int64_t out;
            uint32_t status = entry(&out);
            if (status != 0) {
                printf("aborted %u\n", status);
                return 0;
            }
            int64_t *answered = (int64_t *) out;
            printf("%" PRId64 "\n", answered[1]);
            return 0;
        }
    "#;
    let (_swept, built) = build("m.o", VALUES, harness);
    let answered = run(&built, &[]);

    assert!(answered.status.success(), "the run ended: {answered:?}");
    assert_eq!(String::from_utf8_lossy(&answered.stdout).trim(), "42");
}

/// A call reaching a value declared in the same module, and a call reaching one published by
/// another, are different `Reaches` variants on the wire — never the one tag doing for both, which
/// is exactly the distinction a typo in either spelling would erase without a test reading a real
/// occurrence of each back.
#[test]
fn a_local_value_reach_and_a_published_one_read_as_different_variants() {
    let program = Program::read(PUBLISHED_VALUE).expect("every field this carries reads");

    let publisher = program
        .modules
        .iter()
        .find(|it| it.name == "publisher")
        .unwrap();
    let entry = &publisher.entries[0];
    // The entry's own body reaches ks and then ys, both same-module: Reaches::Value.
    let mut local_reaches = 0;
    walk(&entry.body, &mut |reaches| {
        if matches!(reaches, Reaches::Value { .. }) {
            local_reaches += 1;
        }
        assert!(
            !matches!(reaches, Reaches::PublishedValue { .. }),
            "the entry's own body never crosses a module boundary"
        );
    });
    assert_eq!(local_reaches, 2, "the entry builds ks, then ys");

    let reader = program
        .modules
        .iter()
        .find(|it| it.name == "reader")
        .unwrap();
    let mut published_reaches = 0;
    for definition in &reader.definitions {
        walk(definition_body(definition), &mut |reaches| {
            if let Reaches::PublishedValue { module, name } = reaches {
                assert_eq!(module, "publisher");
                assert_eq!(name, "ys");
                published_reaches += 1;
            }
            assert!(
                !matches!(reaches, Reaches::Value { .. }),
                "reader holds no value of its own to reach locally"
            );
        });
    }
    assert_eq!(
        published_reaches, 1,
        "reader.g reaches publisher.ys across the boundary"
    );
}

/// `reader.g` reaches `publisher.ys` across the two objects' shared boundary — here, one object
/// holding both modules, but the call is emitted exactly as it would be split across two: `g`
/// calls `souther5.publisher$value$ys`, the same exported entry `reader` would import from a
/// separate build of `publisher`, never a copy of `ys`'s own body inlined into `reader`'s object.
#[test]
fn a_behavior_answering_with_another_modules_published_value_runs_it_there() {
    let harness: &str = r#"
        #include <inttypes.h>
        #include <stdint.h>
        #include <stdio.h>

        extern uint32_t g(const void *, int64_t *) __asm__("PREFIXsouther5.reader.g");

        int main(void) {
            int64_t out;
            uint32_t status = g(NULL, &out);
            if (status != 0) {
                printf("aborted %u\n", status);
                return 0;
            }
            int64_t *answered = (int64_t *) out;
            printf("%" PRId64 "\n", answered[1]);
            return 0;
        }
    "#;
    let (_swept, built) = build("reader.o", PUBLISHED_VALUE, harness);
    let answered = run(&built, &[]);

    assert!(answered.status.success(), "the run ended: {answered:?}");
    assert_eq!(String::from_utf8_lossy(&answered.stdout).trim(), "42");
}

fn build(object_name: &str, document: &str, harness: &str) -> (TempDir, PathBuf) {
    let into = tempdir().expect("a directory to work in");
    let into_path = into.path().to_path_buf();

    let object = into_path.join(object_name);
    fs::write(&object, object_for(document).expect("an object")).expect("the object written");

    let harness_file = into_path.join("harness.c");
    fs::write(&harness_file, harness.replace("PREFIX", PREFIX)).expect("the harness written");

    let executable = into_path.join("run");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness_file)
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

fn run(executable: &Path, args: &[&str]) -> Output {
    Command::new(executable)
        .args(args)
        .output()
        .expect("the executable to run")
}

fn definition_body(
    definition: &souther_native_driver::transport::Definition,
) -> &souther_native_driver::transport::Node {
    match definition {
        souther_native_driver::transport::Definition::Body { body, .. } => body,
        souther_native_driver::transport::Definition::Composed { .. } => {
            panic!("reader.g is a body, not a composition")
        }
    }
}

/// Every `Reaches` a `Node` tree's calls carry, depth first — the same walk `refusals.rs`'s own
/// documents are small enough not to need, but this one's isn't.
fn walk(node: &souther_native_driver::transport::Node, into: &mut impl FnMut(&Reaches)) {
    use souther_native_driver::transport::Node;
    match node {
        Node::Call {
            reaches, arguments, ..
        } => {
            into(reaches);
            for argument in arguments {
                walk(argument, into);
            }
        }
        Node::Let { value, body, .. } => {
            walk(value, into);
            walk(body, into);
        }
        _ => {}
    }
}
