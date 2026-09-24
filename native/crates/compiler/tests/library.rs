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

/// What a library is linked from beside the program's object: these builds, these objects
/// supplying what no build defines, and the runtime.
fn linking(builds: Vec<std::path::PathBuf>, supplying: Vec<std::path::PathBuf>) -> Linking {
    Linking {
        builds,
        supplying,
        runtime: support::runtime().to_path_buf(),
    }
}

/// Every function the header declares, by the name before its parameters.
fn declared_in(header: &str) -> BTreeSet<String> {
    header
        .lines()
        .filter(|line| line.ends_with(");"))
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

/// Every function the manifest names, wherever it names one.
fn described_in(manifest: &Value) -> BTreeSet<String> {
    let mut named = BTreeSet::new();
    let mut add = |function: &Value| {
        if let Some(name) = function.get("name").and_then(Value::as_str) {
            named.insert(name.to_string());
        }
    };
    for function in manifest["runtime"].as_array().unwrap() {
        add(function);
    }
    for module in manifest["modules"].as_array().unwrap() {
        for behavior in module["behaviors"].as_array().unwrap() {
            add(&behavior["call"]);
        }
        for value in module["values"].as_array().unwrap() {
            add(&value["read"]);
        }
        // Each kind has the members it has: a newtype one field, a sum its cases and no
        // constructor. A member a kind has not got is absent, and indexing it answers null.
        for declaration in module["declarations"].as_array().unwrap() {
            for operation in ["construct", "case", "decode", "encode"] {
                add(&declaration[operation]);
            }
            for field in declaration["fields"].as_array().into_iter().flatten() {
                add(&field["read"]);
            }
            add(&declaration["field"]["read"]);
        }
    }
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
    for document in [ADDING, VALUES] {
        let into = tempdir().unwrap();
        let built = library_for(document, &linking(vec![], vec![]), into.path()).unwrap();

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
    souther_status status = souther2_m_calculation_b_add(2, 3, &answer);
    printf("%u %" PRId64 "\n", status, answer);
    answer = -1;
    status = souther2_m_calculation_b_add(INT64_MAX, 1, &answer);
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
    souther_status status = souther2_m_m_t_P_construct(7, &built);
    printf("%u %" PRId64 "\n", status, souther2_m_m_t_P_f_n(built));
    said(souther2_m_m_t_P_encode(built));

    const char *json = "{\"n\": 9}";
    souther_decoded reading = NULL;
    status = souther2_m_m_t_P_decode((const uint8_t *) json, (int64_t) strlen(json), &reading);
    printf("%u %d %" PRId64 "\n", status, souther_decoded_outcome(reading) == SOUTHER_DECODED_VALUE,
           souther2_m_m_t_P_f_n(souther_decoded_value(reading)));

    json = "{}";
    status = souther2_m_m_t_P_decode((const uint8_t *) json, (int64_t) strlen(json), &reading);
    souther_issue issue = souther_decoded_issue(reading, 0);
    printf("%u %d %" PRId64 " ", status, souther_decoded_outcome(reading) == SOUTHER_DECODED_ISSUES,
           souther_decoded_issue_count(reading));
    said(souther_issue_code(issue));

    souther_value published = NULL;
    status = souther2_m_m_v_ys(&published);
    printf("%u %" PRId64 "\n", status, souther2_m_m_t_P_f_n(published));
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
    souther_status status = souther2_m_calculation_b_add(2, 3, &answer);
    std::printf("%u %lld\n", status, static_cast<long long>(answer));
    return 0;
}
"#;

fn ran(document: &str, program: &str) -> String {
    ran_as(document, program, "cc", "host.c")
}

fn ran_as(document: &str, program: &str, compiler: &str, named: &str) -> String {
    let into = tempdir().unwrap();
    let built = library_for(document, &linking(vec![], vec![]), into.path()).unwrap();
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
    let refused = library_for(
        VALUES,
        &linking(vec![again], vec![]),
        &into.path().join("built"),
    )
    .err()
    .expect("a module in two objects is refused");
    assert!(
        refused
            .to_string()
            .contains("the module m is carried by two"),
        "{refused}"
    );
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
    let refused = library_for(
        ADDING,
        &linking(vec![other], vec![]),
        &into.path().join("built"),
    )
    .err()
    .expect("an object with no surface is refused");
    assert!(
        refused
            .to_string()
            .contains("carries no surface for a host"),
        "{refused}"
    );
}
