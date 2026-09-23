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
            r#"{{"transport":10,"declarations":["#,
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
        &let_(0, S, &unit("m.A"), &read(0, S), S),
    )]));
    is_the_halves_disagreeing(
        &helpers(&[h(&[], &let_(0, S, &unit("m.A"), &read(0, A), A))]),
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

/// What a call hands over is a value of what the callee takes.
#[test]
fn an_argument_is_a_value_of_what_the_callee_takes() {
    let g = helper("m.g", &[S], &read(0, S));
    let reaches = r#"{"is":"helper","declared":"m.g"}"#;
    reads_whole(&helpers(&[
        g.clone(),
        h(&[], &call(reaches, &[unit("m.A")], S)),
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
        &format!(r#""declared":"m.P","values":[{}]"#, unit("m.A")),
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
            &format!(r#""declared":"m.P","values":[{}]"#, unit("m.A")),
            ty,
        )
    };
    reads_whole(&helpers(&[h(&[], &built(P))]));
    is_the_halves_disagreeing(&helpers(&[h(&[], &built(A))]), "m.P");
}

/// Each branch of a fork answers a value of what the fork answers: a case of it, or it.
#[test]
fn a_forks_branches_answer_values_of_what_it_answers() {
    let fork = |els: &str, ty: &str| {
        node(
            "if",
            &format!(
                r#""cond":{},"then":{},"else":{els}"#,
                truth(true),
                unit("m.A")
            ),
            ty,
        )
    };
    reads_whole(&helpers(&[h(&[], &fork(&unit("m.B"), S))]));
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
                arm(r#"{"tests":"nothing"}"#, None, &unit("m.A"))
            ),
            S,
        )
    };
    reads_whole(&helpers(&[h(&[&optional], &fork(S))]));
    is_the_halves_disagreeing(&helpers(&[h(&[&optional], &fork(A))]), "m.h");
}

/// What a present value holds is a value of what the optional holds.
#[test]
fn a_present_value_holds_a_value_of_what_its_optional_holds() {
    let some = |value: &str| node("some", &format!(r#""value":{value}"#), &option_of(S));
    reads_whole(&helpers(&[h(&[], &some(&unit("m.A")))]));
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

/// What a function value is applied to is a value of what it takes.
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
    reads_whole(&helpers(&[h(&[&function], &applied(unit("m.A")))]));
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
            &read(0, &listed(A)),
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
