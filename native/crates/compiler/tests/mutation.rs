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
//!
//! A panic is the loud way a relation goes unheld. The quiet one is a document that reads, and
//! lowers to something other than what it says: two readers of one rule (what a name stands for
//! where it is read) that agree on every document the writer writes and part on one it does not.
//! What is asked of such a document is that it means what its binders say and not what their
//! numbers happen to be: renamed, binder by binder, into numbers nothing else uses, it must lower
//! to the same object. The renaming below is the one place in this file that knows what a scope is,
//! and it is written to the language's rule and not to any reader's.

use serde_json::{Value, json};
use souther_native_driver::{NotLowered, object_for};
use std::collections::{BTreeMap, BTreeSet};
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
        "transport": 15,
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
        "else": int(2),
        "type": prim(INT), "aborts": []
    });
    // A quotient answers a `Rational`, which nothing here lays out, so it stands in a document of
    // its own: read, and refused as not lowered, which is a different thing for a change to hit.
    let dividing = program(
        json!([]),
        json!([helper(
            "m.quotient",
            &[prim(INT), prim(INT)],
            binary(
                "DIV",
                read(0, prim(INT)),
                read(1, prim(INT)),
                prim("RATIONAL"),
                json!(["DIVISION_BY_ZERO"])
            )
        )]),
        json!([]),
    );
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

    // A `let` whose scope closes before a parameter is read, and a fork whose arm binds a value
    // that the arm beside it does not: what a binder's number would mean if it were the number of
    // something in force, and the reads that come after it.
    let scoped = json!({
        "core": "binary", "op": "ADD", "reading": { "is": "astheystand" },
        "left": {
            "core": "let", "binding": 1, "binds": prim(INT),
            "value": binary("ADD", read(0, prim(INT)), int(1), prim(INT),
                            json!(["REQUIRED_FORM_HAS_NO_PLACE"])),
            "body": binary("MUL", read(1, prim(INT)), int(2), prim(INT),
                           json!(["REQUIRED_FORM_HAS_NO_PLACE"])),
            "type": prim(INT), "aborts": []
        },
        "right": read(0, prim(INT)),
        "type": prim(INT), "aborts": ["REQUIRED_FORM_HAS_NO_PLACE"]
    });
    let scoping = program(
        json!([]),
        json!([helper("m.scoped", &[prim(INT)], scoped)]),
        json!([]),
    );
    let beside = json!({
        "core": "match", "subject": read(1, json!({ "option": prim(INT) })),
        "arms": [
            { "selects": [{ "tests": "held" }], "binding": 2, "binds": prim(INT),
              "body": read(2, prim(INT)) },
            { "selects": [{ "tests": "nothing" }], "binding": null, "binds": null,
              "body": read(0, prim(INT)) }
        ],
        "type": prim(INT), "aborts": []
    });
    let arms = program(
        json!([]),
        json!([helper(
            "m.beside",
            &[prim(INT), json!({ "option": prim(INT) })],
            beside
        )]),
        json!([]),
    );

    vec![
        ("binders whose scopes close before a read", scoping),
        ("an arm that binds beside one that does not", arms),
        ("a construction that owes a clause", constructing),
        ("a kernel call", kernel),
        ("a fork on an optional", optional),
        ("arithmetic of every kind", arithmetic),
        ("a quotient", dividing),
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
        ("ensures", read_json(include_str!("ensures.transport.json"))),
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
    let meaning_the_numbers: Vec<&String> = outcomes
        .iter()
        .filter_map(|it| match it {
            Outcome::DependsOnNumbers(said) => Some(said),
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
        meaning_the_numbers.is_empty(),
        "{} of {tried} changed documents that read lower to something else once their binders \
         are renamed, e.g.:\n{}",
        meaning_the_numbers.len(),
        meaning_the_numbers
            .iter()
            .take(4)
            .map(|it| it.chars().take(900).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
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
    /// The changed document was read, and lowers to the same object when its binders are renamed.
    Read,
    /// The changed document was read and lowers to something else once its binders are renamed:
    /// what it lowers to depends on the numbers and not on what they name.
    DependsOnNumbers(String),
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
        Ok(Ok(object)) => match by_numbers(name, &mutant, &object) {
            Some(said) => Outcome::DependsOnNumbers(said),
            None => Outcome::Read,
        },
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

/// Every binder of every body renamed to a number nothing else uses, each read following the binder
/// that is in force where it stands: a `let` for its body, a match arm for its body, a function
/// value's parameter for its body, a field for the clauses of its declaration, the answer for the
/// rule that reads it. What is a parameter of a helper, a value, a behavior or a rule over a
/// behavior's answer stands as the position it is handed at and is not renamed.
///
/// Written to the rule and not to a reader: a name is what the nearest enclosing binder of that
/// number says it is, and is that binder's alone for as long as its scope lasts.
fn rename_binders(document: &mut Value, mode: Mode) {
    let mut fresh = 1_000_000_000_000_u64;
    // A declaration's fields are what its clauses read.
    if let Some(declarations) = document
        .get_mut("declarations")
        .and_then(Value::as_array_mut)
    {
        for declaration in declarations {
            let mut names: BTreeMap<u64, u64> = BTreeMap::new();
            let mut fields: Vec<&mut Value> = Vec::new();
            let is_product = declaration.get("is").and_then(Value::as_str) == Some("product");
            if is_product
                && let Some(all) = declaration.get_mut("fields").and_then(Value::as_array_mut)
            {
                fields.extend(all.iter_mut());
            }
            for field in fields {
                if let Some(old) = field.get("binding").and_then(Value::as_u64) {
                    fresh += 1;
                    bind(&mut names, old, fresh);
                    field["binding"] = Value::from(fresh);
                }
            }
            if let Some(field) = declaration.get_mut("field")
                && let Some(old) = field.get("binding").and_then(Value::as_u64)
            {
                fresh += 1;
                bind(&mut names, old, fresh);
                field["binding"] = Value::from(fresh);
            }
            if let Some(clauses) = declaration
                .get_mut("invariants")
                .and_then(Value::as_array_mut)
            {
                for clause in clauses {
                    if let Some(condition) = clause.get_mut("condition") {
                        rename(condition, &mut names.clone(), &mut fresh, mode, 0);
                    }
                }
            }
        }
    }
    // A rule a behavior's answer is held to reads the parameters where each stands, and the answer
    // under a number of its own for the whole of the rule.
    if let Some(behaviors) = document.get_mut("behaviors").and_then(Value::as_array_mut) {
        for target in behaviors {
            let Some(contract) = target
                .get_mut("ensures")
                .and_then(|it| it.get_mut("contract"))
            else {
                continue;
            };
            let handed = contract
                .get("parameters")
                .and_then(Value::as_array)
                .map_or(0, Vec::len) as u64;
            if let Some(rules) = contract.get_mut("rules").and_then(Value::as_array_mut) {
                for rule in rules {
                    let mut names: BTreeMap<u64, u64> = BTreeMap::new();
                    if let Some(old) = rule.get("value").and_then(Value::as_u64) {
                        let new = number(
                            mode,
                            &names,
                            handed,
                            rule.get("condition"),
                            Some(old),
                            &mut fresh,
                        );
                        bind(&mut names, old, new);
                        rule["value"] = Value::from(new);
                    }
                    if let Some(condition) = rule.get_mut("condition") {
                        rename(condition, &mut names, &mut fresh, mode, handed);
                    }
                }
            }
        }
    }
    if let Some(modules) = document.get_mut("modules").and_then(Value::as_array_mut) {
        for module in modules {
            for place in ["helpers", "values", "entries", "definitions", "examples"] {
                if let Some(bodies) = module.get_mut(place).and_then(Value::as_array_mut) {
                    for body in bodies {
                        // What is handed to a body is numbered by where it stands and is in force
                        // for all of it: a helper's or a behavior's parameters and a value's
                        // handovers.
                        let handed = ["parameters", "handovers"]
                            .iter()
                            .find_map(|key| body.get(*key).and_then(Value::as_array))
                            .map_or(0, Vec::len) as u64;
                        if let Some(node) = body.get_mut("body") {
                            rename(node, &mut BTreeMap::new(), &mut fresh, mode, handed);
                        }
                    }
                }
            }
        }
    }
}

fn rename(
    node: &mut Value,
    names: &mut BTreeMap<u64, u64>,
    fresh: &mut u64,
    mode: Mode,
    handed: u64,
) {
    match node {
        Value::Array(items) => {
            for item in items {
                rename(item, names, fresh, mode, handed);
            }
        }
        Value::Object(fields) => match fields.get("core").and_then(Value::as_str) {
            Some("read") => {
                if let Some(old) = fields.get("binding").and_then(Value::as_u64)
                    && let Some(new) = names.get(&old)
                {
                    fields.insert("binding".to_string(), Value::from(*new));
                }
            }
            Some("let") => {
                if let Some(value) = fields.get_mut("value") {
                    rename(value, names, fresh, mode, handed);
                }
                let old = fields.get("binding").and_then(Value::as_u64);
                let new = number(mode, names, handed, fields.get("body"), old, fresh);
                let before = old.and_then(|old| bind(names, old, new));
                fields.insert("binding".to_string(), Value::from(new));
                if let Some(body) = fields.get_mut("body") {
                    rename(body, names, fresh, mode, handed);
                }
                leave(names, old, before);
            }
            Some("match") => {
                if let Some(subject) = fields.get_mut("subject") {
                    rename(subject, names, fresh, mode, handed);
                }
                if let Some(arms) = fields.get_mut("arms").and_then(Value::as_array_mut) {
                    for arm in arms {
                        let old = arm.get("binding").and_then(Value::as_u64);
                        let mut before = None;
                        if let Some(old) = old {
                            let new =
                                number(mode, names, handed, arm.get("body"), Some(old), fresh);
                            before = bind(names, old, new);
                            arm["binding"] = Value::from(new);
                        }
                        if let Some(body) = arm.get_mut("body") {
                            rename(body, names, fresh, mode, handed);
                        }
                        leave(names, old, before);
                    }
                }
            }
            Some("block") => {
                let mut entered: Vec<(Option<u64>, Option<u64>)> = Vec::new();
                if let Some(parameters) = fields.get_mut("parameters").and_then(Value::as_array_mut)
                {
                    for parameter in parameters {
                        let old = parameter.get("binding").and_then(Value::as_u64);
                        *fresh += 1;
                        let before = old.and_then(|old| bind(names, old, *fresh));
                        parameter["binding"] = Value::from(*fresh);
                        entered.push((old, before));
                    }
                }
                if let Some(body) = fields.get_mut("body") {
                    rename(body, names, fresh, mode, handed);
                }
                for (old, before) in entered.into_iter().rev() {
                    leave(names, old, before);
                }
            }
            _ => {
                for (_, inner) in fields.iter_mut() {
                    rename(inner, names, fresh, mode, handed);
                }
            }
        },
        _ => {}
    }
}

/// How a binder is renamed.
#[derive(Clone, Copy)]
enum Mode {
    /// To a number nothing else uses.
    Fresh,
    /// To the smallest number already in force that its scope does not read, so that it shadows what
    /// that number stood for without changing what any read in its scope says. A document renamed
    /// this way means what it meant before, in the language's scope rule; whether a driver reads it
    /// at all is its own answer, and one that does has to lower it as it lowered the original.
    Collapse,
}

/// The number a binder is renamed to, for a scope `body`: fresh, or in `Collapse` mode one in force
/// that the scope does not read.
fn number(
    mode: Mode,
    names: &BTreeMap<u64, u64>,
    handed: u64,
    body: Option<&Value>,
    old: Option<u64>,
    fresh: &mut u64,
) -> u64 {
    if let (Mode::Collapse, Some(body), Some(old)) = (mode, body, old) {
        let mut read = BTreeSet::new();
        free_reads(body, &mut vec![old], &mut read);
        let read: BTreeSet<u64> = read
            .into_iter()
            .map(|it| names.get(&it).copied().unwrap_or(it))
            .collect();
        let mut in_force: BTreeSet<u64> = (0..handed).collect();
        in_force.extend(names.values().copied());
        if let Some(target) = in_force.into_iter().find(|it| !read.contains(it)) {
            return target;
        }
    }
    *fresh += 1;
    *fresh
}

/// Every number `node` reads that no binder inside it binds.
fn free_reads(node: &Value, bound: &mut Vec<u64>, into: &mut BTreeSet<u64>) {
    match node {
        Value::Array(items) => items.iter().for_each(|it| free_reads(it, bound, into)),
        Value::Object(fields) => match fields.get("core").and_then(Value::as_str) {
            Some("read") => {
                if let Some(number) = fields.get("binding").and_then(Value::as_u64)
                    && !bound.contains(&number)
                {
                    into.insert(number);
                }
            }
            Some("let") => {
                if let Some(value) = fields.get("value") {
                    free_reads(value, bound, into);
                }
                let own = fields.get("binding").and_then(Value::as_u64);
                bound.extend(own);
                if let Some(body) = fields.get("body") {
                    free_reads(body, bound, into);
                }
                if own.is_some() {
                    bound.pop();
                }
            }
            Some("match") => {
                if let Some(subject) = fields.get("subject") {
                    free_reads(subject, bound, into);
                }
                for arm in fields
                    .get("arms")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let own = arm.get("binding").and_then(Value::as_u64);
                    bound.extend(own);
                    if let Some(body) = arm.get("body") {
                        free_reads(body, bound, into);
                    }
                    if own.is_some() {
                        bound.pop();
                    }
                }
            }
            Some("block") => {
                let own: Vec<u64> = fields
                    .get("parameters")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|it| it.get("binding").and_then(Value::as_u64))
                    .collect();
                bound.extend(own.iter().copied());
                if let Some(body) = fields.get("body") {
                    free_reads(body, bound, into);
                }
                bound.truncate(bound.len() - own.len());
            }
            _ => fields.values().for_each(|it| free_reads(it, bound, into)),
        },
        _ => {}
    }
}

/// `old` standing for `new` from here, answering what it stood for before.
///
/// Replacing what a number stood for is the point of this function: the renaming follows the
/// language's rule, where a binder shadows an enclosing one of its number for as long as its scope
/// lasts. That is why it is written here, and once.
#[expect(
    clippy::disallowed_methods,
    reason = "the language's own rule: an inner binder shadows an enclosing one of its number"
)]
fn bind(names: &mut BTreeMap<u64, u64>, old: u64, new: u64) -> Option<u64> {
    names.insert(old, new)
}

/// `old` back to what it stood for before its binder, or out of force where it stood for nothing.
fn leave(names: &mut BTreeMap<u64, u64>, old: Option<u64>, before: Option<u64>) {
    let Some(old) = old else { return };
    match before {
        Some(before) => {
            bind(names, old, before);
        }
        None => {
            names.remove(&old);
        }
    }
}

/// Whether what a document lowers to depends on the numbers of its binders and not on what they
/// name, answered by renaming them both ways: to numbers nothing else uses, which a driver has to
/// read and lower alike, and onto numbers already in force, which a driver may refuse and, if it
/// reads, has to lower alike. `None` is that it does not.
fn by_numbers(name: &str, document: &Value, object: &[u8]) -> Option<String> {
    for (mode, kind, must_read) in [
        (Mode::Fresh, "renamed to numbers nothing else uses", true),
        (
            Mode::Collapse,
            "renamed onto numbers already in force",
            false,
        ),
    ] {
        let mut renamed = document.clone();
        rename_binders(&mut renamed, mode);
        if renamed == *document {
            continue;
        }
        match catch_unwind(AssertUnwindSafe(|| object_for(&renamed.to_string()))) {
            Ok(Ok(again)) if again == object => {}
            Ok(Ok(_)) => {
                return Some(format!(
                    "{name}: lowers to something else {kind}\n  {document}\n  {renamed}"
                ));
            }
            Ok(Err(refused)) if must_read => {
                return Some(format!(
                    "{name}: is read, and refused {kind} ({refused})\n  {document}\n  {renamed}"
                ));
            }
            Ok(Err(_)) => {}
            Err(_) => {
                return Some(format!(
                    "{name}: is read, and {kind} makes the driver panic\n  {document}\n  {renamed}"
                ));
            }
        }
    }
    None
}

/// What a document means is its binders' and not their numbers, for every document held here as
/// the writer wrote it, and for the ones written by hand for what the writer's do not hold.
#[test]
fn a_document_means_what_its_binders_say_and_not_what_their_numbers_are() {
    let mut checked = 0;
    for (name, document) in fixtures() {
        // What this backend does not lower yet has no object to compare.
        let object = match object_for(&document.to_string()) {
            Ok(object) => object,
            Err(refused) if refused.downcast_ref::<NotLowered>().is_some() => continue,
            Err(refused) => panic!("{name} is read: {refused}"),
        };
        if let Some(said) = by_numbers(name, &document, &object) {
            panic!("{said}");
        }
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} documents were held to this");
}
