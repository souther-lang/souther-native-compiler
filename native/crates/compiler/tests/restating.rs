//! A value standing as a wider type that holds what it is made of another way, run.
//!
//! An `Int` standing as a case of a union is carried with its token, so whatever holds one has to
//! be rebuilt to stand as what holds the union: an optional, a list, a tuple, and a function, which
//! is wrapped so that what it takes and what it answers are restated where it is called. The
//! checker writes each of these where a position is wider than the value put in it, so each is
//! written here as the checker would write it and run, and what comes back out of the wider value
//! is the `Int` that went in.
//!
//! A program reaching them is not written here, because the language keeps an anonymous union out
//! of any other type's position (E2307) and its lists and branches do not widen one another; so
//! the document is written by hand, which is what this side of the wire reads either way.

use serde_json::{Value, json};
use souther_native_driver::object_for;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

mod support;
use support::PREFIX;

fn int_ty() -> Value {
    json!({"prim": "INT"})
}

/// `Int | m.A`, which holds an `Int` carried.
fn union() -> Value {
    json!({"union": [{"is": "primitive", "prim": "INT"}, {"is": "declared", "declared": "m.A"}]})
}

fn option_of(ty: Value) -> Value {
    json!({"option": ty})
}

fn list_of(ty: Value) -> Value {
    json!({"list": ty})
}

fn fn_of(takes: Value, answers: Value) -> Value {
    json!({"fn": {"takes": [takes], "answers": answers}})
}

fn node(core: &str, mut fields: Value, ty: Value) -> Value {
    fields["core"] = json!(core);
    fields["type"] = ty;
    if fields.get("aborts").is_none() {
        fields["aborts"] = json!([]);
    }
    fields
}

fn int(value: i64) -> Value {
    node("int", json!({"value": value}), int_ty())
}

fn read(binding: usize, ty: Value) -> Value {
    node("read", json!({"binding": binding}), ty)
}

fn widen(value: Value, ty: Value) -> Value {
    node("widen", json!({"value": value}), ty)
}

fn let_(binding: usize, value: Value, body: Value) -> Value {
    let binds = value["type"].clone();
    let ty = body["type"].clone();
    node(
        "let",
        json!({"binding": binding, "binds": binds, "value": value, "body": body}),
        ty,
    )
}

fn add(left: Value, right: Value) -> Value {
    node(
        "binary",
        json!({"op": "ADD", "reading": {"is": "astheystand"}, "left": left, "right": right,
               "aborts": ["REQUIRED_FORM_HAS_NO_PLACE"]}),
        int_ty(),
    )
}

/// What an optional holding `value` is, or the one holding nothing where `value` is none.
fn some(value: Value) -> Value {
    let ty = option_of(value["type"].clone());
    node("some", json!({"value": value}), ty)
}

fn none(of: Value) -> Value {
    node("none", json!({}), option_of(of))
}

fn list(elements: Vec<Value>, of: Value) -> Value {
    node("list", json!({"elements": elements}), list_of(of))
}

/// The element at `at` of `xs`, a list of `of`, as `List.get` answers it.
fn get(at: i64, xs: Value, of: Value) -> Value {
    node(
        "call",
        json!({"reaches": {"is": "kernel", "kernel": "list.get",
                           "takes": [int_ty(), list_of(of.clone())], "fact": {"is": "none"}},
               "arguments": [int(at), xs]}),
        option_of(of),
    )
}

fn length(xs: Value, of: Value) -> Value {
    node(
        "call",
        json!({"reaches": {"is": "kernel", "kernel": "list.length",
                           "takes": [list_of(of)], "fact": {"is": "none"}},
               "arguments": [xs]}),
        int_ty(),
    )
}

fn block(site: usize, parameter: usize, takes: Value, body: Value) -> Value {
    let ty = fn_of(takes, body["type"].clone());
    node(
        "block",
        json!({"site": site, "parameters": [{"binding": parameter, "name": "x"}], "body": body}),
        ty,
    )
}

fn apply(function: Value, argument: Value) -> Value {
    let ty = function["type"]["fn"]["answers"].clone();
    node(
        "apply",
        json!({"function": function, "arguments": [argument]}),
        ty,
    )
}

fn arm(selects: Value, binding: Option<(usize, Value)>, body: Value) -> Value {
    let (binding, binds) = match binding {
        Some((number, ty)) => (json!(number), ty),
        None => (Value::Null, Value::Null),
    };
    json!({"selects": [selects], "binding": binding, "binds": binds, "body": body})
}

fn match_(subject: Value, arms: Vec<Value>) -> Value {
    node("match", json!({"subject": subject, "arms": arms}), int_ty())
}

/// What an optional holds, answered by `then` over the binding `binding`, or -1 where it holds
/// nothing.
fn held(optional: Value, binding: usize, then: Value) -> Value {
    let holds = optional["type"]["option"].clone();
    match_(
        optional,
        vec![
            arm(json!({"tests": "held"}), Some((binding, holds)), then),
            arm(json!({"tests": "nothing"}), None, int(-1)),
        ],
    )
}

/// The `Int` a value of the union carries, or -2 where it is the other case.
fn opened(value: Value, binding: usize) -> Value {
    match_(
        value,
        vec![
            arm(
                json!({"tests": "which", "atoms": [{"is": "primitive", "prim": "INT"}]}),
                Some((binding, int_ty())),
                read(binding, int_ty()),
            ),
            arm(
                json!({"tests": "which", "atoms": [{"is": "declared", "declared": "m.A"}]}),
                None,
                int(-2),
            ),
        ],
    )
}

/// Every behavior below takes `a: Int` as its binding 0 and answers an `Int`.
fn behaviors() -> Vec<(&'static str, Value)> {
    let a = || read(0, int_ty());
    let max = int(i64::MAX);
    vec![
        // An optional holding an `Int`, standing as one holding the union.
        (
            "optional",
            let_(
                1,
                widen(some(a()), option_of(union())),
                held(read(1, option_of(union())), 2, opened(read(2, union()), 3)),
            ),
        ),
        // One holding nothing stays one holding nothing.
        (
            "absent",
            held(widen(none(int_ty()), option_of(union())), 1, int(-3)),
        ),
        // A list of `Int`s, standing as a list of the union: as long, and each element carried.
        (
            "listed",
            let_(
                1,
                widen(list(vec![int(7), a(), int(9)], int_ty()), list_of(union())),
                add(
                    length(read(1, list_of(union())), union()),
                    held(
                        get(1, read(1, list_of(union())), union()),
                        2,
                        opened(read(2, union()), 3),
                    ),
                ),
            ),
        ),
        // An empty one.
        (
            "emptied",
            length(widen(list(vec![], int_ty()), list_of(union())), union()),
        ),
        // A list of optionals, each rebuilt around what it holds.
        (
            "nested",
            held(
                get(
                    0,
                    widen(
                        list(vec![some(a())], option_of(int_ty())),
                        list_of(option_of(union())),
                    ),
                    option_of(union()),
                ),
                1,
                held(read(1, option_of(union())), 2, opened(read(2, union()), 3)),
            ),
        ),
        // A tuple whose first member is carried and whose second stands where it is.
        (
            "tupled",
            let_(
                1,
                widen(
                    node(
                        "tuple",
                        json!({"members": [a(), int(9)]}),
                        json!({"tuple": [int_ty(), int_ty()]}),
                    ),
                    json!({"tuple": [union(), int_ty()]}),
                ),
                add(
                    opened(
                        node(
                            "member",
                            json!({"tuple": read(1, json!({"tuple": [union(), int_ty()]})), "at": 0}),
                            union(),
                        ),
                        2,
                    ),
                    node(
                        "member",
                        json!({"tuple": read(1, json!({"tuple": [union(), int_ty()]})), "at": 1}),
                        int_ty(),
                    ),
                ),
            ),
        ),
        // A function answering an `Int`, called as one answering the union.
        (
            "answering",
            opened(
                apply(
                    widen(
                        block(0, 1, int_ty(), read(1, int_ty())),
                        fn_of(int_ty(), union()),
                    ),
                    a(),
                ),
                2,
            ),
        ),
        // A function taking the union, called as one taking an `Int`.
        (
            "taking",
            apply(
                widen(
                    block(1, 1, union(), opened(read(1, union()), 2)),
                    fn_of(int_ty(), int_ty()),
                ),
                a(),
            ),
        ),
        // A function that ends the run, wrapped: what it ended with is what the call ends with.
        (
            "aborting",
            opened(
                apply(
                    widen(
                        block(2, 1, int_ty(), add(read(1, int_ty()), max)),
                        fn_of(int_ty(), union()),
                    ),
                    a(),
                ),
                2,
            ),
        ),
    ]
}

fn document() -> String {
    let behaviors = behaviors();
    let targets: Vec<Value> = behaviors
        .iter()
        .map(|(name, _)| {
            json!({"module": "m", "name": name, "is": "body",
                   "parameters": {"named": [{"name": "a", "input": {"is": "scalar", "scalar": "INT"}}]},
                   "output": {"is": "scalar", "scalar": "INT"},
                   "ensures": {"at": "none"}, "requirements": []})
        })
        .collect();
    let definitions: Vec<Value> = behaviors
        .into_iter()
        .map(|(name, body)| {
            json!({"is": "body", "declared": format!("m.{name}"), "parameters": ["a"],
                   "publication": "published", "body": body})
        })
        .collect();
    json!({
        "transport": 22,
        "declarations": [{"module": "m", "name": "A", "by": "amodule", "is": "unit"}],
        "behaviors": targets,
        "modules": [{"name": "m", "publishes": ["m.A"], "helpers": [], "values": [],
                     "entries": [], "definitions": definitions, "examples": []}]
    })
    .to_string()
}

const HARNESS: &str = r#"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>

#define BEHAVIOR(name) \
    extern uint32_t name(const void *, int64_t, int64_t *) __asm__("PREFIXsouther4.m." #name);
BEHAVIOR(optional)
BEHAVIOR(absent)
BEHAVIOR(listed)
BEHAVIOR(emptied)
BEHAVIOR(nested)
BEHAVIOR(tupled)
BEHAVIOR(answering)
BEHAVIOR(taking)
BEHAVIOR(aborting)
extern int64_t souther_mark(void);
extern void souther_reset(int64_t);

static void ran(const char *what, uint32_t (*behavior)(const void *, int64_t, int64_t *), int64_t a) {
    int64_t mark = souther_mark();
    int64_t answer = 0;
    uint32_t status = behavior(0, a, &answer);
    if (status == 0) {
        printf("%s %" PRId64 "\n", what, answer);
    } else {
        printf("%s ended\n", what);
    }
    souther_reset(mark);
}

int main(void) {
    ran("optional", optional, 5);
    ran("absent", absent, 5);
    ran("listed", listed, 5);
    ran("emptied", emptied, 5);
    ran("nested", nested, 5);
    ran("tupled", tupled, 5);
    ran("answering", answering, 5);
    ran("taking", taking, 5);
    ran("aborting", aborting, 1);
    return 0;
}
"#;

#[test]
fn a_value_rebuilt_to_stand_wider_answers_what_went_in() {
    let into = tempdir().unwrap();
    let object = into.path().join("m.o");
    fs::write(&object, object_for(&document()).unwrap()).unwrap();
    let harness = into.path().join("harness.c");
    fs::write(&harness, HARNESS.replace("PREFIX", PREFIX)).unwrap();
    let executable = into.path().join("restating");
    let linked = Command::new("cc")
        .arg("-o")
        .arg(&executable)
        .arg(&harness)
        .arg(&object)
        .arg(support::runtime())
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let run = Command::new(&executable).output().unwrap();

    assert_eq!(
        String::from_utf8(run.stdout).unwrap(),
        concat!(
            "optional 5\n",
            "absent -1\n",
            "listed 8\n",
            "emptied 0\n",
            "nested 5\n",
            "tupled 14\n",
            "answering 5\n",
            "taking 5\n",
            "aborting ended\n",
        )
    );
}
