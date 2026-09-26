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
/// (a name, what it takes and what it answers, and nothing else), and what a host makes a
/// capability of an implementation of its own through. Found by walking the whole manifest rather than by a list of where
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
                if let Some(implement) = members.get("implement") {
                    named.insert(implement.as_str().unwrap().to_string());
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
    souther_status status = souther4_m_calculation_b_add(NULL, 2, 3, &answer);
    printf("%u %" PRId64 "\n", status, answer);
    answer = -1;
    status = souther4_m_calculation_b_add(NULL, INT64_MAX, 1, &answer);
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
    souther_status status = souther4_m_m_t_P_construct(7, &built);
    printf("%u %" PRId64 "\n", status, souther4_m_m_t_P_f_n(built));
    said(souther4_m_m_t_P_encode(built));

    const char *json = "{\"n\": 9}";
    souther_decoded reading = NULL;
    status = souther4_m_m_t_P_decode((const uint8_t *) json, (int64_t) strlen(json), &reading);
    printf("%u %d %" PRId64 "\n", status, souther_decoded_outcome(reading) == SOUTHER_DECODED_VALUE,
           souther4_m_m_t_P_f_n(souther_decoded_value(reading)));

    json = "{}";
    status = souther4_m_m_t_P_decode((const uint8_t *) json, (int64_t) strlen(json), &reading);
    souther_issue issue = souther_decoded_issue(reading, 0);
    printf("%u %d %" PRId64 " ", status, souther_decoded_outcome(reading) == SOUTHER_DECODED_ISSUES,
           souther_decoded_issue_count(reading));
    said(souther_issue_code(issue));

    souther_value published = NULL;
    status = souther4_m_m_v_ys(&published);
    printf("%u %" PRId64 "\n", status, souther4_m_m_t_P_f_n(published));
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
    souther_status status = souther4_m_calculation_b_add(NULL, 2, 3, &answer);
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

/// A behavior with no body answered by what a host implements it as, through a capability the
/// host makes of that and hands where the behavior is required, and by nothing where it hands none.
///
/// Two implementations of the one behavior at once, each told apart by what it is handed first,
/// which one function pointer serves; one called from another thread; and one calling the program
/// again from inside itself with another, which neither sees the other's.
const IMPLEMENTING: &str = r#"
#include <pthread.h>
#include <stdio.h>
#include "souther.h"

static souther_status added(void *by, int64_t a, int64_t *out) {
    *out = a + *(const int64_t *) by;
    return SOUTHER_ANSWERED;
}

static souther_status below(void *by, int64_t a, int64_t *out) {
    *out = a - 1;
    return SOUTHER_ANSWERED;
}

static souther_status thrown(void *by, int64_t a, int64_t *out) {
    return SOUTHER_HOST_EXCEPTION;
}

static souther_status aborted(void *by, int64_t a, int64_t *out) {
    return SOUTHER_INVARIANT_NOT_HELD;
}

static souther_status unbound(void *by, int64_t a, int64_t *out) {
    return SOUTHER_INJECTION_UNBOUND;
}

/* Calls the program again with the requirements it was handed, and answers what it was asked. */
static souther_status nesting(void *by, int64_t a, int64_t *out) {
    int64_t inner = -1;
    souther_status status =
        souther4_m_m_b_twice((const souther_capability *const *) by, a, &inner);
    printf("inner %u %lld\n", status, (long long) inner);
    *out = a;
    return SOUTHER_ANSWERED;
}

typedef struct {
    souther_capability capability;
    souther_hosted hosted;
    const souther_capability *requirements[1];
} implemented;

static void implement(implemented *into, souther4_m_m_b_lookUp_implementation by, void *userdata) {
    souther4_m_m_b_lookUp_implement(&into->capability, &into->hosted, by, userdata);
    into->requirements[0] = &into->capability;
}

static void twice(const char *what, implemented *with) {
    int64_t answer = -1;
    souther_status status = souther4_m_m_b_twice(with->requirements, 1, &answer);
    printf("%s %u %lld\n", what, status, (long long) answer);
}

static void *elsewhere(void *with) {
    twice("elsewhere", (implemented *) with);
    return NULL;
}

int main(void) {
    int64_t mark = souther_mark();
    int64_t answer = -1;
    souther_status status = souther4_m_m_b_twice(NULL, 1, &answer);
    printf("nothing %d %lld\n", status == SOUTHER_INJECTION_UNBOUND, (long long) answer);
    const souther_capability *none[1] = {NULL};
    status = souther4_m_m_b_twice(none, 1, &answer);
    printf("none %d %lld\n", status == SOUTHER_INJECTION_UNBOUND, (long long) answer);

    int64_t twenty = 20, thirty = 30;
    implemented by_twenty, by_thirty;
    implement(&by_twenty, added, &twenty);
    implement(&by_thirty, added, &thirty);
    twice("added", &by_twenty);
    twice("other", &by_thirty);
    twice("again", &by_twenty);
    answer = -1;
    status = souther4_m_m_b_looked(by_twenty.requirements, 1, &answer);
    printf("looked %u %lld\n", status, (long long) answer);

    pthread_t thread;
    pthread_create(&thread, NULL, elsewhere, &by_thirty);
    pthread_join(thread, NULL);

    implemented by_below, by_thrown, by_aborted, by_unbound, by_nesting;
    implement(&by_below, below, NULL);
    answer = -1;
    status = souther4_m_m_b_twice(by_below.requirements, 1, &answer);
    printf("below %d %lld\n", status == SOUTHER_ENSURES_NOT_HELD, (long long) answer);

    implement(&by_thrown, thrown, NULL);
    status = souther4_m_m_b_twice(by_thrown.requirements, 1, &answer);
    printf("thrown %d %lld\n", status == SOUTHER_HOST_EXCEPTION, (long long) answer);

    implement(&by_aborted, aborted, NULL);
    status = souther4_m_m_b_twice(by_aborted.requirements, 1, &answer);
    printf("aborted %d\n", status == SOUTHER_INJECTION_PROTOCOL_VIOLATION);

    implement(&by_unbound, unbound, NULL);
    status = souther4_m_m_b_twice(by_unbound.requirements, 1, &answer);
    printf("claimed %d\n", status == SOUTHER_INJECTION_PROTOCOL_VIOLATION);

    implement(&by_nesting, nesting, (void *) by_twenty.requirements);
    twice("outer", &by_nesting);
    souther_reset(mark);
    return 0;
}
"#;

#[test]
fn a_host_implements_a_behavior_with_no_body_through_a_capability() {
    assert!(!IMPLEMENTING.contains("__asm__"));
    assert_eq!(
        ran(ENSURES, IMPLEMENTING),
        "nothing 1 -1\n\
         none 1 -1\n\
         added 0 42\n\
         other 0 62\n\
         again 0 42\n\
         looked 0 42\n\
         elsewhere 0 62\n\
         below 1 -1\n\
         thrown 1 -1\n\
         aborted 1\n\
         claimed 1\n\
         inner 0 42\n\
         outer 0 2\n"
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

/// A sum with a primitive and a case the language gives among its cases: no program declares one
/// (a sum's cases are declared, E1020), and the reader and the writer walk the cases of one the way
/// they walk a union's a behavior answers, so it is where both are put to a host's documents. The
/// primitive stands under the contents key beside its name, and the case the language gives is its
/// name alone.
const CARRYING: &str = r#"{"transport":25,"declarations":[
    {"module":"m","name":"A","by":"amodule","is":"unit"},
    {"module":"m","name":"Q","by":"amodule","is":"sum",
     "cases":[{"is":"primitive","prim":"INT"},{"is":"language","case":"DIVISION_BY_ZERO"},
              {"is":"declared","declared":"m.A"}],
     "form":{"is":"discriminated","tag":"type","contents":"value"}}],
  "behaviors":[],
  "modules":[{"name":"m","publishes":["m.A","m.Q"],"helpers":[],"values":[],"entries":[],
              "definitions":[],"examples":[]}]}"#;

const READING_CASES: &str = r#"
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
#include "souther.h"

static void said(souther_string text) {
    fwrite(souther_string_bytes(text), 1, (size_t) souther_string_length(text), stdout);
    printf("\n");
}

static void read(const char *json) {
    souther_decoded reading = NULL;
    souther_status status = souther4_m_m_t_Q_decode((const uint8_t *) json, (int64_t) strlen(json),
                                                   &reading);
    if (souther_decoded_outcome(reading) != SOUTHER_DECODED_VALUE) {
        printf("%u issues %" PRId64 " ", status, souther_decoded_issue_count(reading));
        said(souther_issue_code(souther_decoded_issue(reading, 0)));
        return;
    }
    souther_value value = souther_decoded_value(reading);
    uint32_t which = souther4_m_m_t_Q_case(value);
    printf("%u case %u", status, which);
    if (which == 0) {
        printf(" holds %" PRId64, souther_case_int_read(value));
    }
    printf(" ");
    said(souther4_m_m_t_Q_encode(value));
}

int main(void) {
    int64_t mark = souther_mark();
    read("{\"type\": \"Int\", \"value\": 4}");
    read("{\"type\": \"DivisionByZero\"}");
    read("{\"type\": \"A\"}");
    read("{\"type\": \"Int\"}");
    read("{\"type\": \"Int\", \"value\": true}");
    said(souther4_m_m_t_Q_encode(souther_case_int_make(9)));
    said(souther4_m_m_t_Q_encode(souther_case_division_by_zero_make()));
    souther_reset(mark);
    return 0;
}
"#;

#[test]
fn a_case_no_declaration_names_is_read_and_written_as_the_language_writes_it() {
    assert_eq!(
        ran(CARRYING, READING_CASES),
        concat!(
            "0 case 0 holds 4 {\"type\":\"Int\",\"value\":4}\n",
            "0 case 1 {\"type\":\"DivisionByZero\"}\n",
            "0 case 2 {\"type\":\"A\"}\n",
            "0 issues 1 missing_field\n",
            "0 issues 1 type_mismatch\n",
            "{\"type\":\"Int\",\"value\":9}\n",
            "{\"type\":\"DivisionByZero\"}\n",
        )
    );
}
