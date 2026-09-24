//! A document that reads is not one a field of which, changed, can make the driver panic.
//!
//! `Coherent` holds every relation between two statements the document makes, and the lowering
//! after it trusts that with `expect`, `unreachable!` and indexing. So a relation `Coherent` does
//! not hold shows up as a panic in the lowering the day a document breaks it, and never before: the
//! documents tests write are ones the checker could have written. This writes the other ones.
//!
//! Each field of each document below is replaced, one at a time, by another value found under the
//! same key in any of them, and by the extremes of a number; an element of a list is dropped and
//! repeated, a key is left out, a truth is turned round. The driver may refuse every one of these.
//! What it may not do is panic, and a panic names the field that was changed and what it was
//! changed to, which is a relation nothing was holding.
//!
//! Not a check that what is accepted is right: a different value for a field is very often a
//! different program the checker would have written. That is held where each relation is, in
//! `coherence.rs`; this finds the ones nobody thought to.

use serde_json::{Value, json};
use souther_native_driver::object_for;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// How many mutants are tried in all. The documents have far more sites than this, and the ones
/// tried are spread evenly over them, so what is missed by a smaller number is missed everywhere at
/// once and not in one document.
const BUDGET: usize = 4000;

const INT: &str = "INT";

fn prim(name: &str) -> Value {
    json!({ "prim": name })
}

fn read(binding: u64, ty: Value) -> Value {
    json!({ "core": "read", "binding": binding, "type": ty, "aborts": [] })
}

fn int(value: i64) -> Value {
    json!({ "core": "int", "value": value, "type": prim(INT), "aborts": [] })
}

fn binary(op: &str, left: Value, right: Value, ty: Value, aborts: Value) -> Value {
    json!({
        "core": "binary", "op": op, "reading": { "is": "astheystand" },
        "left": left, "right": right, "type": ty, "aborts": aborts
    })
}

fn program(declarations: Value, helpers: Value, publishes: Value) -> Value {
    json!({
        "transport": 13,
        "declarations": declarations,
        "behaviors": [],
        "modules": [{
            "name": "m", "publishes": publishes, "helpers": helpers,
            "values": [], "entries": [], "definitions": [], "examples": []
        }]
    })
}

fn helper(name: &str, takes: &[Value], body: Value) -> Value {
    let parameters: Vec<Value> = takes
        .iter()
        .enumerate()
        .map(|(at, ty)| json!({ "name": format!("p{at}"), "type": ty }))
        .collect();
    json!({ "declared": name, "parameters": parameters, "body": body })
}

/// What no fixture the writer produced holds: a type that states a clause and is built, a kernel
/// call, a fork on an optional, arithmetic of each kind, and an operator read in a type.
fn by_hand() -> Vec<(&'static str, Value)> {
    let amount = json!({ "declared": "m.R" });
    let clause = binary("GE", read(0, prim(INT)), int(0), prim("BOOL"), json!([]));
    let declarations = json!([{
        "module": "m", "name": "R", "by": "amodule", "is": "product",
        "fields": [{ "name": "count", "binding": 0,
                     "codec": { "is": "scalar", "scalar": INT } }],
        "invariants": [{ "name": "counted", "condition": clause }]
    }]);
    let built = json!({
        "core": "construct", "declared": "m.R", "values": [read(0, prim(INT))],
        "type": amount, "aborts": ["INVARIANT_NOT_HELD"]
    });
    let constructing = program(
        declarations.clone(),
        json!([helper("m.make", &[prim(INT)], built)]),
        json!(["m.R"]),
    );

    let added = json!({
        "core": "call",
        "reaches": { "is": "kernel", "kernel": "int.add",
                     "takes": [prim(INT), prim(INT)], "fact": { "is": "none" } },
        "arguments": [read(0, prim(INT)), read(1, prim(INT))],
        "type": prim(INT), "aborts": ["REQUIRED_FORM_HAS_NO_PLACE"]
    });
    let kernel = program(
        json!([]),
        json!([helper("m.add", &[prim(INT), prim(INT)], added)]),
        json!([]),
    );

    let negated = json!({
        "core": "neg", "operand": read(1, prim(INT)), "type": prim(INT),
        "aborts": ["REQUIRED_FORM_HAS_NO_PLACE"]
    });
    let forking = json!({
        "core": "match", "subject": read(0, json!({ "option": prim(INT) })),
        "arms": [
            { "selects": [{ "tests": "held" }], "binding": 1, "binds": prim(INT),
              "body": negated },
            { "selects": [{ "tests": "nothing" }], "binding": null, "binds": null,
              "body": int(0) }
        ],
        "type": prim(INT), "aborts": []
    });
    let optional = program(
        json!([]),
        json!([helper("m.fork", &[json!({ "option": prim(INT) })], forking)]),
        json!([]),
    );

    let arithmetic = binary(
        "MUL",
        binary(
            "SUB",
            binary(
                "ADD",
                read(0, prim(INT)),
                int(1),
                prim(INT),
                json!(["REQUIRED_FORM_HAS_NO_PLACE"]),
            ),
            read(1, prim(INT)),
            prim(INT),
            json!(["REQUIRED_FORM_HAS_NO_PLACE"]),
        ),
        int(2),
        prim(INT),
        json!(["REQUIRED_FORM_HAS_NO_PLACE"]),
    );
    let compared = json!({
        "core": "if",
        "cond": binary("LE", arithmetic, int(3), prim("BOOL"), json!([])),
        "then": int(1),
        "else": binary(
            "DIV", int(1), int(2), prim("RATIONAL"), json!(["DIVISION_BY_ZERO"])
        ),
        "type": prim(INT), "aborts": []
    });
    let arithmetic = program(
        json!([]),
        json!([helper("m.calc", &[prim(INT), prim(INT)], compared)]),
        json!([]),
    );

    let in_a_type = json!({
        "core": "binary", "op": "EQ",
        "reading": { "is": "in", "type": amount },
        "left": read(0, amount.clone()), "right": read(1, amount.clone()),
        "type": prim("BOOL"), "aborts": []
    });
    let reading = program(
        declarations,
        json!([helper("m.same", &[amount.clone(), amount], in_a_type)]),
        json!(["m.R"]),
    );

    vec![
        ("a construction that owes a clause", constructing),
        ("a kernel call", kernel),
        ("a fork on an optional", optional),
        ("arithmetic of every kind", arithmetic),
        ("an operator read in a type", reading),
    ]
}

fn fixtures() -> Vec<(&'static str, Value)> {
    let read_json = |text: &str| serde_json::from_str::<Value>(text).expect("a fixture is JSON");
    let mut documents = vec![
        ("adding", read_json(include_str!("adding.transport.json"))),
        (
            "closures",
            read_json(include_str!("closures.transport.json")),
        ),
        (
            "composing",
            read_json(include_str!("composing.transport.json")),
        ),
        (
            "published_value",
            read_json(include_str!("published_value.transport.json")),
        ),
        ("values", read_json(include_str!("values.transport.json"))),
    ];
    documents.extend(by_hand());
    documents
}

/// One step down into a document.
#[derive(Clone, Debug)]
enum Step {
    Key(String),
    At(usize),
}

/// Where a value stands, and the name its kind of place goes by: the key it is under, or the key
/// its list is under with brackets for an element of it.
struct Site {
    path: Vec<Step>,
    place: String,
    value: Value,
}

fn sites(value: &Value, path: &mut Vec<Step>, place: &str, into: &mut Vec<Site>) {
    into.push(Site {
        path: path.clone(),
        place: place.to_string(),
        value: value.clone(),
    });
    match value {
        Value::Object(fields) => {
            for (key, inner) in fields {
                path.push(Step::Key(key.clone()));
                sites(inner, path, key, into);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (at, inner) in items.iter().enumerate() {
                path.push(Step::At(at));
                sites(inner, path, &format!("{place}[]"), into);
                path.pop();
            }
        }
        _ => {}
    }
}

fn set(root: &mut Value, path: &[Step], with: Option<Value>) {
    let Some((step, rest)) = path.split_first() else {
        *root = with.expect("the root is replaced, never removed");
        return;
    };
    if rest.is_empty() {
        match (step, root) {
            (Step::Key(key), Value::Object(fields)) => match with {
                Some(value) => {
                    fields.insert(key.clone(), value);
                }
                None => {
                    fields.remove(key);
                }
            },
            (Step::At(at), Value::Array(items)) => match with {
                Some(value) => items[*at] = value,
                None => {
                    items.remove(*at);
                }
            },
            _ => unreachable!("a site's path leads to it"),
        }
        return;
    }
    let inner = match (step, root) {
        (Step::Key(key), Value::Object(fields)) => fields.get_mut(key),
        (Step::At(at), Value::Array(items)) => items.get_mut(*at),
        _ => None,
    }
    .expect("a site's path leads to it");
    set(inner, rest, with);
}

/// Every change tried at one site: another value seen in the same kind of place, the extremes of a
/// number, a truth turned round, a list element dropped or repeated, a key left out.
fn changes(site: &Site, pools: &BTreeMap<String, Vec<Value>>, removable: bool) -> Vec<Change> {
    let mut out = Vec::new();
    for other in pools.get(&site.place).into_iter().flatten() {
        if *other != site.value {
            out.push(Change::To(other.clone()));
        }
    }
    match &site.value {
        Value::Number(number) => {
            let at = number.as_i64().unwrap_or(0);
            for extreme in [0, -1, at.wrapping_add(1), i64::MIN, i64::MAX] {
                if extreme != at {
                    out.push(Change::To(Value::from(extreme)));
                }
            }
        }
        Value::Bool(truth) => out.push(Change::To(Value::Bool(!truth))),
        Value::Array(items) if !items.is_empty() => {
            let mut repeated = items.clone();
            repeated.push(items[0].clone());
            out.push(Change::To(Value::Array(repeated)));
            out.push(Change::To(Value::Array(Vec::new())));
        }
        _ => {}
    }
    if removable {
        out.push(Change::Remove);
    }
    out
}

#[derive(Debug)]
enum Change {
    To(Value),
    Remove,
}

#[test]
fn no_field_of_a_document_that_reads_can_be_changed_into_a_panic() {
    let documents = fixtures();

    // What each kind of place is seen to hold, across every document.
    let mut pools: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut all: Vec<(usize, Site)> = Vec::new();
    for (at, (_, document)) in documents.iter().enumerate() {
        let mut found = Vec::new();
        sites(document, &mut Vec::new(), "", &mut found);
        for site in found {
            let pool = pools.entry(site.place.clone()).or_default();
            if !pool.contains(&site.value) && pool.len() < 10 && !site.place.is_empty() {
                pool.push(site.value.clone());
            }
            all.push((at, site));
        }
    }

    // Every change at every site, spread evenly over the budget.
    let mut tries: Vec<(usize, usize, Change)> = Vec::new();
    for (at, (index, site)) in all.iter().enumerate() {
        if site.path.is_empty() {
            continue;
        }
        let removable = matches!(site.path.last(), Some(Step::Key(_)));
        for change in changes(site, &pools, removable) {
            tries.push((*index, at, change));
        }
    }
    let stride = tries.len().div_ceil(BUDGET).max(1);

    // Independent of one another, so they are shared out over the machine: what a change costs is
    // mostly the code generation for the ones that read.
    let tries: Vec<(usize, usize, Change)> = tries.into_iter().step_by(stride).collect();
    let workers = std::thread::available_parallelism().map_or(1, |it| it.get());
    let share = tries.len().div_ceil(workers).max(1);

    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcomes: Vec<Outcome> = std::thread::scope(|scope| {
        let handles: Vec<_> = tries
            .chunks(share)
            .map(|chunk| {
                let (documents, all) = (&documents, &all);
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|(index, at, change)| try_change(documents, all, *index, *at, change))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|it| {
                it.join()
                    .expect("a change is tried, never left to panic here")
            })
            .collect()
    });
    std::panic::set_hook(quiet);

    let tried = outcomes.len();
    let accepted = outcomes
        .iter()
        .filter(|it| matches!(it, Outcome::Read))
        .count();
    let panicked: Vec<&String> = outcomes
        .iter()
        .filter_map(|it| match it {
            Outcome::Panicked(said) => Some(said),
            _ => None,
        })
        .collect();

    let refused = outcomes
        .iter()
        .filter(|it| matches!(it, Outcome::Refused))
        .count();
    assert!(tried > 1000, "only {tried} changes were tried");
    assert!(
        refused > accepted,
        "{refused} changed documents were refused and {accepted} read: the changes are meant to \
         break relations, and most do"
    );
    assert!(
        accepted > 0,
        "no changed document read, so nothing reached the lowering"
    );
    assert!(
        panicked.is_empty(),
        "{} of {tried} changed documents made the driver panic, e.g.:\n{}",
        panicked.len(),
        panicked
            .iter()
            .take(12)
            .map(|it| it.chars().take(400).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

enum Outcome {
    /// The changed document was read.
    Read,
    /// It was refused, which is what nearly all of them are.
    Refused,
    /// It made the driver panic, and this says which field was changed to what.
    Panicked(String),
}

fn try_change(
    documents: &[(&'static str, Value)],
    all: &[(usize, Site)],
    index: usize,
    at: usize,
    change: &Change,
) -> Outcome {
    let (name, document) = &documents[index];
    let site = &all[at].1;
    let mut mutant = document.clone();
    match change {
        Change::To(value) => set(&mut mutant, &site.path, Some(value.clone())),
        Change::Remove => set(&mut mutant, &site.path, None),
    }
    let text = mutant.to_string();
    match catch_unwind(AssertUnwindSafe(|| object_for(&text))) {
        Ok(Ok(_)) => Outcome::Read,
        Ok(Err(_)) => Outcome::Refused,
        Err(cause) => {
            let said = cause
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| cause.downcast_ref::<&str>().map(|it| it.to_string()))
                .unwrap_or_default();
            let mut steps = String::new();
            for step in &site.path {
                match step {
                    Step::Key(key) => steps.push_str(&format!(".{key}")),
                    Step::At(at) => steps.push_str(&format!("[{at}]")),
                }
            }
            let to = match change {
                Change::To(value) => value.to_string(),
                Change::Remove => "(left out)".to_string(),
            };
            Outcome::Panicked(format!("{name}{steps} -> {to}: {said}"))
        }
    }
}
