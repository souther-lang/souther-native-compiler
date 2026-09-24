//! Every relation the lowering reads a type across, and a document that breaks each one.
//!
//! The lowering takes a node's own type for the type of the value it makes of that node, and the
//! value is made from something else the document states: a binder, a callee, a declaration, a
//! member. Each test here writes a document the checker could have written, which is read whole,
//! and the same document with one of those two statements changed, which is refused as the two
//! halves disagreeing and not as something this backend is behind on.

use souther_native_driver::{NotLowered, object_for};

const INT: &str = r#"{"prim":"INT"}"#;
const BOOL: &str = r#"{"prim":"BOOL"}"#;
const STRING: &str = r#"{"prim":"STRING"}"#;
const DECIMAL: &str = r#"{"prim":"DECIMAL"}"#;
const A: &str = r#"{"declared":"m.A"}"#;
const S: &str = r#"{"declared":"m.S"}"#;
const P: &str = r#"{"declared":"m.P"}"#;

/// The units `m.A` and `m.B`, the sum `m.S = m.A | m.B`, and the product `m.P` with one field `f`
/// of `m.S`; the behaviors, helpers and local definitions given; one module `m`.
fn document(behaviors: &[String], helpers: &[String], definitions: &[String]) -> String {
    format!(
        concat!(
            r#"{{"transport":11,"declarations":["#,
            r#"{{"module":"m","name":"A","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"B","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"S","by":"amodule","is":"sum","#,
            r#""cases":[{{"is":"declared","declared":"m.A"}},{{"is":"declared","declared":"m.B"}}],"#,
            r#""form":{{"is":"enumeration"}}}},"#,
            r#"{{"module":"m","name":"P","by":"amodule","is":"product","#,
            r#""fields":[{{"name":"f","codec":{{"is":"named","declared":"m.S"}}}}],"invariants":0}}],"#,
            r#""behaviors":[{}],"#,
            r#""modules":[{{"name":"m","helpers":[{}],"values":[],"entries":[],"definitions":[{}],"#,
            r#""examples":[]}}]}}"#
        ),
        behaviors.join(","),
        helpers.join(","),
        definitions.join(",")
    )
}

/// The helper `m.h`, taking the types given and answering with `body`.
fn h(takes: &[&str], body: &str) -> String {
    helper("m.h", takes, body)
}

fn helper(declared: &str, takes: &[&str], body: &str) -> String {
    let parameters: Vec<String> = takes
        .iter()
        .enumerate()
        .map(|(at, ty)| format!(r#"{{"name":"p{at}","type":{ty}}}"#))
        .collect();
    format!(
        r#"{{"declared":"{declared}","parameters":[{}],"body":{body}}}"#,
        parameters.join(",")
    )
}

fn helpers(helpers: &[String]) -> String {
    document(&[], helpers, &[])
}

fn node(core: &str, fields: &str, ty: &str) -> String {
    format!(r#"{{"core":"{core}",{fields},"type":{ty},"aborts":[]}}"#)
}

/// `value` standing as `ty`, as the checker says it wherever a value stands as a type other than its
/// own.
fn widen(value: &str, ty: &str) -> String {
    node("widen", &format!(r#""value":{value}"#), ty)
}

fn read(binding: usize, ty: &str) -> String {
    node("read", &format!(r#""binding":{binding}"#), ty)
}

fn int(value: i64) -> String {
    node("int", &format!(r#""value":{value}"#), INT)
}

fn truth(value: bool) -> String {
    node("bool", &format!(r#""value":{value}"#), BOOL)
}

fn unit(declared: &str) -> String {
    node(
        "unit",
        &format!(r#""declared":"{declared}""#),
        &format!(r#"{{"declared":"{declared}"}}"#),
    )
}

fn let_(binding: usize, binds: &str, value: &str, body: &str, ty: &str) -> String {
    node(
        "let",
        &format!(r#""binding":{binding},"binds":{binds},"value":{value},"body":{body}"#),
        ty,
    )
}

fn call(reaches: &str, arguments: &[String], ty: &str) -> String {
    node(
        "call",
        &format!(
            r#""reaches":{reaches},"arguments":[{}]"#,
            arguments.join(",")
        ),
        ty,
    )
}

fn tuple_of(members: &[&str]) -> String {
    format!(r#"{{"tuple":[{}]}}"#, members.join(","))
}

fn option_of(ty: &str) -> String {
    format!(r#"{{"option":{ty}}}"#)
}

fn fn_of(takes: &[&str], answers: &str) -> String {
    format!(
        r#"{{"fn":{{"takes":[{}],"answers":{answers}}}}}"#,
        takes.join(",")
    )
}

fn which(atoms: &[&str]) -> String {
    let atoms: Vec<String> = atoms
        .iter()
        .map(|it| format!(r#"{{"is":"declared","declared":"{it}"}}"#))
        .collect();
    format!(r#"{{"tests":"which","atoms":[{}]}}"#, atoms.join(","))
}

fn arm(selects: &str, binding: Option<(usize, &str)>, body: &str) -> String {
    let (binding, binds) = match binding {
        Some((number, ty)) => (number.to_string(), ty.to_string()),
        None => ("null".to_string(), "null".to_string()),
    };
    format!(r#"{{"selects":[{selects}],"binding":{binding},"binds":{binds},"body":{body}}}"#)
}

fn reads_whole(document: &str) {
    if let Err(refused) = object_for(document) {
        panic!("a document the checker could have written is refused: {refused}");
    }
}

fn is_the_halves_disagreeing(document: &str, naming: &str) {
    let refused = object_for(document).expect_err("a document the checker could not have written");
    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains(naming), "{refused}");
}

/// A read's type is its binder's: an `Int` parameter read as a `Bool` would hand an eight-byte
/// value to whatever reads one byte, and the helper's answer, which is its body's type, would say
/// `Bool` over it.
#[test]
fn a_read_is_typed_as_its_binder_binds() {
    reads_whole(&helpers(&[h(&[INT], &read(0, INT))]));
    is_the_halves_disagreeing(&helpers(&[h(&[INT], &read(0, BOOL))]), "m.h");
}

/// The same with a number read as text. One machine word each, so only the types tell them apart.
#[test]
fn a_read_typed_as_another_type_of_the_same_width_is_still_the_halves_disagreeing() {
    is_the_halves_disagreeing(&helpers(&[h(&[INT], &read(0, STRING))]), "m.h");
}

/// A binder this backend has no representation for, read as one it has, is still two statements
/// of one type that disagree, and is refused as that before anything asks for the `Decimal`'s
/// layout.
#[test]
fn a_read_disagreeing_with_a_binder_that_has_no_layout_is_the_halves_disagreeing() {
    is_the_halves_disagreeing(&helpers(&[h(&[DECIMAL], &read(0, INT))]), "m.h");
}

/// A behavior's parameters are bound at what its target takes, and its body read one of them as
/// something else: the body's root would pass as the target's answer while being an `Int`.
#[test]
fn a_behaviors_read_is_typed_as_its_target_takes() {
    let target = |answers: &str| {
        format!(
            r#"{{"module":"m","name":"b","is":"body","inputs":[{{"is":"scalar","scalar":"INT"}}],"output":{{"is":"scalar","scalar":"{answers}"}}}}"#
        )
    };
    let body = |read: String| {
        format!(
            r#"{{"is":"body","declared":"m.b","parameters":["a"],"publication":"kept","body":{read}}}"#
        )
    };
    reads_whole(&document(&[target("INT")], &[], &[body(read(0, INT))]));
    is_the_halves_disagreeing(
        &document(&[target("BOOL")], &[], &[body(read(0, BOOL))]),
        "m.b",
    );
}

/// A let binds its name wider than its value where the checker says so, and every read of the name
/// is typed as it was bound, not as the value was. Read at the value's narrower type, it is two
/// statements of one binding that disagree, however consistent the value and the read are with
/// each other.
#[test]
fn a_read_of_a_let_is_typed_as_the_let_binds_it() {
    reads_whole(&helpers(&[h(
        &[],
        &let_(0, S, &widen(&unit("m.A"), S), &read(0, S), S),
    )]));
    is_the_halves_disagreeing(
        &helpers(&[h(&[], &let_(0, S, &widen(&unit("m.A"), S), &read(0, A), A))]),
        "m.h",
    );
}

/// What a let is given is a value of what it binds.
#[test]
fn a_let_given_what_it_does_not_bind_is_the_halves_disagreeing() {
    is_the_halves_disagreeing(
        &helpers(&[h(&[], &let_(0, A, &unit("m.B"), &read(0, A), A))]),
        "m.h",
    );
}

/// A call stands at what the helper it reaches answers. Called as an `Int`, a helper answering a
/// `Bool` writes one byte where its caller reads eight: the mistake the answer's own signature
/// once made, made at the call instead.
#[test]
fn a_call_of_a_helper_stands_at_what_the_helper_answers() {
    let g = helper("m.g", &[], &truth(true));
    let reaches = r#"{"is":"helper","declared":"m.g"}"#;
    reads_whole(&helpers(&[g.clone(), h(&[], &call(reaches, &[], BOOL))]));
    is_the_halves_disagreeing(&helpers(&[g, h(&[], &call(reaches, &[], INT))]), "m.g");
}

/// A call of a behavior stands at what its target answers.
#[test]
fn a_call_of_a_behavior_stands_at_what_its_target_answers() {
    let target = r#"{"module":"m","name":"b","is":"body","inputs":[],"output":{"is":"scalar","scalar":"INT"}}"#.to_string();
    let body = format!(
        r#"{{"is":"body","declared":"m.b","parameters":[],"publication":"kept","body":{}}}"#,
        int(1)
    );
    let reaches = r#"{"is":"behavior","declared":"m.b"}"#;
    reads_whole(&document(
        std::slice::from_ref(&target),
        &[h(&[], &call(reaches, &[], INT))],
        std::slice::from_ref(&body),
    ));
    is_the_halves_disagreeing(
        &document(&[target], &[h(&[], &call(reaches, &[], BOOL))], &[body]),
        "m.b",
    );
}

/// What a call hands over is what the callee takes.
#[test]
fn an_argument_is_a_value_of_what_the_callee_takes() {
    let g = helper("m.g", &[S], &read(0, S));
    let reaches = r#"{"is":"helper","declared":"m.g"}"#;
    reads_whole(&helpers(&[
        g.clone(),
        h(&[], &call(reaches, &[widen(&unit("m.A"), S)], S)),
    ]));
    is_the_halves_disagreeing(&helpers(&[g, h(&[], &call(reaches, &[int(1)], S))]), "m.g");
}

/// A tuple is typed as what its members are.
#[test]
fn a_tuple_is_typed_as_its_members() {
    let tuple = |ty: &str| node("tuple", &format!(r#""members":[{}]"#, int(1)), ty);
    reads_whole(&helpers(&[h(&[], &tuple(&tuple_of(&[INT])))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &tuple(&tuple_of(&[BOOL])))]), "m.h");
}

/// A member is typed as the tuple says it is. It is read out of a slot at the width its own type
/// gives, so a member typed otherwise is read at the wrong width.
#[test]
fn a_member_is_typed_as_the_tuple_holds_it() {
    let pair = tuple_of(&[INT, BOOL]);
    let tuple = node(
        "tuple",
        &format!(r#""members":[{},{}]"#, int(1), truth(true)),
        &pair,
    );
    let member = |ty: &str| node("member", &format!(r#""tuple":{tuple},"at":1"#), ty);
    reads_whole(&helpers(&[h(&[], &member(BOOL))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &member(INT))]), "m.h");
}

/// A field is read as the type its declaration gives it. A field of `m.S` read as its case `m.A`
/// would let what follows treat an `m.B` as an `m.A`.
#[test]
fn a_field_is_read_as_its_declaration_types_it() {
    let built = node(
        "construct",
        &format!(r#""declared":"m.P","values":[{}]"#, widen(&unit("m.A"), S)),
        P,
    );
    let field = |ty: &str| node("field", &format!(r#""target":{built},"field":"f""#), ty);
    reads_whole(&helpers(&[h(&[], &field(S))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &field(A))]), "m.P");
}

/// A construction is typed as what it builds.
#[test]
fn a_construction_is_typed_as_what_it_builds() {
    let built = |ty: &str| {
        node(
            "construct",
            &format!(r#""declared":"m.P","values":[{}]"#, widen(&unit("m.A"), S)),
            ty,
        )
    };
    reads_whole(&helpers(&[h(&[], &built(P))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &built(A))]), "m.P");
}

/// Each branch of a fork answers what the fork answers: the fork's type, or a case of it standing as
/// that type.
#[test]
fn a_forks_branches_answer_values_of_what_it_answers() {
    let fork = |els: &str, ty: &str| {
        node(
            "if",
            &format!(
                r#""cond":{},"then":{},"else":{els}"#,
                truth(true),
                widen(&unit("m.A"), ty)
            ),
            ty,
        )
    };
    reads_whole(&helpers(&[h(&[], &fork(&widen(&unit("m.B"), S), S))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &fork(&unit("m.B"), A))]), "m.h");
}

/// An arm reads what it forks on as what it binds, and every case it tests has to be one of those:
/// an arm testing `m.A | m.B` and binding `m.A` would read an `m.B` as an `m.A`.
#[test]
fn an_arm_binds_every_case_it_tests() {
    let fork = |tests: &[&str]| {
        node(
            "match",
            &format!(
                r#""subject":{},"arms":[{}]"#,
                read(0, S),
                arm(&which(tests), Some((1, A)), &read(1, A))
            ),
            A,
        )
    };
    reads_whole(&helpers(&[h(&[S], &fork(&["m.A"]))]));
    is_the_halves_disagreeing(&helpers(&[h(&[S], &fork(&["m.A", "m.B"]))]), "m.h");
}

/// An optional's present value is read as what the optional holds.
#[test]
fn an_arm_reads_a_present_value_as_what_the_optional_holds() {
    let optional = option_of(S);
    let fork = |binds: &str| {
        node(
            "match",
            &format!(
                r#""subject":{},"arms":[{},{}]"#,
                read(0, &optional),
                arm(r#"{"tests":"held"}"#, Some((1, binds)), &read(1, binds)),
                arm(r#"{"tests":"nothing"}"#, None, &widen(&unit("m.A"), S))
            ),
            S,
        )
    };
    reads_whole(&helpers(&[h(&[&optional], &fork(S))]));
    is_the_halves_disagreeing(&helpers(&[h(&[&optional], &fork(A))]), "m.h");
}

/// What a present value holds is what the optional holds.
#[test]
fn a_present_value_holds_a_value_of_what_its_optional_holds() {
    let some = |value: &str| node("some", &format!(r#""value":{value}"#), &option_of(S));
    reads_whole(&helpers(&[h(&[], &some(&widen(&unit("m.A"), S)))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &some(&int(1)))]), "m.h");
}

/// A comparison answers a truth, and what the lowering makes of one is a truth.
#[test]
fn a_comparison_answers_a_truth() {
    let compare = |ty: &str| {
        node(
            "binary",
            &format!(r#""op":"EQ","left":{},"right":{}"#, int(1), int(2)),
            ty,
        )
    };
    reads_whole(&helpers(&[h(&[], &compare(BOOL))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &compare(INT))]), "m.h");
}

/// A negation is typed as what it negates.
#[test]
fn a_negation_is_typed_as_what_it_negates() {
    let negated = node("neg", &format!(r#""operand":{}"#, read(0, INT)), BOOL);
    is_the_halves_disagreeing(&helpers(&[h(&[INT], &negated)]), "negation");
}

/// `int.add` takes two numbers and answers one.
#[test]
fn int_add_answers_a_number() {
    let reaches = r#"{"is":"kernel","kernel":"int.add"}"#;
    let added = |ty: &str| {
        {
            node(
                "call",
                &format!(r#""reaches":{reaches},"arguments":[{},{}]"#, int(1), int(2)),
                ty,
            )
        }
        .replace(
            r#""aborts":[]}"#,
            r#""aborts":["REQUIRED_FORM_HAS_NO_PLACE"]}"#,
        )
    };
    reads_whole(&helpers(&[h(&[], &added(INT))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &added(BOOL))]), "int.add");
}

/// A closure reads what it captures as the binder outside it binds it. The capture is laid out
/// by what the read says, and restored by it, so a read typed otherwise carries the value out at
/// the wrong width.
#[test]
fn a_capture_is_read_as_its_binder_outside_binds_it() {
    let block = |ty: &str| {
        node(
            "block",
            &format!(r#""site":0,"parameters":[],"body":{}"#, read(0, ty)),
            &fn_of(&[], ty),
        )
    };
    reads_whole(&helpers(&[h(&[INT], &block(INT))]));
    is_the_halves_disagreeing(&helpers(&[h(&[INT], &block(BOOL))]), "m.h");
}

/// What a function value is applied to is what it takes.
#[test]
fn a_function_value_is_applied_to_values_of_what_it_takes() {
    let function = fn_of(&[S], S);
    let applied = |argument: String| {
        node(
            "apply",
            &format!(
                r#""function":{},"arguments":[{argument}]"#,
                read(0, &function)
            ),
            S,
        )
    };
    reads_whole(&helpers(&[h(
        &[&function],
        &applied(widen(&unit("m.A"), S)),
    )]));
    is_the_halves_disagreeing(&helpers(&[h(&[&function], &applied(int(1)))]), "m.h");
}

/// A call of a value stands at what the value answers, which is its body's type.
#[test]
fn a_call_of_a_value_stands_at_what_the_value_answers() {
    let good = include_str!("values.transport.json");
    let reaching = r#""reaches":{"is":"value","module":"m","name":"ks"},"arguments":[],"type":{"declared":"m.P"}"#;
    assert!(good.contains(reaching), "the fixture this perturbs moved");
    let bad = good.replace(
        reaching,
        r#""reaches":{"is":"value","module":"m","name":"ks"},"arguments":[],"type":{"prim":"INT"}"#,
    );
    is_the_halves_disagreeing(&bad, "m.ks");
}

/// A call of another module's published value stands at what that value's entry answers, where
/// the document carries the entry.
#[test]
fn a_call_of_a_published_value_stands_at_what_its_entry_answers() {
    let good = include_str!("published_value.transport.json");
    let reaching = r#""reaches":{"is":"publishedvalue","module":"publisher","name":"ys"},"arguments":[],"type":{"declared":"publisher.Box"}"#;
    assert!(good.contains(reaching), "the fixture this perturbs moved");
    let bad = good.replace(
        reaching,
        r#""reaches":{"is":"publishedvalue","module":"publisher","name":"ys"},"arguments":[],"type":{"prim":"INT"}"#,
    );
    is_the_halves_disagreeing(&bad, "ys");
}

/// `m.inner` answers an `Int` and `m.flag` takes a `Bool`; `m.outer` runs one after the other.
fn two_stages(flag_takes: &str, outer_answers: &str) -> String {
    let target = |name: &str, is: &str, takes: &str, answers: &str| {
        format!(
            r#"{{"module":"m","name":"{name}","is":"{is}","inputs":[{{"is":"scalar","scalar":"{takes}"}}],"output":{{"is":"scalar","scalar":"{answers}"}}}}"#
        )
    };
    let body = |name: &str, ty: &str| {
        format!(
            r#"{{"is":"body","declared":"m.{name}","parameters":["a"],"publication":"kept","body":{}}}"#,
            read(0, ty)
        )
    };
    let flag_answers = flag_takes;
    let flag_ty = if flag_takes == "INT" { INT } else { BOOL };
    let outer = r#"{"is":"composed","declared":"m.outer","publication":"published","stages":[{"behavior":"m.inner","routing":{"is":"always"}},{"behavior":"m.flag","routing":{"is":"always"}}]}"#;
    document(
        &[
            target("inner", "body", "INT", "INT"),
            target("flag", "body", flag_takes, flag_answers),
            target("outer", "composed", "INT", outer_answers),
        ],
        &[],
        &[body("inner", INT), body("flag", flag_ty), outer.to_string()],
    )
}

/// A stage after the first is handed what the stage before answered.
#[test]
fn a_stage_is_handed_what_the_stage_before_answered() {
    reads_whole(&two_stages("INT", "INT"));
    is_the_halves_disagreeing(&two_stages("BOOL", "BOOL"), "m.flag");
}

/// What the last stage answers is what the composition answers.
#[test]
fn what_a_compositions_last_stage_answers_is_what_it_answers() {
    is_the_halves_disagreeing(&two_stages("INT", "BOOL"), "m.outer");
}

/// A document whose halves disagree in one body, and which this backend is behind on in another,
/// is refused as disagreeing, whichever of the two bodies is read first. `m.g` answers whether one
/// list is a list of a wider type, which nothing here has a rule for.
#[test]
fn a_disagreement_anywhere_is_refused_before_anything_is_not_lowered() {
    let listed = |of: &str| format!(r#"{{"list":{of}}}"#);
    let g = helper(
        "m.g",
        &[&listed(A)],
        &let_(
            1,
            &listed(S),
            &widen(&read(0, &listed(A)), &listed(S)),
            &read(1, &listed(S)),
            &listed(S),
        ),
    );
    let refused =
        object_for(&helpers(std::slice::from_ref(&g))).expect_err("nothing lays a list out");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    is_the_halves_disagreeing(&helpers(&[g, h(&[INT], &read(0, BOOL))]), "m.h");
}

/// What transport 10 stopped carrying is not read if a document carries it: each was a second
/// statement of a fact the document states elsewhere, and a document still writing one is a
/// writer this driver does not read.
#[test]
fn a_second_statement_transport_10_dropped_is_not_read() {
    let good = helpers(&[h(&[INT], &read(0, INT))]);
    reads_whole(&good);
    for (said, again) in [
        (
            r#""body":{"core":"read""#,
            r#""answers":{"prim":"INT"},"body":{"core":"read""#,
        ),
        (
            r#""parameters":[{"name":"p0","type":{"prim":"INT"}}]"#,
            r#""parameters":[{"name":"p0","type":{"prim":"INT"}}],"takes":[{"prim":"INT"}]"#,
        ),
    ] {
        assert!(good.contains(said), "the document this perturbs moved");
        let refused = object_for(&good.replace(said, again)).expect_err("a field no longer read");
        assert!(refused.to_string().contains("unknown field"), "{refused}");
    }
}

/// The same document with `m`'s rows replaced by those given.
fn with_rows(document: &str, rows: &[String]) -> String {
    let none = r#""examples":[]"#;
    assert_eq!(
        document.matches(none).count(),
        1,
        "one module to put rows in"
    );
    document.replace(none, &format!(r#""examples":[{}]"#, rows.join(",")))
}

fn row(behavior: &str, at: usize, body: &str) -> String {
    format!(r#"{{"behavior":"{behavior}","at":{at},"body":{body}}}"#)
}

/// `m.b`, taking nothing and answering an `Int` its body makes.
fn b() -> (String, String) {
    (
        r#"{"module":"m","name":"b","is":"body","inputs":[],"output":{"is":"scalar","scalar":"INT"}}"#
            .to_string(),
        format!(
            r#"{{"is":"body","declared":"m.b","parameters":[],"publication":"kept","body":{}}}"#,
            int(1)
        ),
    )
}

/// A helper with no layout here, which on its own is refused as not lowered.
fn behind() -> String {
    helper("m.behind", &[DECIMAL], &read(0, DECIMAL))
}

#[test]
fn a_helper_this_backend_is_behind_on_is_on_its_own_not_lowered() {
    let refused = object_for(&helpers(&[behind()])).expect_err("no layout for a Decimal");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
}

/// Two helpers one module holds under one name: whichever was read last would be checked, and
/// whichever was declared first compiled. Refused as the name written twice, and not as the first
/// copy's `Decimal`, which a backend reading the document in another order would never have met.
#[test]
fn a_helper_written_twice_is_refused_before_either_is_lowered() {
    let first = helper("m.g", &[DECIMAL], &read(0, DECIMAL));
    let second = helper("m.g", &[INT], &read(0, INT));
    is_the_halves_disagreeing(&helpers(&[first, second]), "m.g");
}

/// A value, an entry and a module written twice are the same mistake, refused the same way.
#[test]
fn a_value_an_entry_or_a_module_written_twice_is_the_halves_disagreeing() {
    let values = include_str!("values.transport.json");
    let twice = |from: &str, to: &str| {
        let at = values.find(from).expect("the fixture this perturbs moved");
        let end = at
            + values[at..]
                .find(to)
                .expect("the fixture this perturbs moved");
        let once = &values[at..end];
        values.replacen(once, &format!("{once},{once}"), 1)
    };
    is_the_halves_disagreeing(
        &twice(
            r#"{"module":"m","name":"ks""#,
            r#",{"module":"m","name":"ys""#,
        ),
        "m.ks",
    );
    is_the_halves_disagreeing(
        &twice(
            r#"{"value":{"module":"m","name":"ys"}"#,
            r#"],"definitions""#,
        ),
        "m.ys",
    );
    let module = |helpers: &str| {
        format!(
            r#"{{"name":"m","helpers":[{helpers}],"values":[],"entries":[],"definitions":[],"examples":[]}}"#
        )
    };
    let document = helpers(&[]).replace(
        &module(""),
        &format!("{},{}", module(&behind()), module("")),
    );
    is_the_halves_disagreeing(&document, "two modules");
}

/// Two rows of one behavior at one place.
#[test]
fn a_row_written_twice_is_the_halves_disagreeing() {
    let (target, body) = b();
    let document = document(&[target], &[behind()], &[body]);
    let rows = [row("b", 0, &int(1)), row("b", 0, &int(2))];
    is_the_halves_disagreeing(&with_rows(&document, &rows), "m.b");
}

/// A target saying a name is defined here, with no local definition under the name, is refused
/// as that, and not as the target's `Decimal` having no layout.
#[test]
fn a_target_defined_here_with_nothing_defining_it_is_refused_before_its_signature_is_asked() {
    let target = r#"{"module":"m","name":"b","is":"body","inputs":[],"output":{"is":"scalar","scalar":"DECIMAL"}}"#;
    is_the_halves_disagreeing(&document(&[target.to_string()], &[], &[]), "m.b");
}

/// Two calls of another build's published value, at two types: one declaration answers one way.
#[test]
fn another_builds_value_called_at_two_types_is_refused_before_anything_is_lowered() {
    let reaches = r#"{"is":"publishedvalue","module":"other","name":"v"}"#;
    let calls = |second: &str| {
        helpers(&[
            behind(),
            helper("m.g", &[], &call(reaches, &[], INT)),
            h(&[], &call(reaches, &[], second)),
        ])
    };
    reads_whole(&calls(INT).replace(&format!("{},", behind()), ""));
    is_the_halves_disagreeing(&calls(BOOL), "`other`'s published value v");
}

/// A quotient is a `Rational` whatever it divides, so a `/` typed as anything else is refused as
/// that, before the `Decimal` elsewhere is found to have no layout.
#[test]
fn a_quotient_is_a_rational() {
    let divided = |ty: &str| {
        node(
            "binary",
            &format!(r#""op":"DIV","left":{},"right":{}"#, int(1), int(2)),
            ty,
        )
    };
    is_the_halves_disagreeing(&helpers(&[behind(), h(&[], &divided(INT))]), "m.h");
    is_the_halves_disagreeing(&helpers(&[behind(), h(&[], &divided(BOOL))]), "m.h");
}

/// Two numbers of two types: `Int + Rational` is one the checker writes and `Int + Decimal` is one
/// it refuses. Neither has a lowering here, and which of the two a pair is would take the checker's
/// decision, which the checked tree does not record (souther-lang/souther#1919); so both are not
/// lowered, and neither is refused as something the checker could not have written.
#[test]
fn numbers_of_two_types_are_not_told_apart_until_the_checker_says() {
    let rational = r#"{"prim":"RATIONAL"}"#;
    for other in [DECIMAL, rational] {
        let added = node(
            "binary",
            &format!(
                r#""op":"ADD","left":{},"right":{}"#,
                read(0, INT),
                read(1, other)
            ),
            rational,
        );
        let refused = object_for(&helpers(&[h(&[INT, other], &added)])).expect_err("no lowering");
        assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    }
}

/// Arithmetic over two `Int`s can leave their range, and names the one reason it ends without a
/// value; naming none is the checker and this backend disagreeing about what kind of site it is.
#[test]
fn arithmetic_that_can_overflow_names_one_reason() {
    let added = |aborts: &str| {
        node(
            "binary",
            &format!(r#""op":"ADD","left":{},"right":{}"#, int(1), int(2)),
            INT,
        )
        .replace(r#""aborts":[]}"#, &format!(r#""aborts":[{aborts}]}}"#))
    };
    reads_whole(&helpers(&[h(
        &[],
        &added(r#""REQUIRED_FORM_HAS_NO_PLACE""#),
    )]));
    is_the_halves_disagreeing(&helpers(&[behind(), h(&[], &added(""))]), "reasons");
}

/// A row is a body like any other: a published value it calls is declared, and a closure it
/// builds is planned, the same as in a behavior's body.
#[test]
fn a_rows_body_is_read_as_every_other_body_is() {
    let (target, body) = b();
    let calling = call(
        r#"{"is":"publishedvalue","module":"other","name":"v"}"#,
        &[],
        INT,
    );
    let block = node(
        "block",
        &format!(r#""site":0,"parameters":[],"body":{}"#, int(1)),
        &fn_of(&[], INT),
    );
    let applied = node(
        "apply",
        &format!(r#""function":{block},"arguments":[]"#),
        INT,
    );
    let document = document(&[target], &[], &[body]);
    reads_whole(&with_rows(
        &document,
        &[row("b", 0, &calling), row("b", 1, &applied)],
    ));
}

/// What a unit value names is a unit, and what a construction builds has fields.
#[test]
fn a_unit_or_a_construction_names_the_kind_of_declaration_it_makes() {
    let unit_of_a_product = node("unit", r#""declared":"m.P""#, P);
    is_the_halves_disagreeing(&helpers(&[h(&[], &unit_of_a_product)]), "m.P");
    let building_a_unit = node("construct", r#""declared":"m.A","values":[]"#, A);
    is_the_halves_disagreeing(&helpers(&[h(&[], &building_a_unit)]), "m.A");
}

/// An arm tests the leaves a case resolved to, and at least one of them.
#[test]
fn an_arm_tests_at_least_one_leaf() {
    let fork = |selects: &str| {
        node(
            "match",
            &format!(
                r#""subject":{},"arms":[{}]"#,
                read(0, S),
                arm(selects, None, &unit("m.A"))
            ),
            A,
        )
    };
    reads_whole(&helpers(&[h(&[S], &fork(&which(&["m.A", "m.B"])))]));
    is_the_halves_disagreeing(&helpers(&[h(&[S], &fork(&which(&["m.S"])))]), "m.S");
    is_the_halves_disagreeing(&helpers(&[h(&[S], &fork(&which(&[])))]), "no case");
    let testing_nothing = fork("");
    is_the_halves_disagreeing(&helpers(&[h(&[S], &testing_nothing)]), "nothing");
}

/// A sum's cases are the leaves it descends to, so a sum standing as one is a set of cases this
/// side would have to descend itself.
#[test]
fn a_sum_standing_as_a_case_of_a_sum_is_the_halves_disagreeing() {
    let document = helpers(&[]).replace(
        r#""cases":[{"is":"declared","declared":"m.A"},{"is":"declared","declared":"m.B"}]"#,
        r#""cases":[{"is":"declared","declared":"m.A"},{"is":"declared","declared":"m.S"}]"#,
    );
    is_the_halves_disagreeing(&document, "m.S");
}

/// `m`'s values and entries replaced by those given.
fn owning(document: &str, values: &[String], entries: &[String]) -> String {
    for none in [r#""values":[]"#, r#""entries":[]"#] {
        assert_eq!(
            document.matches(none).count(),
            1,
            "one module to put them in"
        );
    }
    document
        .replace(
            r#""values":[]"#,
            &format!(r#""values":[{}]"#, values.join(",")),
        )
        .replace(
            r#""entries":[]"#,
            &format!(r#""entries":[{}]"#, entries.join(",")),
        )
}

fn value(module: &str, name: &str) -> String {
    format!(
        r#"{{"module":"{module}","name":"{name}","handovers":[],"body":{}}}"#,
        int(1)
    )
}

fn entry(module: &str, name: &str) -> String {
    let reaches = format!(r#"{{"is":"value","module":"{module}","name":"{name}"}}"#);
    format!(
        r#"{{"value":{{"module":"{module}","name":"{name}"}},"body":{}}}"#,
        call(&reaches, &[], INT)
    )
}

/// A value runs in the module that declares it and in no other (ADR-0074), so `m` building a value
/// `other` declares would put a home in `m`'s object for what `m` does not own.
#[test]
fn a_module_builds_only_the_values_it_declares() {
    reads_whole(&owning(
        &helpers(&[behind()]).replace(&behind(), ""),
        &[value("m", "v")],
        &[],
    ));
    is_the_halves_disagreeing(
        &owning(&helpers(&[behind()]), &[value("other", "v")], &[]),
        "other.v",
    );
}

/// An entry is for a value its module builds. One for a value `m` does not build, whatever its
/// body answers, would export `m.v`'s symbol with nothing of `m`'s behind it.
#[test]
fn a_module_publishes_entries_only_for_the_values_it_builds() {
    let answering = format!(
        r#"{{"value":{{"module":"m","name":"v"}},"body":{}}}"#,
        int(1)
    );
    reads_whole(&owning(
        &helpers(&[]),
        &[value("m", "v")],
        &[entry("m", "v")],
    ));
    is_the_halves_disagreeing(&owning(&helpers(&[]), &[], &[answering]), "m.v");
}

/// A helper and a value are two kinds of definition, and a module holds none of one name as both.
/// Refused as that, before anything is declared in the object.
#[test]
fn a_module_holds_no_name_as_both_a_helper_and_a_value() {
    let document = owning(
        &helpers(&[behind(), helper("m.v", &[], &int(1))]),
        &[value("m", "v")],
        &[],
    );
    is_the_halves_disagreeing(&document, "m.v");
}

/// A module defines the behaviors it declares. `m` defining `other.b` would put `other`'s behavior
/// in `m`'s object.
#[test]
fn a_module_defines_only_the_behaviors_it_declares() {
    let target = r#"{"module":"other","name":"b","is":"body","inputs":[],"output":{"is":"scalar","scalar":"INT"}}"#;
    let body = format!(
        r#"{{"is":"body","declared":"other.b","parameters":[],"publication":"kept","body":{}}}"#,
        int(1)
    );
    is_the_halves_disagreeing(
        &document(&[target.to_string()], &[behind()], &[body]),
        "other.b",
    );
}

/// A behavior another build implements is one of a module this document does not build.
#[test]
fn a_behavior_implemented_elsewhere_is_of_a_module_this_document_does_not_build() {
    let target = |module: &str| {
        format!(
            r#"{{"module":"{module}","name":"b","is":"elsewhere","inputs":[],"output":{{"is":"scalar","scalar":"INT"}}}}"#
        )
    };
    reads_whole(&document(&[target("other")], &[], &[]));
    is_the_halves_disagreeing(&document(&[target("m")], &[behind()], &[]), "m.b");
}

/// What the language itself declares is a set of alternatives or a single value.
#[test]
fn the_language_declares_nothing_built_from_fields() {
    let document = helpers(&[behind()]).replace(
        r#"{"module":"m","name":"P","by":"amodule","is":"product""#,
        r#"{"module":"m","name":"P","by":"thelanguage","is":"product""#,
    );
    is_the_halves_disagreeing(&document, "m.P");
}

/// Every declaration a type names, at any depth, is one the document carries: a field's codec, a
/// node's type, a binding's.
#[test]
fn every_declaration_a_type_names_is_one_the_document_carries() {
    let missing = r#"{"declared":"m.Missing"}"#;
    let codec = helpers(&[behind()]).replace(
        r#""codec":{"is":"named","declared":"m.S"}"#,
        r#""codec":{"is":"named","declared":"m.Missing"}"#,
    );
    is_the_halves_disagreeing(&codec, "m.Missing");
    let deep = option_of(&tuple_of(&[INT, missing]));
    is_the_halves_disagreeing(
        &helpers(&[behind(), h(&[&deep], &read(0, &deep))]),
        "m.Missing",
    );
}

/// A name a symbol cannot carry is refused where it is read, and not where a symbol is built from
/// it, which would be after the document was held to be whole.
#[test]
fn a_name_no_symbol_can_carry_is_refused_where_it_is_read() {
    let dotted_type = helpers(&[behind()]).replace(
        r#"{"module":"m","name":"A","by":"amodule","is":"unit"}"#,
        r#"{"module":"m","name":"A.x","by":"amodule","is":"unit"}"#,
    );
    is_the_halves_disagreeing(&dotted_type, "m.A.x");
    let dollar_module =
        helpers(&[behind()]).replace(r#""name":"m","helpers""#, r#""name":"m$","helpers""#);
    is_the_halves_disagreeing(&dollar_module, "m$");
    let dotted_behavior = r#"{"module":"other","name":"b.c","is":"elsewhere","inputs":[],"output":{"is":"scalar","scalar":"INT"}}"#;
    is_the_halves_disagreeing(
        &document(&[dotted_behavior.to_string()], &[behind()], &[]),
        "other.b.c",
    );
}

/// A negation answers a number, as every arithmetic does.
#[test]
fn a_negation_answers_a_number() {
    let negated = node("neg", &format!(r#""operand":{}"#, read(0, BOOL)), BOOL);
    is_the_halves_disagreeing(&helpers(&[behind(), h(&[BOOL], &negated)]), "a number");
}

/// A value in a slot is of exactly the type the slot takes it at, and where the checker let a
/// narrower one stand there it says so with a `Widen`. A case standing bare where its sum is taken
/// is a document the checker does not write.
#[test]
fn a_narrower_value_stands_in_a_slot_only_under_a_widen() {
    let fork = |then: &str| {
        node(
            "if",
            &format!(
                r#""cond":{},"then":{then},"else":{}"#,
                truth(true),
                widen(&unit("m.B"), S)
            ),
            S,
        )
    };
    reads_whole(&helpers(&[h(&[], &fork(&widen(&unit("m.A"), S)))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &fork(&unit("m.A")))]), "m.h");
}

/// A `Widen` is the checker's answer that its value may stand as the type it names, and a value of
/// one unit standing as another is not one it gives.
#[test]
fn a_widen_stands_a_value_only_as_what_it_is_a_value_of() {
    reads_whole(&helpers(&[h(&[], &widen(&unit("m.A"), S))]));
    is_the_halves_disagreeing(
        &helpers(&[h(&[], &widen(&unit("m.A"), r#"{"declared":"m.B"}"#))]),
        "a value standing as a wider type",
    );
}

/// A `Widen` says only what differs: one over a value of its own type, or over another `Widen`, is
/// a statement the checker never makes.
#[test]
fn a_widen_says_only_what_differs() {
    is_the_halves_disagreeing(&helpers(&[h(&[], &widen(&unit("m.A"), A))]), "its own type");
    let union =
        r#"{"union":[{"is":"declared","declared":"m.A"},{"is":"declared","declared":"m.B"}]}"#;
    is_the_halves_disagreeing(
        &helpers(&[h(&[], &widen(&widen(&unit("m.A"), union), S))]),
        "at one position once",
    );
}

/// A value standing as a type this backend has no representation for is not lowered, which is a
/// different answer from the two halves disagreeing: a union with a primitive among its members is
/// one the checker writes and nothing here lays out.
#[test]
fn a_widen_to_a_type_with_no_representation_is_not_lowered() {
    let union = r#"{"union":[{"is":"primitive","prim":"INT"},{"is":"declared","declared":"m.A"}]}"#;
    let refused = object_for(&helpers(&[h(&[], &widen(&int(1), union))]))
        .expect_err("a union with a primitive among its members has no representation");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
}

/// A function taking a sum stands as one taking a case of it, and one answering a case stands as
/// one answering the sum: what the position hands it is a value it takes, and what it answers is a
/// value of what the position answers. The other way round is not a function the position can use.
#[test]
fn a_function_stands_as_one_taking_less_and_answering_more() {
    let wide = fn_of(&[S], A);
    let narrow = fn_of(&[A], S);
    reads_whole(&helpers(&[h(&[&wide], &widen(&read(0, &wide), &narrow))]));
    is_the_halves_disagreeing(
        &helpers(&[h(&[&narrow], &widen(&read(0, &narrow), &wide))]),
        "a value standing as a wider type",
    );
}

/// Both sides of `++` stand as the list it answers, each under a `Widen` where it holds a narrower
/// element. A document with one side left at its own list is one the checker does not write, and it
/// is refused as that and not as a list this backend does not lay out.
#[test]
fn a_concat_operand_narrower_than_its_slot_without_a_widen_is_the_halves_disagreeing() {
    let listed = |of: &str| format!(r#"{{"list":{of}}}"#);
    let b = r#"{"declared":"m.B"}"#;
    let joined = |left: &str, right: &str| {
        node(
            "binary",
            &format!(r#""op":"CONCAT","left":{left},"right":{right}"#),
            &listed(S),
        )
    };
    let takes = [listed(A), listed(b)];
    let takes: Vec<&str> = takes.iter().map(String::as_str).collect();
    let both = joined(
        &widen(&read(0, &listed(A)), &listed(S)),
        &widen(&read(1, &listed(b)), &listed(S)),
    );
    let refused = object_for(&helpers(&[h(&takes, &both)])).expect_err("nothing lays a list out");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");

    let bare = joined(
        &read(0, &listed(A)),
        &widen(&read(1, &listed(b)), &listed(S)),
    );
    is_the_halves_disagreeing(&helpers(&[h(&takes, &bare)]), "the left side of ++");
}

/// Two strings joined are a string, and each side is one.
#[test]
fn a_concat_of_two_strings_reads_whole() {
    let joined = node(
        "binary",
        &format!(
            r#""op":"CONCAT","left":{},"right":{}"#,
            read(0, STRING),
            read(1, STRING)
        ),
        STRING,
    );
    reads_whole(&helpers(&[h(&[STRING, STRING], &joined)]));
    let answered_wrong = node(
        "binary",
        &format!(
            r#""op":"CONCAT","left":{},"right":{}"#,
            read(0, STRING),
            read(1, STRING)
        ),
        INT,
    );
    is_the_halves_disagreeing(&helpers(&[h(&[STRING, STRING], &answered_wrong)]), "++");
}
