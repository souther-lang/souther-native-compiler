//! What a build for a host writes, held to itself: the header, the manifest, the object and the
//! shared library name one set of functions, and a C program that includes the header and nothing
//! else calls them through the library.
//!
//! The sets are asked of each artifact as it is — the header read as C declarations, the manifest
//! read as JSON, the object and the library read by `nm` — and not of the surface they were written
//! from, which is what they are being held to.

use serde_json::Value;
use souther_native_driver::{Linking, library_for, object_for};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

mod support;
use support::PREFIX;

const ADDING: &str = include_str!("adding.transport.json");
const VALUES: &str = include_str!("values.transport.json");
/// `m.lookUp` has no body and declares nothing to depend on, and holds its answer to not being
/// below what it was handed; `m.twice` doubles what it answers, and `m.looked` is a composition
/// with it as the first stage.
const ENSURES: &str = include_str!("ensures.transport.json");

/// What a library is linked from beside the program's object: these builds, and the runtime.
fn linking(builds: Vec<std::path::PathBuf>) -> Linking {
    Linking {
        builds,
        runtime: support::runtime().to_path_buf(),
    }
}

/// Every function the header declares, by the name before its parameters. A type the header names
/// is not one.
fn declared_in(header: &str) -> BTreeSet<String> {
    header
        .lines()
        .filter(|line| line.ends_with(");") && !line.starts_with("typedef "))
        .map(|line| {
            let before = &line[..line.find('(').expect("a declaration takes something")];
            before
                .rsplit([' ', '*'])
                .next()
                .expect("a declaration names its function")
                .to_string()
        })
        .collect()
}

/// Every function the manifest names, wherever it names one: every member shaped as a function is
/// (a name, what it takes and what it answers, and nothing else), and what a host registers an
/// implementation through. Found by walking the whole manifest rather than by a list of where
/// functions are kept, so a function a later version puts somewhere new is held to the header and
/// the library without this having to be told, and a type named where a function is expected is
/// caught for not being defined.
fn described_in(manifest: &Value) -> BTreeSet<String> {
    fn walk(value: &Value, named: &mut BTreeSet<String>) {
        match value {
            Value::Object(members) => {
                let mut keys: Vec<&str> = members.keys().map(String::as_str).collect();
                keys.sort_unstable();
                if keys == ["answers", "name", "takes"] {
                    named.insert(members["name"].as_str().unwrap().to_string());
                }
                if let Some(register) = members.get("register") {
                    named.insert(register.as_str().unwrap().to_string());
                }
                members.values().for_each(|it| walk(it, named));
            }
            Value::Array(items) => items.iter().for_each(|it| walk(it, named)),
            _ => {}
        }
    }
    let mut named = BTreeSet::new();
    walk(manifest, &mut named);
    named
}

/// Every symbol a file defines and makes visible outside it, spelt the way a C declaration is.
fn defined_in(file: &Path, exported: bool) -> BTreeSet<String> {
    // Linux is the only other host these compile on (`support::PREFIX`).
    let mut command = Command::new("nm");
    if cfg!(target_os = "macos") {
        command.arg("-gU");
    } else if exported {
        command.args(["-D", "--defined-only"]);
    } else {
        command.args(["-g", "--defined-only"]);
    }
    let said = command.arg(file).output().expect("nm to run");
    assert!(
        said.status.success(),
        "{}",
        String::from_utf8_lossy(&said.stderr)
    );
    String::from_utf8(said.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().nth(2))
        .map(|name| name.strip_prefix(PREFIX).unwrap_or(name).to_string())
        .collect()
}

#[test]
fn the_header_the_manifest_and_the_library_name_one_set_of_functions() {
    for document in [ADDING, VALUES, ENSURES] {
        let into = tempdir().unwrap();
        let built = library_for(document, &linking(vec![]), into.path()).unwrap();

        let declarations = fs::read_to_string(&built.declarations).unwrap();
        // What an FFI with no preprocessor reads: not one directive, whatever the program is.
        for line in declarations.lines() {
            assert!(!line.trim_start().starts_with('#'), "{line}");
        }
        let header = declared_in(&declarations);
        let manifest: Value =
            serde_json::from_str(&fs::read_to_string(&built.manifest).unwrap()).unwrap();
        let described = described_in(&manifest);
        let exported = defined_in(&built.library, true);

        assert!(!header.is_empty());
        assert_eq!(header, described, "the header and the manifest");
        assert_eq!(header, exported, "the header and the library");

        // What the object defines for a host is what the manifest says it does; the rest of what
        // it defines is between objects this compiler built, and is spelt so no C name is one.
        let host = format!("souther{}_", manifest["abi"]);
        let in_the_object: BTreeSet<String> = defined_in(&built.object, false)
            .into_iter()
            .filter(|name| name.starts_with(&host))
            .collect();
        let of_the_object: BTreeSet<String> = described
            .iter()
            .filter(|name| name.starts_with(&host))
            .cloned()
            .collect();
        assert_eq!(in_the_object, of_the_object, "the object and the manifest");
        for name in &exported {
            assert!(
                !name.contains(['.', '$']),
                "{name} is exported and is not what a host calls"
            );
        }
    }
}

/// A behavior reached through the header alone: no `__asm__` label, and nothing declared by hand.
const CALLING: &str = r#"
#include <inttypes.h>
#include <stdio.h>
#include "souther.h"

int main(void) {
    int64_t mark = souther_mark();
    int64_t answer = -1;
    souther_status status = souther3_m_calculation_b_add(2, 3, &answer);
    printf("%u %" PRId64 "\n", status, answer);
    answer = -1;
    status = souther3_m_calculation_b_add(INT64_MAX, 1, &answer);
    printf("%d %" PRId64 "\n", status == SOUTHER_REQUIRED_FORM_HAS_NO_PLACE, answer);
    souther_reset(mark);
    return 0;
}
"#;

/// A value built, read, written and read back, and a published value read, the same way.
const VALUING: &str = r#"
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
#include "souther.h"

static void said(souther_string text) {
    fwrite(souther_string_bytes(text), 1, (size_t) souther_string_length(text), stdout);
    printf("\n");
}

int main(void) {
    int64_t mark = souther_mark();
    souther_value built = NULL;
    souther_status status = souther3_m_m_t_P_construct(7, &built);
    printf("%u %" PRId64 "\n", status, souther3_m_m_t_P_f_n(built));
    said(souther3_m_m_t_P_encode(built));

    const char *json = "{\"n\": 9}";
    souther_decoded reading = NULL;
    status = souther3_m_m_t_P_decode((const uint8_t *) json, (int64_t) strlen(json), &reading);
    printf("%u %d %" PRId64 "\n", status, souther_decoded_outcome(reading) == SOUTHER_DECODED_VALUE,
           souther3_m_m_t_P_f_n(souther_decoded_value(reading)));

    json = "{}";
    status = souther3_m_m_t_P_decode((const uint8_t *) json, (int64_t) strlen(json), &reading);
    souther_issue issue = souther_decoded_issue(reading, 0);
    printf("%u %d %" PRId64 " ", status, souther_decoded_outcome(reading) == SOUTHER_DECODED_ISSUES,
           souther_decoded_issue_count(reading));
    said(souther_issue_code(issue));

    souther_value published = NULL;
    status = souther3_m_m_v_ys(&published);
    printf("%u %" PRId64 "\n", status, souther3_m_m_t_P_f_n(published));
    souther_reset(mark);
    return 0;
}
"#;

/// The same behavior from C++, through the same header: C linkage is the header's to say.
const CALLING_FROM_CPP: &str = r#"
#include <cstdio>
#include "souther.h"

int main() {
    int64_t answer = -1;
    souther_status status = souther3_m_calculation_b_add(2, 3, &answer);
    std::printf("%u %lld\n", status, static_cast<long long>(answer));
    return 0;
}
"#;

fn ran(document: &str, program: &str) -> String {
    ran_as(document, program, "cc", "host.c")
}

fn ran_as(document: &str, program: &str, compiler: &str, named: &str) -> String {
    let into = tempdir().unwrap();
    let built = library_for(document, &linking(vec![]), into.path()).unwrap();
    let source = into.path().join(named);
    fs::write(&source, program).unwrap();
    let executable = into.path().join("host");
    let compiled = Command::new(compiler)
        .args(["-Wall", "-Werror", "-o"])
        .arg(&executable)
        .arg(&source)
        .arg("-I")
        .arg(into.path())
        .arg(&built.library)
        .arg(format!("-Wl,-rpath,{}", into.path().display()))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let run = Command::new(&executable).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8(run.stdout).unwrap()
}

#[test]
fn a_host_calls_a_behavior_through_the_header_and_the_library() {
    assert!(!CALLING.contains("__asm__"));
    assert_eq!(ran(ADDING, CALLING), "0 5\n1 -1\n");
}

#[test]
fn a_host_builds_reads_and_writes_a_value_through_the_header_and_the_library() {
    assert!(!VALUING.contains("__asm__"));
    assert_eq!(
        ran(VALUES, VALUING),
        "0 7\n{\"n\":7}\n0 1 9\n0 1 1 missing_field\n0 42\n"
    );
}

/// A behavior with no body answered by what a host registered for it on the calling thread, and
/// by nothing where it registered nothing.
const IMPLEMENTING: &str = r#"
#include <pthread.h>
#include <stdio.h>
#include "souther.h"

static souther_status added(int64_t a, int64_t *out) {
    *out = a + 20;
    return SOUTHER_ANSWERED;
}

static souther_status below(int64_t a, int64_t *out) {
    *out = a - 1;
    return SOUTHER_ANSWERED;
}

static souther_status thrown(int64_t a, int64_t *out) {
    return SOUTHER_HOST_EXCEPTION;
}

static souther_status aborted(int64_t a, int64_t *out) {
    return SOUTHER_INVARIANT_NOT_HELD;
}

static souther_status unbound(int64_t a, int64_t *out) {
    return SOUTHER_INJECTION_UNBOUND;
}

/* Registers another implementation around a call of its own, and puts back what it replaced. */
static souther_status nesting(int64_t a, int64_t *out) {
    souther3_m_m_b_lookUp_implementation before = souther3_m_m_b_lookUp_register(added);
    int64_t inner = -1;
    souther_status status = souther3_m_m_b_twice(a, &inner);
    souther3_m_m_b_lookUp_implementation replaced = souther3_m_m_b_lookUp_register(before);
    printf("inner %u %lld %d\n", status, (long long) inner, replaced == added);
    *out = a;
    return SOUTHER_ANSWERED;
}

static void *elsewhere(void *ignored) {
    int64_t answer = -1;
    souther_status status = souther3_m_m_b_twice(1, &answer);
    printf("elsewhere %d %lld\n", status == SOUTHER_INJECTION_UNBOUND, (long long) answer);
    return NULL;
}

static void twice(const char *what) {
    int64_t answer = -1;
    souther_status status = souther3_m_m_b_twice(1, &answer);
    printf("%s %u %lld\n", what, status, (long long) answer);
}

int main(void) {
    int64_t mark = souther_mark();
    int64_t answer = -1;
    souther_status status = souther3_m_m_b_twice(1, &answer);
    printf("nothing %d %lld\n", status == SOUTHER_INJECTION_UNBOUND, (long long) answer);

    printf("first %d\n", souther3_m_m_b_lookUp_register(added) == NULL);
    twice("added");
    answer = -1;
    status = souther3_m_m_b_looked(1, &answer);
    printf("looked %u %lld\n", status, (long long) answer);

    pthread_t thread;
    pthread_create(&thread, NULL, elsewhere, NULL);
    pthread_join(thread, NULL);

    printf("replaced %d\n", souther3_m_m_b_lookUp_register(below) == added);
    answer = -1;
    status = souther3_m_m_b_twice(1, &answer);
    printf("below %d %lld\n", status == SOUTHER_ENSURES_NOT_HELD, (long long) answer);

    souther3_m_m_b_lookUp_register(thrown);
    answer = -1;
    status = souther3_m_m_b_twice(1, &answer);
    printf("thrown %d %lld\n", status == SOUTHER_HOST_EXCEPTION, (long long) answer);

    souther3_m_m_b_lookUp_register(aborted);
    status = souther3_m_m_b_twice(1, &answer);
    printf("aborted %d\n", status == SOUTHER_INJECTION_PROTOCOL_VIOLATION);

    souther3_m_m_b_lookUp_register(unbound);
    status = souther3_m_m_b_twice(1, &answer);
    printf("claimed %d\n", status == SOUTHER_INJECTION_PROTOCOL_VIOLATION);

    souther3_m_m_b_lookUp_register(nesting);
    twice("outer");
    printf("kept %d\n", souther3_m_m_b_lookUp_register(NULL) == nesting);
    status = souther3_m_m_b_twice(1, &answer);
    printf("removed %d\n", status == SOUTHER_INJECTION_UNBOUND);
    souther_reset(mark);
    return 0;
}
"#;

#[test]
fn a_host_implements_a_behavior_with_no_body_by_registering_it() {
    assert!(!IMPLEMENTING.contains("__asm__"));
    assert_eq!(
        ran(ENSURES, IMPLEMENTING),
        "nothing 1 -1\n\
         first 1\n\
         added 0 42\n\
         looked 0 42\n\
         elsewhere 1 -1\n\
         replaced 1\n\
         below 1 -1\n\
         thrown 1 -1\n\
         aborted 1\n\
         claimed 1\n\
         inner 0 42 1\n\
         outer 0 2\n\
         kept 1\n\
         removed 1\n"
    );
}

#[test]
fn a_cpp_program_calls_a_behavior_through_the_same_header() {
    assert_eq!(ran_as(ADDING, CALLING_FROM_CPP, "c++", "host.cpp"), "0 5\n");
}

/// A module is declared by one build, so an object carrying one this build carries too is refused
/// rather than linked as a second definition of everything in it.
#[test]
fn a_module_two_objects_carry_is_refused() {
    let into = tempdir().unwrap();
    let again = into.path().join("again.o");
    fs::write(&again, object_for(VALUES).unwrap()).unwrap();
    let refused = library_for(VALUES, &linking(vec![again]), &into.path().join("built"))
        .err()
        .expect("a module in two objects is refused");
    assert!(
        refused
            .to_string()
            .contains("the module m is carried by two"),
        "{refused}"
    );
}

/// A build that fails part of the way leaves the last build as it was, and nothing beside it: its
/// object, header and manifest are not put next to the last one's library.
#[test]
fn a_build_that_fails_leaves_the_last_one_whole() {
    let into = tempdir().unwrap();
    let built = into.path().join("built");
    library_for(ADDING, &linking(vec![]), &built).unwrap();
    let before: Vec<Vec<u8>> = ["souther.o", "souther.json"]
        .iter()
        .map(|it| fs::read(built.join(it)).unwrap())
        .collect();
    let again = into.path().join("again.o");
    fs::write(&again, object_for(VALUES).unwrap()).unwrap();

    library_for(VALUES, &linking(vec![again]), &built)
        .err()
        .expect("a module in two objects is refused");

    let after: Vec<Vec<u8>> = ["souther.o", "souther.json"]
        .iter()
        .map(|it| fs::read(built.join(it)).unwrap())
        .collect();
    assert!(before == after, "the last build was written over");
    let beside: BTreeSet<String> = fs::read_dir(into.path())
        .unwrap()
        .map(|it| it.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        beside,
        BTreeSet::from(["built".to_string(), "again.o".to_string()])
    );
}

/// A directory holding what no build writes is not replaced by one, and keeps what it holds.
#[test]
fn a_directory_a_build_did_not_write_is_not_replaced() {
    let into = tempdir().unwrap();
    let mine = into.path().join("notes.txt");
    fs::write(&mine, "kept").unwrap();

    let refused = library_for(ADDING, &linking(vec![]), into.path())
        .err()
        .expect("a directory holding another file");

    assert!(refused.to_string().contains("notes.txt"), "{refused}");
    assert_eq!(fs::read_to_string(&mine).unwrap(), "kept");
}

/// An object that says nothing of what it offers a host is not linked into a library as though it
/// offered nothing.
#[test]
fn an_object_carrying_no_surface_is_refused() {
    let into = tempdir().unwrap();
    let source = into.path().join("other.c");
    fs::write(&source, "int other(void) { return 0; }\n").unwrap();
    let other = into.path().join("other.o");
    let compiled = Command::new("cc")
        .arg("-c")
        .arg(&source)
        .arg("-o")
        .arg(&other)
        .status()
        .unwrap();
    assert!(compiled.success());
    let refused = library_for(ADDING, &linking(vec![other]), &into.path().join("built"))
        .err()
        .expect("an object with no surface is refused");
    assert!(
        refused
            .to_string()
            .contains("carries no surface for a host"),
        "{refused}"
    );
}
