//! A function value crossing to a host and back, and from one object built by this compiler to
//! another: called through the function the object defines for the shape it crosses in, made by a
//! host of an implementation of its own, and called through its header by code another object
//! wrote.
//!
//! The documents are written here and not by the Java half from source: no module the checker
//! accepts yet publishes a value whose type is a function (souther-lang/souther#1974, #1990), so
//! no source would bring one this far. What they hold is what the Java half writes for such a value
//! when it is kept by its module and applied there — a block, the captures it reads bound around
//! it — with an entry beside it, which is what publishing it adds.
//!
//! `functions.transport.json` is the document [`publishing`] builds, written out for the Java half's
//! tests of what a binding makes of it, which have no source to check either; a test here holds the
//! two to one document.

use serde_json::{Value, json};
use souther_native_driver::transport::TRANSPORT_VERSION;
use souther_native_driver::{Linking, library_for, object_for};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

mod support;

fn int() -> Value {
    json!({"prim": "INT"})
}

fn truth() -> Value {
    json!({"prim": "BOOL"})
}

fn function(takes: Vec<Value>, answers: Value) -> Value {
    json!({"fn": {"takes": takes, "answers": answers}})
}

fn optional(of: Value) -> Value {
    json!({"option": of})
}

fn tuple(of: Vec<Value>) -> Value {
    json!({"tuple": of})
}

fn read(binding: usize, ty: Value) -> Value {
    json!({"core": "read", "binding": binding, "type": ty, "aborts": []})
}

fn number(value: i64) -> Value {
    json!({"core": "int", "value": value, "type": int(), "aborts": []})
}

fn added(left: Value, right: Value) -> Value {
    json!({
        "core": "binary", "op": "ADD", "reading": {"is": "astheystand"},
        "left": left, "right": right, "type": int(), "aborts": ["REQUIRED_FORM_HAS_NO_PLACE"]
    })
}

fn above(left: Value, right: Value) -> Value {
    json!({
        "core": "binary", "op": "GT", "reading": {"is": "astheystand"},
        "left": left, "right": right, "type": truth(), "aborts": []
    })
}

fn bound(binding: usize, value: Value, body: Value) -> Value {
    let binds = value["type"].clone();
    let ty = body["type"].clone();
    json!({"core": "let", "binding": binding, "binds": binds, "value": value, "body": body,
           "type": ty, "aborts": []})
}

fn block(site: usize, parameters: &[(usize, &str)], body: Value, ty: Value) -> Value {
    let parameters: Vec<Value> = parameters
        .iter()
        .map(|(binding, name)| json!({"binding": binding, "name": name}))
        .collect();
    json!({"core": "block", "site": site, "parameters": parameters, "body": body, "type": ty,
           "aborts": []})
}

fn applied(function: Value, arguments: Vec<Value>) -> Value {
    let ty = function["type"]["fn"]["answers"].clone();
    json!({"core": "apply", "function": function, "arguments": arguments, "type": ty,
           "aborts": []})
}

fn chosen(cond: Value, then: Value, otherwise: Value) -> Value {
    let ty = then["type"].clone();
    json!({"core": "if", "cond": cond, "then": then, "else": otherwise, "type": ty, "aborts": []})
}

fn some(value: Value) -> Value {
    let ty = optional(value["type"].clone());
    json!({"core": "some", "value": value, "type": ty, "aborts": []})
}

fn none(ty: Value) -> Value {
    json!({"core": "none", "type": ty, "aborts": []})
}

fn members(members: Vec<Value>) -> Value {
    let ty = tuple(members.iter().map(|it| it["type"].clone()).collect());
    json!({"core": "tuple", "members": members, "type": ty, "aborts": []})
}

fn member(of: Value, at: usize) -> Value {
    let ty = of["type"]["tuple"][at].clone();
    json!({"core": "member", "tuple": of, "at": at, "type": ty, "aborts": []})
}

/// A value `m` publishes, and the entry a host and another object read it through.
fn published(name: &str, body: Value) -> (Value, Value) {
    let ty = body["type"].clone();
    (
        json!({"module": "m", "name": name, "handovers": [], "body": body}),
        json!({
            "value": {"module": "m", "name": name},
            "body": {"core": "call", "reaches": {"is": "value", "module": "m", "name": name},
                     "arguments": [], "type": ty, "aborts": []}
        }),
    )
}

/// `m`, publishing:
///
/// - `bump`, adding five it captured;
/// - `overflow`, adding what leaves an `Int`'s range;
/// - `twice`, applying what it is handed twice;
/// - `pairing`, answering a tuple;
/// - `deep`, answering an optional of an optional, one of three ways;
/// - `meet`, taking a tuple holding an optional and answering the optional;
/// - `lifted`, answering a function that captured what it was handed.
fn publishing() -> Value {
    let unary = function(vec![int()], int());
    let values = vec![
        published(
            "bump",
            bound(
                0,
                number(5),
                block(
                    0,
                    &[(1, "x")],
                    added(read(1, int()), read(0, int())),
                    unary.clone(),
                ),
            ),
        ),
        published(
            "overflow",
            bound(
                2,
                number(i64::MAX),
                block(
                    1,
                    &[(3, "x")],
                    added(read(3, int()), read(2, int())),
                    unary.clone(),
                ),
            ),
        ),
        published(
            "twice",
            block(
                2,
                &[(4, "f"), (5, "x")],
                applied(
                    read(4, unary.clone()),
                    vec![applied(read(4, unary.clone()), vec![read(5, int())])],
                ),
                function(vec![unary.clone(), int()], int()),
            ),
        ),
        published(
            "pairing",
            bound(
                6,
                number(0),
                block(
                    3,
                    &[(7, "x")],
                    members(vec![read(7, int()), above(read(7, int()), read(6, int()))]),
                    function(vec![int()], tuple(vec![int(), truth()])),
                ),
            ),
        ),
        published(
            "deep",
            block(
                4,
                &[(8, "x")],
                chosen(
                    above(read(8, int()), number(0)),
                    some(some(read(8, int()))),
                    chosen(
                        above(read(8, int()), number(-1)),
                        some(none(optional(int()))),
                        none(optional(optional(int()))),
                    ),
                ),
                function(vec![int()], optional(optional(int()))),
            ),
        ),
        published(
            "meet",
            block(
                5,
                &[(9, "p")],
                member(read(9, tuple(vec![int(), optional(int())])), 1),
                function(vec![tuple(vec![int(), optional(int())])], optional(int())),
            ),
        ),
        published(
            "lifted",
            block(
                6,
                &[(10, "n")],
                block(
                    7,
                    &[(11, "x")],
                    added(read(11, int()), read(10, int())),
                    unary.clone(),
                ),
                function(vec![int()], unary.clone()),
            ),
        ),
    ];
    let (values, entries): (Vec<Value>, Vec<Value>) = values.into_iter().unzip();
    json!({
        "transport": TRANSPORT_VERSION, "declarations": [], "behaviors": [],
        "modules": [{"name": "m", "publishes": [], "helpers": [], "values": values,
                     "entries": entries, "definitions": [], "examples": []}]
    })
}

/// `r`, whose `g` applies `m.bump`, which another build publishes, to what it is handed.
fn reaching() -> Value {
    let unary = function(vec![int()], int());
    let bump = json!({"core": "call",
                      "reaches": {"is": "publishedvalue", "module": "m", "name": "bump"},
                      "arguments": [], "type": unary, "aborts": []});
    json!({
        "transport": TRANSPORT_VERSION, "declarations": [],
        "behaviors": [{"module": "r", "name": "g", "is": "body",
                       "parameters": {"named": [{"name": "a", "input": {"is": "scalar", "scalar": "INT"}}]},
                       "output": {"is": "scalar", "scalar": "INT"}, "ensures": {"at": "none"},
                       "requirements": []}],
        "modules": [{"name": "r", "publishes": [], "helpers": [], "values": [], "entries": [],
                     "definitions": [{"is": "body", "declared": "r.g", "parameters": ["a"],
                                      "publication": "published",
                                      "body": applied(bump, vec![read(0, int())])}],
                     "examples": []}]
    })
}

/// The document the Java half's tests read, held to what [`publishing`] builds.
const PUBLISHING: &str = include_str!("functions.transport.json");

#[test]
fn the_document_the_java_half_reads_is_the_one_built_here() {
    let written: Value = serde_json::from_str(PUBLISHING).unwrap();
    assert_eq!(written, publishing());
}

/// What a host does with what `m` publishes, through the header and nothing else.
const HOSTING: &str = r#"
#include <inttypes.h>
#include <stdio.h>
#include "souther.h"

static souther_status times(void *by, int64_t x, int64_t *out) {
    *out = x * *(const int64_t *) by;
    return SOUTHER_ANSWERED;
}

static souther_status thrown(void *by, int64_t x, int64_t *out) {
    return SOUTHER_HOST_EXCEPTION;
}

static souther_status aborted(void *by, int64_t x, int64_t *out) {
    return SOUTHER_DIVISION_BY_ZERO;
}

int main(void) {
    int64_t mark = souther_mark();

    souther_function bump = NULL;
    souther_status status = souther5_m_m_v_bump(&bump);
    int64_t out = -1;
    souther_status called = souther5_m_m_fn_f1_int_int_call(bump, 3, &out);
    printf("bump %u %u %" PRId64 "\n", status, called, out);

    souther_function overflow = NULL;
    souther5_m_m_v_overflow(&overflow);
    out = -1;
    called = souther5_m_m_fn_f1_int_int_call(overflow, 1, &out);
    printf("overflow %d %" PRId64 "\n", called == SOUTHER_REQUIRED_FORM_HAS_NO_PLACE, out);

    souther_function twice = NULL;
    souther5_m_m_v_twice(&twice);
    called = souther5_m_m_fn_f2_f1_int_int_int_int_call(twice, bump, 1, &out);
    printf("twice bump %u %" PRId64 "\n", called, out);

    int64_t three = 3;
    souther_hosted_function tripling;
    souther_function tripled = souther5_m_m_fn_f1_int_int_implement(&tripling, times, &three);
    called = souther5_m_m_fn_f2_f1_int_int_int_int_call(twice, tripled, 2, &out);
    printf("twice hosted %u %" PRId64 "\n", called, out);
    called = souther5_m_m_fn_f1_int_int_call(tripled, 5, &out);
    printf("hosted %u %" PRId64 "\n", called, out);

    souther_hosted_function throwing;
    out = -1;
    called = souther5_m_m_fn_f2_f1_int_int_int_int_call(
            twice, souther5_m_m_fn_f1_int_int_implement(&throwing, thrown, NULL), 2, &out);
    printf("thrown %d %" PRId64 "\n", called == SOUTHER_HOST_EXCEPTION, out);
    souther_hosted_function aborting;
    called = souther5_m_m_fn_f2_f1_int_int_int_int_call(
            twice, souther5_m_m_fn_f1_int_int_implement(&aborting, aborted, NULL), 2, &out);
    printf("aborted %d\n", called == SOUTHER_INJECTION_PROTOCOL_VIOLATION);
    souther_hosted_function empty;
    called = souther5_m_m_fn_f2_f1_int_int_int_int_call(
            twice, souther5_m_m_fn_f1_int_int_implement(&empty, NULL, NULL), 2, &out);
    printf("unbound %d\n", called == SOUTHER_INJECTION_UNBOUND);

    souther_function pairing = NULL;
    souther5_m_m_v_pairing(&pairing);
    int64_t first = 0;
    uint8_t second = 9;
    called = souther5_m_m_fn_f1_int_t2_int_bool_call(pairing, -4, &first, &second);
    printf("pairing %u %" PRId64 " %u", called, first, second);
    souther5_m_m_fn_f1_int_t2_int_bool_call(pairing, 4, &first, &second);
    printf(" %" PRId64 " %u\n", first, second);

    souther_function deep = NULL;
    souther5_m_m_v_deep(&deep);
    for (int64_t x = 1; x >= -1; x--) {
        uint8_t outer = 9, inner = 9;
        int64_t held = -9;
        called = souther5_m_m_fn_f1_int_o_o_int_call(deep, x, &outer, &inner, &held);
        printf("deep %" PRId64 ": %u %u %u %" PRId64 "\n", x, called, outer, inner, held);
    }

    souther_function meet = NULL;
    souther5_m_m_v_meet(&meet);
    uint8_t there = 9;
    int64_t met = -9;
    souther5_m_m_fn_f1_t2_int_o_int_o_int_call(meet, 4, 1, 7, &there, &met);
    printf("meet %u %" PRId64, there, met);
    there = 9;
    met = -9;
    souther5_m_m_fn_f1_t2_int_o_int_o_int_call(meet, 4, 0, 99, &there, &met);
    printf(" %u %" PRId64 "\n", there, met);

    souther_function lifted = NULL;
    souther5_m_m_v_lifted(&lifted);
    souther_function plus_ten = NULL;
    called = souther5_m_m_fn_f1_int_f1_int_int_call(lifted, 10, &plus_ten);
    souther5_m_m_fn_f1_int_int_call(plus_ten, 1, &out);
    printf("lifted %u %" PRId64 "\n", called, out);

    souther_reset(mark);
    return 0;
}
"#;

/// What a host calls `r.g` through.
const REACHING: &str = r#"
#include <inttypes.h>
#include <stdio.h>
#include "souther.h"

int main(void) {
    int64_t out = -1;
    souther_status status = souther5_m_r_b_g(NULL, 4, &out);
    printf("%u %" PRId64 "\n", status, out);
    return 0;
}
"#;

fn linking(builds: Vec<PathBuf>) -> Linking {
    Linking {
        builds,
        runtime: support::runtime().to_path_buf(),
    }
}

fn ran(document: &Value, builds: Vec<PathBuf>, program: &str, into: &Path) -> String {
    let built = library_for(&document.to_string(), &linking(builds), &into.join("built")).unwrap();
    let source = into.join("host.c");
    fs::write(&source, program).unwrap();
    let executable = into.join("host");
    let compiled = Command::new("cc")
        .args(["-Wall", "-Werror", "-o"])
        .arg(&executable)
        .arg(&source)
        .arg("-I")
        .arg(into.join("built"))
        .arg(&built.library)
        .arg(format!("-Wl,-rpath,{}", into.join("built").display()))
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

/// A host is handed a function value and calls it; hands one of its own over, which the object
/// calls; and is handed and hands over a tuple and an optional at any depth in what the function
/// takes and answers. A status other than an answer comes back as it was answered, and what a
/// host's function answers is held to what an implementation may answer, as a behavior's is. Room
/// for an optional's value is left as the host had it where the optional holds nothing.
#[test]
fn a_host_calls_a_function_value_and_hands_one_of_its_own_over() {
    let into = tempdir().unwrap();

    let said = ran(&publishing(), vec![], HOSTING, into.path());

    assert_eq!(
        said,
        "bump 0 0 8\n\
         overflow 1 -1\n\
         twice bump 0 11\n\
         twice hosted 0 18\n\
         hosted 0 15\n\
         thrown 1 -1\n\
         aborted 1\n\
         unbound 1\n\
         pairing 0 -4 0 4 1\n\
         deep 1: 0 1 1 1\n\
         deep 0: 0 1 0 -9\n\
         deep -1: 0 0 9 -9\n\
         meet 1 7 0 -9\n\
         lifted 0 11\n"
    );
}

/// A function value one object made is called by code another object wrote, through its header
/// alone: `r` reaches `m.bump` in `m`'s object and applies it.
#[test]
fn a_function_value_another_build_publishes_is_called_through_its_header() {
    let into = tempdir().unwrap();
    let object = into.path().join("m.o");
    fs::write(&object, object_for(&publishing().to_string()).unwrap()).unwrap();

    let said = ran(&reaching(), vec![object], REACHING, into.path());

    assert_eq!(said, "0 9\n");
}

/// What the manifest says of each function value's shape, and of each function a host reaches one
/// through.
#[test]
fn the_manifest_says_the_shape_each_function_value_crosses_in() {
    let into = tempdir().unwrap();
    let built = library_for(
        &publishing().to_string(),
        &linking(vec![]),
        &into.path().join("built"),
    )
    .unwrap();
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(&built.manifest).unwrap()).unwrap();
    let module = &manifest["modules"][0];

    let deep = module["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|it| it["name"] == "deep")
        .unwrap();
    assert_eq!(
        deep["read"]["available"]["signature"],
        json!({"takes": [], "answers": {"function": {
            "takes": [{"leaf": "int"}],
            "answers": {"option": {"option": {"leaf": "int"}}}
        }}})
    );
    let crossed: Vec<&str> = module["functions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| it["call"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        crossed,
        [
            "souther5_m_m_fn_f1_int_int_call",
            "souther5_m_m_fn_f2_f1_int_int_int_int_call",
            "souther5_m_m_fn_f1_int_t2_int_bool_call",
            "souther5_m_m_fn_f1_int_o_o_int_call",
            "souther5_m_m_fn_f1_t2_int_o_int_o_int_call",
            "souther5_m_m_fn_f1_int_f1_int_int_call",
        ]
    );
}
