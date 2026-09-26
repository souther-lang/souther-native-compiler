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
const DATE: &str = r#"{"prim":"DATE"}"#;
/// A primitive this backend has no layout for, for the tests about what is refused as not lowered
/// rather than as the two halves disagreeing, and about a refusal of the halves that comes before
/// lowering would refuse: a `Rational`, which no build lays out. Every test standing a type here
/// for "no layout" is held to it by `a_helper_this_backend_is_behind_on_is_on_its_own_not_lowered`,
/// which asks it to be refused as not lowered on its own; a type that stopped being so would make
/// the rest agree with themselves, so `Date` and the rest that are laid out are not used for it.
const RATIONAL: &str = r#"{"prim":"RATIONAL"}"#;
/// The same, for the external form a program cannot yet hold as a value: a `Raw`.
const RAW: &str = r#"{"prim":"RAW"}"#;
const A: &str = r#"{"ref":{"is":"declared","declared":"m.A"}}"#;
const S: &str = r#"{"ref":{"is":"declared","declared":"m.S"}}"#;
const P: &str = r#"{"ref":{"is":"declared","declared":"m.P"}}"#;

/// The units `m.A` and `m.B`, the sum `m.S = m.A | m.B`, the product `m.P` with one field `f` of
/// `m.S`, and the newtype `m.N` over an `Int`; the behaviors, helpers and local definitions given;
/// one module `m`.
fn document(behaviors: &[String], helpers: &[String], definitions: &[String]) -> String {
    format!(
        concat!(
            r#"{{"transport":26,"declarations":["#,
            r#"{{"module":"m","name":"A","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"B","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"S","by":"amodule","is":"sum","#,
            r#""cases":[{{"is":"declared","declared":"m.A"}},{{"is":"declared","declared":"m.B"}}],"#,
            r#""form":{{"is":"enumeration"}}}},"#,
            r#"{{"module":"m","name":"P","by":"amodule","is":"product","#,
            r#""fields":[{{"name":"f","binding":0,"codec":{{"is":"named","named":{{"is":"declared","declared":"m.S"}}}}}}],"invariants":[]}},"#,
            r#"{{"module":"m","name":"N","by":"amodule","is":"newtype","#,
            r#""field":{{"name":"v","binding":0,"codec":{{"is":"scalar","scalar":"INT"}}}},"invariants":[]}}],"#,
            r#""behaviors":[{}],"#,
            r#""modules":[{{"name":"m","publishes":[],"helpers":[{}],"values":[],"entries":[],"definitions":[{}],"#,
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
        r#"{{"reached":{},"parameters":[{}],"body":{body}}}"#,
        own(declared),
        parameters.join(",")
    )
}

/// A declaration of `m`, written `m.name`, as `m` reaches it: as its own.
fn own(declared: &str) -> String {
    let (module, name) = declared
        .rsplit_once('.')
        .expect("a helper written module.name");
    format!(r#"{{"is":"own","module":"{module}","name":"{name}"}}"#)
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

/// `node` naming these reasons for ending without a value, on the node itself: the `aborts` at the
/// end of the document, and not the ones on the literals and reads under it, which name none.
fn with_outer_aborts(node: &str, reasons: &str) -> String {
    let empty = r#""aborts":[]}"#;
    let at = node
        .rfind(empty)
        .expect("a node names what it can end without a value for");
    format!(
        "{}{}{}",
        &node[..at],
        format_args!(r#""aborts":[{reasons}]}}"#),
        &node[at + empty.len()..]
    )
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
        &format!(r#""unit":{{"is":"declared","declared":"{declared}"}}"#),
        &format!(r#"{{"ref":{{"is":"declared","declared":"{declared}"}}}}"#),
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

fn list_of(ty: &str) -> String {
    format!(r#"{{"list":{ty}}}"#)
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
/// of one type that disagree, and is refused as that before anything asks for the `Rational`'s
/// layout.
#[test]
fn a_read_disagreeing_with_a_binder_that_has_no_layout_is_the_halves_disagreeing() {
    is_the_halves_disagreeing(&helpers(&[h(&[RATIONAL], &read(0, INT))]), "m.h");
}

/// A behavior's parameters are bound at what its target takes, and its body read one of them as
/// something else: the body's root would pass as the target's answer while being an `Int`.
#[test]
fn a_behaviors_read_is_typed_as_its_target_takes() {
    let target = |answers: &str| {
        format!(
            r#"{{"module":"m","name":"b","is":"body","parameters":{{"named":[{{"name":"a","input":{{"is":"scalar","scalar":"INT"}}}}]}},"output":{{"is":"scalar","scalar":"{answers}"}},"requirements":[],"ensures":{{"at":"none"}}}}"#
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
    let reaches = &format!(r#"{{"is":"helper","reached":{}}}"#, own("m.g"));
    reads_whole(&helpers(&[g.clone(), h(&[], &call(reaches, &[], BOOL))]));
    is_the_halves_disagreeing(&helpers(&[g, h(&[], &call(reaches, &[], INT))]), "m.g");
}

/// A call of a behavior stands at what its target answers.
#[test]
fn a_call_of_a_behavior_stands_at_what_its_target_answers() {
    let target = r#"{"module":"m","name":"b","is":"body","parameters":{"named":[]},"output":{"is":"scalar","scalar":"INT"},"requirements":[],"ensures":{"at":"none"}}"#.to_string();
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
    let reaches = &format!(r#"{{"is":"helper","reached":{}}}"#, own("m.g"));
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

/// A test of which case a value is stands only over a union or a sum, which is the checker's own
/// case space: over anything else, what it is lowered to would read a token from the front of a
/// value that has none — a number, a truth, a product's first slot — and the document is refused as
/// the two halves disagreeing before anything is lowered.
#[test]
fn a_case_is_tested_only_of_a_union_or_a_sum() {
    let int_case = r#"{"is":"primitive","prim":"INT"}"#;
    let fork = |subject: &str, atom: &str, binds: &str| {
        node(
            "match",
            &format!(
                r#""subject":{},"arms":[{}]"#,
                read(0, subject),
                arm(
                    &format!(r#"{{"tests":"which","atoms":[{atom}]}}"#),
                    Some((1, binds)),
                    &int(0)
                )
            ),
            INT,
        )
    };
    let declared = |key: &str| format!(r#"{{"is":"declared","declared":"{key}"}}"#);
    let union = format!(r#"{{"union":[{int_case},{}]}}"#, declared("m.A"));

    // Over a union and over a sum, a case of either.
    reads_whole(&helpers(&[h(&[&union], &fork(&union, int_case, INT))]));
    reads_whole(&helpers(&[h(&[S], &fork(S, &declared("m.A"), A))]));

    // Over a plain `Int`, which is every one of its own cases and says so nowhere.
    is_the_halves_disagreeing(
        &helpers(&[h(&[INT], &fork(INT, int_case, INT))]),
        "only a union or a sum has cases",
    );
    // Over a product and a unit, which have one case each and nothing to test.
    is_the_halves_disagreeing(
        &helpers(&[h(&[P], &fork(P, &declared("m.P"), P))]),
        "only a union or a sum has cases",
    );
    is_the_halves_disagreeing(
        &helpers(&[h(&[A], &fork(A, &declared("m.A"), A))]),
        "only a union or a sum has cases",
    );
    // Over an optional, which answers by holding a value or not.
    let optional = option_of(S);
    is_the_halves_disagreeing(
        &helpers(&[h(&[&optional], &fork(&optional, &declared("m.A"), A))]),
        "only a union or a sum has cases",
    );
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

/// An arm binds what each of its tests leaves to read. A test that an optional holds nothing
/// leaves nothing, so an arm binding a value there is the two halves disagreeing, and not this
/// backend being behind; and an arm testing for `None` among the cases of a union binds the value
/// as one of those cases, the way an arm testing for declared cases does.
#[test]
fn an_arm_binds_what_each_of_its_tests_leaves_to_read() {
    let optional = option_of(S);
    let binding_nothing = node(
        "match",
        &format!(
            r#""subject":{},"arms":[{},{}]"#,
            read(0, &optional),
            arm(r#"{"tests":"held"}"#, Some((1, S)), &read(1, S)),
            arm(
                r#"{"tests":"nothing"}"#,
                Some((2, &optional)),
                &widen(&unit("m.A"), S)
            )
        ),
        S,
    );
    is_the_halves_disagreeing(
        &helpers(&[h(&[&optional], &binding_nothing)]),
        "leaves nothing to bind",
    );

    let none = r#"{"is":"language","case":"NONE"}"#;
    let a_case = r#"{"is":"declared","declared":"m.A"}"#;
    let b_case = r#"{"is":"declared","declared":"m.B"}"#;
    let union = format!(r#"{{"union":[{none},{a_case},{b_case}]}}"#);
    let tested = format!(r#"{{"union":[{none},{a_case}]}}"#);
    let with_cases = node(
        "match",
        &format!(
            r#""subject":{},"arms":[{},{}]"#,
            read(0, &union),
            arm(
                &format!(r#"{{"tests":"which","atoms":[{none},{a_case}]}}"#),
                Some((1, &tested)),
                &int(1)
            ),
            arm(&which(&["m.B"]), None, &int(2)),
        ),
        INT,
    );
    reads_whole(&helpers(&[h(&[&union], &with_cases)]));
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
            &format!(
                r#""op":"EQ","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
                int(1),
                int(2)
            ),
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
    let reaches = r#"{"is":"kernel","kernel":"int.add","takes":[{"prim":"INT"},{"prim":"INT"}],"fact":{"is":"none"}}"#;
    let added = |ty: &str| {
        with_outer_aborts(
            &node(
                "call",
                &format!(r#""reaches":{reaches},"arguments":[{},{}]"#, int(1), int(2)),
                ty,
            ),
            r#""REQUIRED_FORM_HAS_NO_PLACE""#,
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
    let reaching = r#""reaches":{"is":"value","module":"m","name":"ks"},"arguments":[],"type":{"ref":{"is":"declared","declared":"m.P"}}"#;
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
    let reaching = r#""reaches":{"is":"publishedvalue","module":"publisher","name":"ys"},"arguments":[],"type":{"ref":{"is":"declared","declared":"publisher.Box"}}"#;
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
            r#"{{"module":"m","name":"{name}","is":"{is}","parameters":{},"output":{{"is":"scalar","scalar":"{answers}"}},"requirements":[],"ensures":{{"at":"none"}}}}"#,
            taking(is, &format!(r#"{{"is":"scalar","scalar":"{takes}"}}"#))
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

/// `m.flow = m.first >-> m.second`, kept: `m.first` taking an `Int` and answering the boundary
/// shape `first` as the value `made`, `m.second` taking `taken` and answering an `Int`, the second
/// stage routed as `routing`, and the composition answering `flows`.
fn routed(first: &str, made: &str, taken: &str, routing: &str, flows: &str) -> String {
    let scalar_int = r#"{"is":"scalar","scalar":"INT"}"#;
    let target = |name: &str, is: &str, input: &str, output: &str| {
        format!(
            r#"{{"module":"m","name":"{name}","is":"{is}","parameters":{},"output":{output},"requirements":[],"ensures":{{"at":"none"}}}}"#,
            taking(is, input)
        )
    };
    let body = |name: &str, node: &str| {
        format!(
            r#"{{"is":"body","declared":"m.{name}","parameters":["a"],"publication":"kept","body":{node}}}"#
        )
    };
    let flow = format!(
        r#"{{"is":"composed","declared":"m.flow","publication":"kept","stages":[{{"behavior":"m.first","routing":{{"is":"always"}}}},{{"behavior":"m.second","routing":{routing}}}]}}"#
    );
    document(
        &[
            target("first", "body", scalar_int, first),
            target("second", "body", taken, scalar_int),
            target("flow", "composed", scalar_int, flows),
        ],
        &[],
        &[body("first", made), body("second", &int(2)), flow],
    )
}

/// What runs is offered to a stage by its cases exactly where it is a declared type or a union,
/// which is the checker's rule, and it is tested by the token at its front. A plain `Int` routed on
/// its cases would be a token read from a number, and a sum handed whole to a stage that takes it
/// is not what the checker writes either; both are the two halves disagreeing. A stage may accept a
/// case no declaration names, and is handed it the way an arm reads one.
#[test]
fn what_runs_is_routed_on_its_cases_only_where_it_says_them() {
    let scalar = r#"{"is":"scalar","scalar":"INT"}"#;
    let sum = r#"{"is":"nominal","named":{"is":"declared","declared":"m.S"}}"#;
    let int_case = r#"{"is":"primitive","prim":"INT"}"#;
    let a_case = r#"{"is":"declared","declared":"m.A"}"#;
    let b_case = r#"{"is":"declared","declared":"m.B"}"#;
    let on = |cases: &[&str]| format!(r#"{{"is":"oncases","accepted":[{}]}}"#, cases.join(","));
    let always = r#"{"is":"always"}"#;
    let a_sum = widen(&unit("m.A"), S);

    // A sum routed on its cases, every one of them accepted.
    reads_whole(&routed(sum, &a_sum, sum, &on(&[a_case, b_case]), scalar));
    // The same sum handed whole.
    is_the_halves_disagreeing(
        &routed(sum, &a_sum, sum, always, scalar),
        "offered by its cases",
    );
    // A plain `Int` routed on its cases.
    is_the_halves_disagreeing(
        &routed(scalar, &int(1), scalar, &on(&[int_case]), scalar),
        "offered by its cases",
    );

    // A union with an `Int` among its cases, routed on the `Int`, which the stage is handed read
    // back out of what carries it.
    let union = format!(r#"{{"union":[{int_case},{a_case}]}}"#);
    let cases = format!(
        r#"{{"is":"cases","type":{union},"cases":[{int_case},{a_case}],"form":{{"is":"discriminated","tag":"type","contents":"value"}}}}"#
    );
    reads_whole(&routed(
        &cases,
        &widen(&int(1), &union),
        scalar,
        &on(&[int_case]),
        &cases,
    ));
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
/// set is a set of a wider type, which nothing here has a rule for.
#[test]
fn a_disagreement_anywhere_is_refused_before_anything_is_not_lowered() {
    let set = |of: &str| format!(r#"{{"set":{of}}}"#);
    let g = helper(
        "m.g",
        &[&set(A)],
        &let_(
            1,
            &set(S),
            &widen(&read(0, &set(A)), &set(S)),
            &read(1, &set(S)),
            &set(S),
        ),
    );
    let refused =
        object_for(&helpers(std::slice::from_ref(&g))).expect_err("nothing lays a set out");
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
    format!(r#"{{"behavior":"{behavior}","at":{at},"body":{body},"standsIn":[]}}"#)
}

/// `m.b`, taking nothing and answering an `Int` its body makes.
fn b() -> (String, String) {
    (
        r#"{"module":"m","name":"b","is":"body","parameters":{"named":[]},"output":{"is":"scalar","scalar":"INT"},"requirements":[],"ensures":{"at":"none"}}"#
            .to_string(),
        format!(
            r#"{{"is":"body","declared":"m.b","parameters":[],"publication":"kept","body":{}}}"#,
            int(1)
        ),
    )
}

/// A helper with no layout here, which on its own is refused as not lowered.
fn behind() -> String {
    helper("m.behind", &[RATIONAL], &read(0, RATIONAL))
}

#[test]
fn a_helper_this_backend_is_behind_on_is_on_its_own_not_lowered() {
    let refused = object_for(&helpers(&[behind()])).expect_err("no layout for a Rational");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
}

/// A `Raw` is the external form itself, which the runtime builds to write a value and holds only
/// until it is written, so no program holds one as a value yet. Refused as not lowered, as a
/// `Rational` is, and not as the two halves disagreeing: the checker does write one.
#[test]
fn a_raw_is_not_lowered_where_a_value_of_it_is_held() {
    let refused = object_for(&helpers(&[helper("m.raw", &[RAW], &read(0, RAW))]))
        .expect_err("no layout for a Raw");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(refused.to_string().contains("Raw"), "{refused}");
}

/// Two helpers one module holds under one name: whichever was read last would be checked, and
/// whichever was declared first compiled. Refused as the name written twice, and not as the first
/// copy's `Rational`, which a backend reading the document in another order would never have met.
#[test]
fn a_helper_written_twice_is_refused_before_either_is_lowered() {
    let first = helper("m.g", &[RATIONAL], &read(0, RATIONAL));
    let second = helper("m.g", &[INT], &read(0, INT));
    is_the_halves_disagreeing(&helpers(&[first, second]), "m.g");
}

/// A reference's route is the checker's from the module holding it (`ReachName.of`): a module
/// reaches a declaration of its own as its own, and one of another module under that module's
/// name. A reference taking the other route names one declaration two ways, and what a call finds
/// and what the module is held to would be read off two spellings of it.
#[test]
fn a_reference_routed_otherwise_than_the_checker_routes_it_is_the_halves_disagreeing() {
    let as_another_modules =
        helper("m.g", &[INT], &read(0, INT)).replacen(r#""is":"own""#, r#""is":"ofmodule""#, 1);
    is_the_halves_disagreeing(&helpers(&[as_another_modules]), "not the route");
    let another_modules_as_own = helper("m.g", &[INT], &read(0, INT)).replacen(
        r#""module":"m","name":"g""#,
        r#""module":"elsewhere","name":"g""#,
        1,
    );
    is_the_halves_disagreeing(&helpers(&[another_modules_as_own]), "not the route");
}

/// A module holds a declaration as a value or carries a method for it as a helper, not both. A
/// helper is reached under a reference and a value under its declaration, so the two are held
/// against each other by the declaration the reference reaches.
#[test]
fn a_declaration_held_both_as_a_value_and_as_a_helper_is_the_halves_disagreeing() {
    let values = include_str!("values.transport.json");
    let copy = r#"{"reached":{"is":"own","module":"m","name":"ks"},"parameters":[],"body":{"core":"int","value":1,"type":{"prim":"INT"},"aborts":[]}}"#;
    let document = values.replacen(r#""helpers":[]"#, &format!(r#""helpers":[{copy}]"#), 1);
    assert_ne!(document, values, "the fixture this perturbs moved");
    is_the_halves_disagreeing(&document, "m.ks both as a helper and as a value");
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
            r#"{{"name":"m","publishes":[],"helpers":[{helpers}],"values":[],"entries":[],"definitions":[],"examples":[]}}"#
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
/// as that, and not as the target's `Set` having no layout.
#[test]
fn a_target_defined_here_with_nothing_defining_it_is_refused_before_its_signature_is_asked() {
    let target = r#"{"module":"m","name":"b","is":"body","parameters":{"named":[]},"output":{"is":"setof","element":{"is":"scalar","scalar":"INT"}},"requirements":[],"ensures":{"at":"none"}}"#;
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
/// that, before the `Rational` elsewhere is found to have no layout.
#[test]
fn a_quotient_is_a_rational() {
    let divided = |ty: &str| {
        node(
            "binary",
            &format!(
                r#""op":"DIV","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
                int(1),
                int(2)
            ),
            ty,
        )
    };
    is_the_halves_disagreeing(&helpers(&[behind(), h(&[], &divided(INT))]), "m.h");
    is_the_halves_disagreeing(&helpers(&[behind(), h(&[], &divided(BOOL))]), "m.h");
}

/// Two numbers of two types, told apart by what the operator reads them as. `Int + Rational` is
/// read at the exact values of both and answers a `Rational`: the checker writes it, and this
/// backend has no lowering for it. `Int + Decimal` read as they stand is two types where the
/// reading says one: the checker never writes it, and it is the two halves disagreeing.
#[test]
fn numbers_of_two_types_are_told_apart_by_how_the_operator_reads_them() {
    let rational = r#"{"prim":"RATIONAL"}"#;
    let added = |other: &str, reading: &str| {
        with_outer_aborts(
            &node(
                "binary",
                &format!(
                    r#""op":"ADD","reading":{{"is":"{reading}"}},"left":{},"right":{}"#,
                    read(0, INT),
                    read(1, other)
                ),
                rational,
            ),
            r#""REQUIRED_FORM_HAS_NO_PLACE""#,
        )
    };
    let refused = object_for(&helpers(&[h(
        &[INT, rational],
        &added(rational, "exactnumbers"),
    )]))
    .expect_err("no lowering for exact values");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");

    is_the_halves_disagreeing(
        &helpers(&[h(&[INT, DECIMAL], &added(DECIMAL, "astheystand"))]),
        "read as it stands",
    );
}

/// Arithmetic over two `Int`s can leave their range, and names the one reason it ends without a
/// value; naming none is the checker and this backend disagreeing about what kind of site it is.
#[test]
fn arithmetic_that_can_overflow_names_one_reason() {
    let added = |aborts: &str| {
        with_outer_aborts(
            &node(
                "binary",
                &format!(
                    r#""op":"ADD","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
                    int(1),
                    int(2)
                ),
                INT,
            ),
            aborts,
        )
    };
    reads_whole(&helpers(&[h(
        &[],
        &added(r#""REQUIRED_FORM_HAS_NO_PLACE""#),
    )]));
    // None, and another reason. The lowering turns the reason into the status a run that leaves the
    // range ends with, so a reason the checker never gave a sum would end the run for it.
    for wrong in ["", r#""DIVISION_BY_ZERO""#, r#""INVARIANT_NOT_HELD""#] {
        is_the_halves_disagreeing(
            &helpers(&[behind(), h(&[], &added(wrong))]),
            "where the checker names",
        );
    }
}

/// What every arithmetic site owes, by what decides it: a sum, a difference and a product name the
/// one reason whatever type they are over, and a quotient names a zero divisor, and an answer with no
/// place where an operand is already exact.
#[test]
fn every_arithmetic_site_names_exactly_the_reasons_it_owes() {
    let rational = r#"{"prim":"RATIONAL"}"#;
    let over = |op: &str, ty: &str, answers: &str, reading: &str, reasons: &str| {
        with_outer_aborts(
            &node(
                "binary",
                &format!(
                    r#""op":"{op}","reading":{{"is":"{reading}"}},"left":{},"right":{}"#,
                    read(0, ty),
                    read(1, ty)
                ),
                answers,
            ),
            reasons,
        )
    };
    let no_place = r#""REQUIRED_FORM_HAS_NO_PLACE""#;
    let zero = r#""DIVISION_BY_ZERO""#;
    let both = format!("{zero},{no_place}");

    for (op, reasons, wrong) in [
        ("ADD", no_place, zero),
        ("SUB", no_place, zero),
        ("MUL", no_place, ""),
    ] {
        reads_whole(&helpers(&[h(
            &[INT, INT],
            &over(op, INT, INT, "astheystand", reasons),
        )]));
        is_the_halves_disagreeing(
            &helpers(&[h(&[INT, INT], &over(op, INT, INT, "astheystand", wrong))]),
            "where the checker names",
        );
    }
    // A quotient of two Ints names a zero divisor, and of two Rationals a place too.
    let _ = reads_whole_or_not_lowered(&helpers(&[h(
        &[INT, INT],
        &over("DIV", INT, rational, "astheystand", zero),
    )]));
    for (ty, right, wrong) in [
        (INT, zero, no_place),
        (INT, zero, ""),
        (rational, both.as_str(), zero),
    ] {
        let refused = object_for(&helpers(&[h(
            &[ty, ty],
            &over("DIV", ty, rational, "astheystand", wrong),
        )]))
        .expect_err("a quotient names the reasons it owes");
        assert!(
            refused.to_string().contains("where the checker names"),
            "{right}: {refused}"
        );
    }
}

/// A document Coherent reads whole and the lowering then refuses as not lowered is one this backend
/// is behind on; either answer is not the two halves disagreeing.
fn reads_whole_or_not_lowered(document: &str) -> bool {
    match object_for(document) {
        Ok(_) => true,
        Err(refused) => {
            assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
            false
        }
    }
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
    let unit_of_a_product = node("unit", r#""unit":{"is":"declared","declared":"m.P"}"#, P);
    is_the_halves_disagreeing(&helpers(&[h(&[], &unit_of_a_product)]), "m.P");
    let building_a_unit = node("construct", r#""declared":"m.A","values":[]"#, A);
    is_the_halves_disagreeing(&helpers(&[h(&[], &building_a_unit)]), "m.A");
}

/// A unit the language gives is carried as a union carries it, and names its type as the case it
/// is. A primitive named as a unit has no representation designed, and is not lowered rather than
/// the halves disagreeing: the checker names a type so.
#[test]
fn a_unit_the_language_gives_is_carried_and_a_primitive_named_as_one_is_not_lowered() {
    let division = r#"{"is":"language","case":"DIVISION_BY_ZERO"}"#;
    let given = node(
        "unit",
        &format!(r#""unit":{division}"#),
        &format!(r#"{{"ref":{division}}}"#),
    );
    reads_whole(&helpers(&[h(&[], &given)]));
    is_the_halves_disagreeing(
        &helpers(&[h(&[], &node("unit", &format!(r#""unit":{division}"#), A))]),
        "DivisionByZero",
    );

    let int = r#"{"is":"primitive","prim":"INT"}"#;
    let named = node(
        "unit",
        &format!(r#""unit":{int}"#),
        &format!(r#"{{"ref":{int}}}"#),
    );
    let refused = object_for(&helpers(&[h(&[], &named)])).expect_err("a primitive named as a type");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(refused.to_string().contains("named as a type"), "{refused}");
}

/// An `unreachable` ends the run for the one reason there is to, whatever the position it stands in
/// takes. Naming another reason, or none, is the halves disagreeing.
#[test]
fn an_unreachable_ends_the_run_for_the_one_reason_it_has() {
    let unreachable = |ty: &str, aborts: &str| {
        format!(r#"{{"core":"unreachable","reason":"no","type":{ty},"aborts":[{aborts}]}}"#)
    };
    reads_whole(&helpers(&[h(
        &[],
        &unreachable(INT, r#""UNREACHABLE_REACHED""#),
    )]));
    reads_whole(&helpers(&[h(
        &[],
        &unreachable(A, r#""UNREACHABLE_REACHED""#),
    )]));
    is_the_halves_disagreeing(
        &helpers(&[h(&[], &unreachable(INT, r#""DIVISION_BY_ZERO""#))]),
        "unreachable",
    );
    is_the_halves_disagreeing(&helpers(&[h(&[], &unreachable(INT, ""))]), "unreachable");
}

/// A helper answering the type of what does not answer, called where that is what the call stands
/// at, is called at its own type: not the halves disagreeing, and not lowered only because nothing
/// of a value of it is laid out.
#[test]
fn a_helper_that_does_not_answer_is_called_at_its_own_type() {
    let never = r#"{"never":{}}"#;
    let ending = format!(
        r#"{{"core":"unreachable","reason":"no","type":{never},"aborts":["UNREACHABLE_REACHED"]}}"#
    );
    let g = helper("m.g", &[], &ending);
    let reaches = &format!(r#"{{"is":"helper","reached":{}}}"#, own("m.g"));
    let refused = object_for(&helpers(&[g, h(&[], &call(reaches, &[], never))]))
        .expect_err("a value of Never");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(refused.to_string().contains("Never"), "{refused}");
}

/// The type of what does not answer is read, and nothing of it is laid out: a position the
/// checker left typed as it is holds no value this backend could make.
#[test]
fn a_value_of_the_type_of_what_does_not_answer_is_not_lowered() {
    let never = r#"{"never":{}}"#;
    let ending = format!(
        r#"{{"core":"unreachable","reason":"no","type":{never},"aborts":["UNREACHABLE_REACHED"]}}"#
    );
    let refused = object_for(&helpers(&[h(&[], &ending)])).expect_err("a value of Never");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(refused.to_string().contains("Never"), "{refused}");
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
    let target = r#"{"module":"other","name":"b","is":"body","parameters":{"named":[]},"output":{"is":"scalar","scalar":"INT"},"requirements":[],"ensures":{"at":"none"}}"#;
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
    // What is done about another build's clause is nobody's here to decide, and what is done
    // about one of this document's own is.
    let target = |module: &str, ensures: &str| {
        format!(
            r#"{{"module":"{module}","name":"b","is":"elsewhere","parameters":{{"named":[]}},"output":{{"is":"scalar","scalar":"INT"}},"requirements":[],"ensures":{{"at":"{ensures}"}}}}"#
        )
    };
    reads_whole(&document(&[target("other", "undecided")], &[], &[]));
    is_the_halves_disagreeing(&document(&[target("m", "none")], &[behind()], &[]), "m.b");
}

/// A `Decimal` literal carries its integer as integer text, which is what the runtime is handed
/// to make one of: a literal whose integer is written otherwise is the halves disagreeing, and so
/// is one typed as anything but a `Decimal`.
#[test]
fn a_decimal_literal_carries_its_integer_as_integer_text() {
    let literal = |unscaled: &str, ty: &str| {
        node(
            "decimal",
            &format!(r#""unscaled":"{unscaled}","scale":2"#),
            ty,
        )
    };
    reads_whole(&helpers(&[h(&[], &literal("-150", DECIMAL))]));
    reads_whole(&helpers(&[h(&[], &literal("0", DECIMAL))]));
    for written in ["1.50", "", "-", "1e2", "+15", "１"] {
        is_the_halves_disagreeing(
            &helpers(&[h(&[], &literal(written, DECIMAL))]),
            "a decimal literal",
        );
    }
    is_the_halves_disagreeing(&helpers(&[h(&[], &literal("150", INT))]), "m.h");
}

/// A temporal literal carries the count the checker read it as, and the runtime makes the value
/// from it: a count outside what the type holds, a fraction of a second where the type holds none,
/// and a type that is none of the four are the halves disagreeing. What a program may spell is the
/// checker's, and nothing here reads a spelling.
#[test]
fn a_temporal_literal_carries_a_count_its_type_holds() {
    let literal = |count: i64, nano: i64, ty: &str| {
        node("temporal", &format!(r#""count":{count},"nano":{nano}"#), ty)
    };
    let time = r#"{"prim":"TIME"}"#;
    let date_time = r#"{"prim":"DATETIME"}"#;
    let instant = r#"{"prim":"INSTANT"}"#;
    reads_whole(&helpers(&[h(&[], &literal(20_659, 0, DATE))]));
    reads_whole(&helpers(&[h(&[], &literal(365_241_780_471, 0, DATE))]));
    reads_whole(&helpers(&[h(&[], &literal(-365_243_219_162, 0, DATE))]));
    reads_whole(&helpers(&[h(&[], &literal(0, 0, time))]));
    reads_whole(&helpers(&[h(&[], &literal(86_399, 0, time))]));
    reads_whole(&helpers(&[h(&[], &literal(1_785_000_000, 0, date_time))]));
    reads_whole(&helpers(&[h(
        &[],
        &literal(1_785_000_000, 500_000_000, instant),
    )]));
    reads_whole(&helpers(&[h(
        &[],
        &literal(31_556_889_864_403_199, 999_999_999, instant),
    )]));
    for (count, nano, ty) in [
        (365_241_780_472, 0, DATE),
        (-365_243_219_163, 0, DATE),
        (20_659, 1, DATE),
        (-1, 0, time),
        (86_400, 0, time),
        (0, 1, time),
        (365_241_780_471 * 86_400 + 86_400, 0, date_time),
        (0, 1, date_time),
        (31_556_889_864_403_200, 0, instant),
        (-31_557_014_167_219_201, 0, instant),
        (0, 1_000_000_000, instant),
        (0, -1, instant),
    ] {
        is_the_halves_disagreeing(
            &helpers(&[h(&[], &literal(count, nano, ty))]),
            "the two halves disagree",
        );
    }
    is_the_halves_disagreeing(&helpers(&[h(&[], &literal(20_659, 0, INT))]), "m.h");
}

/// A unit the language declares is at home in the runtime, which defines its token: a value of
/// one is built where it stands and tagged by what every object links. One the runtime defines no
/// token for is a value no object can say it is, and is not lowered.
#[test]
fn a_unit_the_language_declares_is_tagged_by_the_runtimes_token() {
    let with = |name: &str| {
        helpers(&[h(&[], &unit(&format!("souther.decimal.{name}")))]).replace(
            r#"{"module":"m","name":"A","by":"amodule","is":"unit"},"#,
            &format!(
                r#"{{"module":"m","name":"A","by":"amodule","is":"unit"}},{{"module":"souther.decimal","name":"{name}","by":"thelanguage","is":"unit"}},"#
            ),
        )
    };
    reads_whole(&with("HALF_UP"));
    let refused = object_for(&with("HALF_SIDEWAYS")).expect_err("a unit no runtime token tags");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(
        refused
            .to_string()
            .contains("souther.decimal.HALF_SIDEWAYS"),
        "{refused}"
    );
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
    let missing = r#"{"ref":{"is":"declared","declared":"m.Missing"}}"#;
    let codec = helpers(&[behind()]).replace(
        r#""codec":{"is":"named","named":{"is":"declared","declared":"m.S"}}"#,
        r#""codec":{"is":"named","named":{"is":"declared","declared":"m.Missing"}}"#,
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
    let dollar_module = helpers(&[behind()]).replace(
        r#""name":"m","publishes":[],"helpers""#,
        r#""name":"m$","publishes":[],"helpers""#,
    );
    is_the_halves_disagreeing(&dollar_module, "m$");
    let dotted_behavior = r#"{"module":"other","name":"b.c","is":"elsewhere","parameters":{"named":[]},"output":{"is":"scalar","scalar":"INT"},"requirements":[],"ensures":{"at":"none"}}"#;
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
        &helpers(&[h(
            &[],
            &widen(
                &unit("m.A"),
                r#"{"ref":{"is":"declared","declared":"m.B"}}"#,
            ),
        )]),
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

/// A primitive standing as a case of a union is carried with the runtime's token for it, so the
/// union's value says which case it is as a declared case's does.
#[test]
fn a_primitive_stands_as_a_case_of_a_union() {
    let union = r#"{"union":[{"is":"primitive","prim":"INT"},{"is":"declared","declared":"m.A"}]}"#;
    reads_whole(&helpers(&[h(&[], &widen(&int(1), union))]));
}

/// A union naming one of an optional's two cases is held the way it holds any case the language
/// gives, by the runtime's token and nothing more, and an arm tests for it by that token.
#[test]
fn a_union_with_an_optionals_case_among_its_cases_is_tested_by_its_token() {
    for case in ["SOME", "NONE"] {
        let atom = format!(r#"{{"is":"language","case":"{case}"}}"#);
        let union = format!(r#"{{"union":[{atom},{{"is":"declared","declared":"m.A"}}]}}"#);
        let fork = node(
            "match",
            &format!(
                r#""subject":{},"arms":[{},{}]"#,
                read(0, &union),
                arm(
                    &format!(r#"{{"tests":"which","atoms":[{atom}]}}"#),
                    None,
                    &int(1)
                ),
                arm(&which(&["m.A"]), None, &int(2)),
            ),
            INT,
        );
        reads_whole(&helpers(&[h(&[&union], &fork)]));
    }
}

/// A value standing as a type this backend has no representation for is not lowered, which is a
/// different answer from the two halves disagreeing: a `Rational` has no representation to carry.
#[test]
fn a_widen_to_a_type_with_no_representation_is_not_lowered() {
    let date =
        r#"{"union":[{"is":"primitive","prim":"RATIONAL"},{"is":"declared","declared":"m.A"}]}"#;
    let refused = object_for(&helpers(&[h(&[], &widen(&unit("m.A"), date))]))
        .expect_err("a union with a Rational among its members has no representation");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(refused.to_string().contains("Rational"), "{refused}");
}

/// What holds a value stands as what holds a wider one wherever what it holds does: a list of a
/// case as a list of its sum, held alike, and an optional or a list of `Int`s as one of a union
/// with `Int` among its cases, rebuilt with what it holds carried (`tests/restating.rs` runs it).
#[test]
fn what_holds_a_value_stands_as_what_holds_a_wider_one() {
    let cases = list_of(A);
    reads_whole(&helpers(&[h(
        &[&cases],
        &widen(&read(0, &cases), &list_of(S)),
    )]));

    let union = r#"{"union":[{"is":"primitive","prim":"INT"},{"is":"declared","declared":"m.A"}]}"#;
    let numbers = list_of(INT);
    reads_whole(&helpers(&[h(
        &[&numbers],
        &widen(&read(0, &numbers), &list_of(union)),
    )]));
    let held = option_of(INT);
    reads_whole(&helpers(&[h(
        &[&held],
        &widen(&read(0, &held), &option_of(union)),
    )]));
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
/// element, and so written the two are joined. A document with one side left at its own list is one
/// the checker does not write, and it is refused as that.
#[test]
fn a_concat_operand_narrower_than_its_slot_without_a_widen_is_the_halves_disagreeing() {
    let b = r#"{"ref":{"is":"declared","declared":"m.B"}}"#;
    let joined = |left: &str, right: &str| {
        node(
            "binary",
            &format!(
                r#""op":"CONCAT","reading":{{"is":"astheystand"}},"left":{left},"right":{right}"#
            ),
            &list_of(S),
        )
    };
    let takes = [list_of(A), list_of(b)];
    let takes: Vec<&str> = takes.iter().map(String::as_str).collect();
    let both = joined(
        &widen(&read(0, &list_of(A)), &list_of(S)),
        &widen(&read(1, &list_of(b)), &list_of(S)),
    );
    object_for(&helpers(&[h(&takes, &both)])).expect("two lists standing as one type are joined");

    let bare = joined(
        &read(0, &list_of(A)),
        &widen(&read(1, &list_of(b)), &list_of(S)),
    );
    is_the_halves_disagreeing(&helpers(&[h(&takes, &bare)]), "the left side of ++");
}

/// Two strings joined are a string, and each side is one.
#[test]
fn a_concat_of_two_strings_reads_whole() {
    let joined = node(
        "binary",
        &format!(
            r#""op":"CONCAT","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
            read(0, STRING),
            read(1, STRING)
        ),
        STRING,
    );
    reads_whole(&helpers(&[h(&[STRING, STRING], &joined)]));
    let answered_wrong = node(
        "binary",
        &format!(
            r#""op":"CONCAT","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
            read(0, STRING),
            read(1, STRING)
        ),
        INT,
    );
    is_the_halves_disagreeing(&helpers(&[h(&[STRING, STRING], &answered_wrong)]), "++");
}

/// The product `m.R` with the fields and clauses given, and the helpers given; one module `m`.
fn with_clauses(fields: &str, invariants: &str, helpers: &[String]) -> String {
    format!(
        concat!(
            r#"{{"transport":26,"declarations":["#,
            r#"{{"module":"m","name":"R","by":"amodule","is":"product","#,
            r#""fields":[{}],"invariants":[{}]}}],"#,
            r#""behaviors":[],"#,
            r#""modules":[{{"name":"m","publishes":[],"helpers":[{}],"values":[],"entries":[],"definitions":[],"#,
            r#""examples":[]}}]}}"#
        ),
        fields,
        invariants,
        helpers.join(",")
    )
}

fn field(name: &str, binding: usize, scalar: &str) -> String {
    format!(
        r#"{{"name":"{name}","binding":{binding},"codec":{{"is":"scalar","scalar":"{scalar}"}}}}"#
    )
}

fn clause(name: Option<&str>, condition: &str) -> String {
    let name = name.map_or("null".to_string(), |it| format!(r#""{it}""#));
    format!(r#"{{"name":{name},"condition":{condition}}}"#)
}

fn at_least(left: &str, right: &str) -> String {
    node(
        "binary",
        &format!(r#""op":"GE","reading":{{"is":"astheystand"}},"left":{left},"right":{right}"#),
        BOOL,
    )
}

/// A clause reads each field under the binding the field is bound at, which need not be where the
/// field sits: here the `Bool` is laid out first and bound second.
#[test]
fn a_clause_reads_a_field_under_its_binding_and_not_its_position() {
    let fields = [field("flag", 1, "BOOL"), field("count", 0, "INT")].join(",");
    let holds = clause(Some("counted"), &at_least(&read(0, INT), &int(0)));
    reads_whole(&with_clauses(&fields, &holds, &[]));

    let by_position = clause(Some("counted"), &at_least(&read(1, INT), &int(0)));
    is_the_halves_disagreeing(
        &with_clauses(&fields, &by_position, &[]),
        "m.R's clause counted",
    );
}

/// What a clause reads is one of the fields and nothing else.
#[test]
fn a_clause_reads_only_what_its_fields_bind() {
    let fields = field("count", 0, "INT");
    let stray = clause(None, &at_least(&read(3, INT), &int(0)));
    is_the_halves_disagreeing(&with_clauses(&fields, &stray, &[]), "m.R's clause 0");
}

/// A clause is something that has to hold, so it is a truth.
#[test]
fn a_clause_is_a_truth() {
    let fields = field("count", 0, "INT");
    is_the_halves_disagreeing(
        &with_clauses(&fields, &clause(None, &read(0, INT)), &[]),
        "where a clause is a truth",
    );
}

/// Two fields under one binding would be one name read for two values.
#[test]
fn no_two_fields_share_a_binding() {
    let fields = [field("one", 0, "INT"), field("other", 0, "INT")].join(",");
    let holds = clause(None, &at_least(&read(0, INT), &int(0)));
    is_the_halves_disagreeing(
        &with_clauses(&fields, &holds, &[]),
        "binds two fields under 0",
    );
}

/// A construction of a type that states a clause names `INVARIANT_NOT_HELD` as what it can end
/// with, and one of a type that states none names nothing: the checker says so, and a document
/// saying otherwise disagrees about the type.
#[test]
fn a_construction_says_it_can_fail_exactly_where_its_type_states_a_clause() {
    let built = |aborts: &str| {
        format!(
            r#"{{"core":"construct","declared":"m.R","values":[{}],"type":{{"ref":{{"is":"declared","declared":"m.R"}}}},"aborts":{aborts}}}"#,
            int(1)
        )
    };
    let fields = field("count", 0, "INT");
    let holds = clause(None, &at_least(&read(0, INT), &int(0)));
    let owing = |aborts: &str| with_clauses(&fields, &holds, &[h(&[], &built(aborts))]);
    let owing_nothing = |aborts: &str| with_clauses(&fields, "", &[h(&[], &built(aborts))]);

    reads_whole(&owing(r#"["INVARIANT_NOT_HELD"]"#));
    reads_whole(&owing_nothing("[]"));
    is_the_halves_disagreeing(&owing("[]"), "states what its values owe");
    is_the_halves_disagreeing(
        &owing_nothing(r#"["INVARIANT_NOT_HELD"]"#),
        "states no clause",
    );
}

/// A clause observes the value being built and builds none, which the checker holds to: so a
/// construction runs clauses that construct nothing in turn.
#[test]
fn a_clause_builds_no_value() {
    let fields = field("count", 0, "INT");
    let built = format!(
        r#"{{"core":"construct","declared":"m.R","values":[{}],"type":{{"ref":{{"is":"declared","declared":"m.R"}}}},"aborts":["INVARIANT_NOT_HELD"]}}"#,
        read(0, INT)
    );
    let read_back = node(
        "field",
        &format!(r#""target":{built},"field":"count""#),
        INT,
    );
    let building = clause(None, &at_least(&read_back, &int(0)));
    is_the_halves_disagreeing(
        &with_clauses(&fields, &building, &[]),
        "constructs m.R, where a clause builds no value",
    );
}

/// A clause this object does not run is read, and held to what the checker holds it to, and is not
/// refused for what this backend cannot lower. This object runs the clauses of a declaration it
/// builds: one whose fields have a representation here, and which a body here constructs or the
/// module publishes, so another build may construct one through this object. A declaration the
/// module keeps and nothing here constructs is built nowhere, whatever its fields are; one whose
/// fields have no representation is built nowhere here either. The same clause on a declaration
/// the module publishes is run, and refused as not lowered.
///
/// The clause makes a function taking a `Rational`, which no lifted function here can take.
#[test]
fn a_clause_of_a_declaration_nothing_here_builds_is_not_run() {
    let date_to_truth = fn_of(&[RATIONAL], BOOL);
    let block = format!(
        r#"{{"core":"block","site":0,"parameters":[{{"binding":1,"name":"x"}}],"body":{},"type":{date_to_truth},"aborts":[]}}"#,
        truth(true)
    );
    let holds = clause(None, &let_(2, &date_to_truth, &block, &truth(true), BOOL));
    let published =
        |document: String| document.replace(r#""publishes":[]"#, r#""publishes":["m.R"]"#);
    let unlaid = r#"{"name":"items","binding":0,"codec":{"is":"setof","element":{"is":"scalar","scalar":"INT"}}}"#;
    let counted = field("count", 0, "INT");

    reads_whole(&published(with_clauses(unlaid, &holds, &[])));
    reads_whole(&with_clauses(&counted, &holds, &[]));

    let refused = object_for(&published(with_clauses(&counted, &holds, &[])))
        .expect_err("the module publishes it, so it is built here and its clause is run");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");

    let disagreeing = clause(None, &let_(2, &date_to_truth, &block, &read(0, BOOL), BOOL));
    is_the_halves_disagreeing(&with_clauses(unlaid, &disagreeing, &[]), "m.R's clause 0");
}

/// A kernel's application states what it takes each argument as, which is the kernel's signature
/// settled for this call, and every argument stands at exactly that: where it is narrower, the
/// argument is a `Widen` saying so. One left narrower without it is a document the checker does not
/// write, and is refused as that. `list.length` takes a list of any element, so a list of a case
/// widened to a list of its sum is one it takes, at the sum.
#[test]
fn a_kernel_argument_stands_at_what_the_application_takes() {
    let length = |argument: &str| {
        let reaches = format!(
            r#"{{"is":"kernel","kernel":"list.length","takes":[{}],"fact":{{"is":"none"}}}}"#,
            list_of(S)
        );
        node(
            "call",
            &format!(r#""reaches":{reaches},"arguments":[{argument}]"#),
            INT,
        )
    };
    reads_whole(&helpers(&[h(
        &[&list_of(A)],
        &length(&widen(&read(0, &list_of(A)), &list_of(S))),
    )]));

    is_the_halves_disagreeing(
        &helpers(&[h(&[&list_of(A)], &length(&read(0, &list_of(A))))]),
        "argument 0 handed to list.length",
    );
}

/// A list literal's type is a list, and every element stands at the element type it says. The
/// empty list is read like any other.
#[test]
fn a_list_holds_what_its_type_says_it_holds() {
    let list = |elements: &[String], ty: &str| {
        node(
            "list",
            &format!(r#""elements":[{}]"#, elements.join(",")),
            ty,
        )
    };
    let of_int = format!(r#"{{"list":{INT}}}"#);

    reads_whole(&helpers(&[h(
        &[INT],
        &list(&[read(0, INT), int(1)], &of_int),
    )]));
    reads_whole(&helpers(&[h(&[INT], &list(&[], &of_int))]));
    is_the_halves_disagreeing(
        &helpers(&[h(&[INT], &list(&[read(0, INT), truth(true)], &of_int))]),
        "element 1 of a list",
    );
    is_the_halves_disagreeing(
        &helpers(&[h(&[INT], &list(&[read(0, INT)], INT))]),
        "which is not a list",
    );
}

/// `list.get` answers an optional of the element of the list it takes, whatever that element is:
/// what the application takes binds the element, and what it answers is held to it.
#[test]
fn list_get_answers_the_element_of_the_list_it_takes() {
    let get = |element: &str, answers: &str| {
        let listed = format!(r#"{{"list":{element}}}"#);
        let reaches = format!(
            r#"{{"is":"kernel","kernel":"list.get","takes":[{INT},{listed}],"fact":{{"is":"none"}}}}"#
        );
        let found = format!(r#"{{"option":{answers}}}"#);
        h(
            &[&listed],
            &node(
                "call",
                &format!(
                    r#""reaches":{reaches},"arguments":[{},{}]"#,
                    int(0),
                    read(0, &listed)
                ),
                &found,
            ),
        )
    };

    reads_whole(&helpers(&[get(INT, INT)]));
    reads_whole(&helpers(&[get(BOOL, BOOL)]));
    is_the_halves_disagreeing(&helpers(&[get(INT, BOOL)]), "a call of list.get");
}

/// `int.add` is lowered as the sum of two `Int`s. What an application says it takes is the
/// checker's statement about that call, and this backend holds it to what it knows of the kernel:
/// both how many it takes and what each is. One that says otherwise is the two halves disagreeing,
/// and is not lowered as though it were the kernel.
#[test]
fn int_add_takes_two_ints_whatever_the_application_says() {
    let added = |takes: &str, arguments: &[String]| {
        let reaches = format!(
            r#"{{"is":"kernel","kernel":"int.add","takes":[{takes}],"fact":{{"is":"none"}}}}"#
        );
        with_outer_aborts(
            &node(
                "call",
                &format!(
                    r#""reaches":{reaches},"arguments":[{}]"#,
                    arguments.join(",")
                ),
                INT,
            ),
            r#""REQUIRED_FORM_HAS_NO_PLACE""#,
        )
    };
    let document = |call: String| helpers(&[h(&[], &call)]);

    reads_whole(&document(added(&format!("{INT},{INT}"), &[int(1), int(2)])));
    // A second `Bool` among what it takes: two arguments, one of them the wrong type.
    is_the_halves_disagreeing(
        &document(added(&format!("{INT},{BOOL}"), &[int(1), truth(true)])),
        "what an application of int.add takes",
    );
    // None at all, and three: as many arguments as the application says, and not what the
    // kernel takes. Each would otherwise be lowered as the sum of the first two or panic on a
    // missing one.
    is_the_halves_disagreeing(&document(added("", &[])), "takes 2 arguments");
    is_the_halves_disagreeing(
        &document(added(
            &format!("{INT},{INT},{INT}"),
            &[int(1), int(2), int(3)],
        )),
        "takes 2 arguments",
    );
}

/// A truth operator and a join are read as their operands stand, and arithmetic as they stand or at
/// their exact values: only a comparison is read in a type. The lowering asks the reading before the
/// operator, so a truth operator claiming to be read in a type is refused here and not lowered as a
/// short circuit over what it does not say it is.
#[test]
fn an_operator_is_read_only_as_the_checker_reads_it() {
    let over = |op: &str, reading: &str, ty: &str| {
        node(
            "binary",
            &format!(
                r#""op":"{op}","reading":{reading},"left":{},"right":{}"#,
                read(0, ty),
                read(1, ty)
            ),
            if matches!(op, "AND" | "OR") { BOOL } else { ty },
        )
    };
    let in_amount = r#"{"is":"in","type":{"ref":{"is":"declared","declared":"m.A"}}}"#;
    let exact = r#"{"is":"exactnumbers"}"#;
    let stands = r#"{"is":"astheystand"}"#;
    let documents = |body: String, ty: &str| helpers(&[h(&[ty, ty], &body)]);

    reads_whole(&documents(over("AND", stands, BOOL), BOOL));
    for reading in [in_amount, exact] {
        is_the_halves_disagreeing(
            &documents(over("AND", reading, BOOL), BOOL),
            "never reads it as",
        );
        is_the_halves_disagreeing(
            &documents(over("CONCAT", reading, STRING), STRING),
            "never reads it as",
        );
    }
    is_the_halves_disagreeing(
        &documents(over("ADD", in_amount, INT), INT),
        "never reads it as",
    );
}

/// A pair read in a type is one the lowering can take apart as that type. A newtype beside what it
/// wraps is read by opening the side that is one; any other pair, a newtype beside a value that
/// states nothing about its own type included, is read by holding both sides as a value of the
/// reading, so each is one. A pair that is neither would have the
/// lowering read a field out of an `Int`, or a token out of a value that carries none.
#[test]
fn a_pair_read_in_a_type_is_one_the_type_takes_apart() {
    let n = r#"{"ref":{"is":"declared","declared":"m.N"}}"#;
    let compared = |reading: &str, left: &str, right: &str| {
        helpers(&[h(
            &[left, right],
            &node(
                "binary",
                &format!(
                    r#""op":"EQ","reading":{{"is":"in","type":{reading}}},"left":{},"right":{}"#,
                    read(0, left),
                    read(1, right)
                ),
                BOOL,
            ),
        )])
    };

    reads_whole(&compared(n, n, INT));
    reads_whole(&compared(n, INT, n));
    reads_whole(&compared(S, S, A));
    reads_whole(&compared(S, A, S));

    // A value that states nothing about its own type is read as the other side's type whatever
    // that is, a newtype included, and is held as it rather than opened. No value of it is ever
    // laid out, so the document is not lowered, and for that value and not for the pair.
    let nothing = r#"{"nothing":{}}"#;
    for reading in [n, S] {
        for (left, right) in [(nothing, reading), (reading, nothing)] {
            let refused = object_for(&compared(reading, left, right))
                .expect_err("no value of Nothing is laid out");
            assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
            assert!(
                refused.to_string().contains("a value of type Nothing"),
                "{refused}"
            );
        }
    }

    is_the_halves_disagreeing(&compared(n, INT, INT), "is not a value of");
    is_the_halves_disagreeing(&compared(n, n, STRING), "is not a value of");
    is_the_halves_disagreeing(&compared(S, INT, S), "is not a value of");
    is_the_halves_disagreeing(&compared(A, S, A), "is not a value of");
}

/// A newtype that wraps itself, directly or through another, has no value, and the checker refuses
/// it where it is written. Every walk that opens a newtype would go round it for ever, so it is
/// refused when the document is read, before a comparison over it asks how far it opens.
#[test]
fn a_newtype_that_wraps_itself_is_the_halves_disagreeing() {
    let newtype = |name: &str, wraps: &str| {
        format!(
            r#"{{"module":"m","name":"{name}","by":"amodule","is":"newtype","field":{{"name":"v","binding":0,"codec":{{"is":"named","named":{{"is":"declared","declared":"m.{wraps}"}}}}}},"invariants":[]}}"#
        )
    };
    let compared = |declarations: &[String], reading: &str| {
        let n = r#"{"ref":{"is":"declared","declared":"m.N"}}"#;
        let body = format!(
            r#"{{"core":"binary","op":"LE","reading":{reading},"left":{},"right":{},"type":{BOOL},"aborts":[]}}"#,
            read(0, n),
            read(1, n)
        );
        format!(
            r#"{{"transport":26,"declarations":[{}],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[{}],"values":[],"entries":[],"definitions":[],"examples":[]}}]}}"#,
            declarations.join(","),
            h(&[n, n], &body)
        )
    };
    let in_n = r#"{"is":"in","type":{"ref":{"is":"declared","declared":"m.N"}}}"#;
    let stands = r#"{"is":"astheystand"}"#;
    for declarations in [
        vec![newtype("N", "N")],
        vec![newtype("N", "M"), newtype("M", "N")],
    ] {
        for reading in [in_n, stands] {
            is_the_halves_disagreeing(&compared(&declarations, reading), "wraps itself");
        }
    }
}

/// Every type a node writes is one the document declares, the ones it carries beside its own
/// included: what a let binds, what an arm reads a value as, what an operator reads its operands in,
/// and what a kernel's application takes and was settled against.
#[test]
fn every_type_a_node_writes_is_one_the_document_declares() {
    let missing = r#"{"ref":{"is":"declared","declared":"m.Missing"}}"#;
    let refuses = |body: String, takes: &[&str]| {
        let refused =
            object_for(&helpers(&[h(takes, &body)])).expect_err("a type nothing declares");
        assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
        assert!(refused.to_string().contains("m.Missing"), "{refused}");
    };

    // What a kernel's application was settled against.
    let ordering = format!(
        r#"{{"is":"kernel","kernel":"list.sort","takes":[],"fact":{{"is":"orderingsubject","type":{missing}}}}}"#
    );
    refuses(call(&ordering, &[], INT), &[]);

    // What an application takes, with no argument of that type to stand beside it.
    let taking = format!(
        r#"{{"is":"kernel","kernel":"list.length","takes":[{missing}],"fact":{{"is":"none"}}}}"#
    );
    refuses(call(&taking, &[int(1)], INT), &[]);

    // What an operator reads its operands in.
    refuses(
        node(
            "binary",
            &format!(
                r#""op":"EQ","reading":{{"is":"in","type":{missing}}},"left":{},"right":{}"#,
                int(1),
                int(2)
            ),
            BOOL,
        ),
        &[],
    );

    // What a let binds.
    refuses(let_(0, missing, &int(1), &int(2), INT), &[]);

    // What an arm reads a value as.
    let optional = option_of(INT);
    refuses(
        node(
            "match",
            &format!(
                r#""subject":{},"arms":[{},{}]"#,
                read(0, &optional),
                arm(r#"{"tests":"held"}"#, Some((1, missing)), &int(1)),
                arm(r#"{"tests":"nothing"}"#, None, &int(2))
            ),
            INT,
        ),
        &[&optional],
    );
}

/// What the checker settles beside what a kernel takes is that kernel's: a pattern belongs to
/// `String.matches` and an ordering subject to the kernels that order. An application of `int.add`
/// carrying either is not one the checker writes, and is refused as that and not lowered as the
/// sum it says it is. What another kernel carries is that kernel's own, and is read but not held
/// where this backend does not lower the kernel.
#[test]
fn a_kernel_settles_what_this_backend_knows_it_settles() {
    let added = |fact: &str| {
        let reaches =
            format!(r#"{{"is":"kernel","kernel":"int.add","takes":[{INT},{INT}],"fact":{fact}}}"#);
        with_outer_aborts(
            &node(
                "call",
                &format!(r#""reaches":{reaches},"arguments":[{},{}]"#, int(1), int(2)),
                INT,
            ),
            r#""REQUIRED_FORM_HAS_NO_PLACE""#,
        )
    };
    let document = |fact: &str| helpers(&[h(&[], &added(fact))]);

    reads_whole(&document(r#"{"is":"none"}"#));
    is_the_halves_disagreeing(
        &document(r#"{"is":"stringmatches","written":"foo","meaning":[{"is":"nothing"}]}"#),
        "settles",
    );
    is_the_halves_disagreeing(
        &document(&format!(r#"{{"is":"orderingsubject","type":{INT}}}"#)),
        "settles",
    );

    // A kernel this backend does not lower is refused as not lowered, whatever it settles. Every
    // kernel over types this backend lays out is lowered, so the one here is a key the standard
    // library does not declare, over a list of `Int`s: what refuses it is the kernel and not a
    // type it takes or answers.
    let ints = list_of(INT);
    let shuffling = node(
        "call",
        &format!(
            r#""reaches":{{"is":"kernel","kernel":"list.shuffle","takes":[{ints}],"fact":{{"is":"orderingsubject","type":{INT}}}}},"arguments":[{}]"#,
            read(0, &ints)
        ),
        &ints,
    );
    let refused =
        object_for(&helpers(&[h(&[&ints], &shuffling)])).expect_err("a kernel nothing here lowers");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(
        refused
            .to_string()
            .contains("a call to the kernel list.shuffle"),
        "{refused}"
    );
}

/// A kernel's call on the wire: the kernel it reaches, what the application takes, and what else
/// it settled.
fn kernel(key: &str, takes: &[&str], fact: &str) -> String {
    format!(
        r#"{{"is":"kernel","kernel":"{key}","takes":[{}],"fact":{fact}}}"#,
        takes.join(",")
    )
}

fn ordering_subject(ty: &str) -> String {
    format!(r#"{{"is":"orderingsubject","type":{ty}}}"#)
}

const NO_FACT: &str = r#"{"is":"none"}"#;

/// A list of a type no value of which is made is empty, so a kernel ordering one compares nothing
/// and is lowered whatever that type is, where comparing two of it would be refused.
#[test]
fn a_list_no_value_of_whose_element_is_made_is_ordered_without_a_comparison() {
    for element in [r#"{"nothing":{}}"#, r#"{"never":{}}"#] {
        let listed = list_of(element);
        for (key, answers) in [
            ("list.sort", listed.clone()),
            ("list.max", option_of(element)),
        ] {
            reads_whole(&helpers(&[h(
                &[&listed],
                &call(
                    &kernel(key, &[&listed], &ordering_subject(element)),
                    &[read(0, &listed)],
                    &answers,
                ),
            )]));
        }
    }
}

/// A function over what has no value is still handed to a kernel, as a block made with no code,
/// and the kernel answers for the empty list or the absent value beside it without calling it.
#[test]
fn a_kernel_is_handed_a_function_that_never_runs_and_calls_none() {
    let nothing = r#"{"nothing":{}}"#;
    let never_run = |answers: &str, body: String| {
        let ty = fn_of(&[nothing], answers);
        let block = node(
            "block",
            &format!(r#""site":0,"parameters":[{{"binding":1,"name":"x"}}],"body":{body}"#),
            &ty,
        );
        (ty, block)
    };
    let empty = (
        list_of(nothing),
        node("list", r#""elements":[]"#, &list_of(nothing)),
    );
    let absent = (
        option_of(nothing),
        format!(
            r#"{{"core":"none","type":{},"aborts":[]}}"#,
            option_of(nothing)
        ),
    );
    for (key, (function, block), (beside, handed), answers, fact) in [
        (
            "list.find",
            never_run(BOOL, truth(true)),
            empty.clone(),
            option_of(nothing),
            NO_FACT.to_string(),
        ),
        (
            "list.sortBy",
            never_run(INT, int(1)),
            empty,
            list_of(nothing),
            ordering_subject(INT),
        ),
        (
            "option.map",
            never_run(INT, int(1)),
            absent,
            option_of(INT),
            NO_FACT.to_string(),
        ),
    ] {
        reads_whole(&helpers(&[h(
            &[],
            &call(
                &kernel(key, &[&function, &beside], &fact),
                &[block, handed],
                &answers,
            ),
        )]));
    }
}

/// A kernel handed a function is held to one type for each of the contract's variables wherever
/// it stands: `List.find`'s predicate takes the list's element, and `Option.map`'s function takes
/// what the optional holds and answers what the answer holds. A predicate over another type than
/// the list holds would be handed each element as what it is not.
#[test]
fn a_function_a_kernel_takes_is_held_to_what_the_rest_of_the_call_binds() {
    let ints = list_of(INT);
    let find = |element: &str| {
        let predicate = fn_of(&[element], BOOL);
        h(
            &[&predicate, &ints],
            &call(
                &kernel("list.find", &[&predicate, &ints], NO_FACT),
                &[read(0, &predicate), read(1, &ints)],
                &option_of(INT),
            ),
        )
    };
    reads_whole(&helpers(&[find(INT)]));
    is_the_halves_disagreeing(
        &helpers(&[find(STRING)]),
        "what an application of list.find takes",
    );

    let optional = option_of(INT);
    let map = |answers: &str| {
        let function = fn_of(&[INT], STRING);
        h(
            &[&function, &optional],
            &call(
                &kernel("option.map", &[&function, &optional], NO_FACT),
                &[read(0, &function), read(1, &optional)],
                &option_of(answers),
            ),
        )
    };
    reads_whole(&helpers(&[map(STRING)]));
    is_the_halves_disagreeing(&helpers(&[map(INT)]), "a call of option.map");
}

/// What an ordering was checked against is what a sort compares by, so it is the one type the
/// kernel orders: what `sortBy`'s key answers, and the element of the list `sort`, `max` and `min`
/// take. One that is another type is the two halves disagreeing, and is not lowered as a
/// comparison of that type over values of this one.
#[test]
fn what_a_kernel_orders_by_is_what_it_takes_orders() {
    let ints = list_of(INT);
    let sort_by = |subject: &str| {
        let key = fn_of(&[INT], STRING);
        h(
            &[&key, &ints],
            &call(
                &kernel("list.sortBy", &[&key, &ints], &ordering_subject(subject)),
                &[read(0, &key), read(1, &ints)],
                &ints,
            ),
        )
    };
    reads_whole(&helpers(&[sort_by(STRING)]));
    is_the_halves_disagreeing(
        &helpers(&[sort_by(INT)]),
        "what an application of list.sortBy orders by",
    );

    for (key, answers) in [
        ("list.sort", ints.clone()),
        ("list.max", option_of(INT)),
        ("list.min", option_of(INT)),
    ] {
        let ordering = |subject: &str| {
            h(
                &[&ints],
                &call(
                    &kernel(key, &[&ints], &ordering_subject(subject)),
                    &[read(0, &ints)],
                    &answers,
                ),
            )
        };
        reads_whole(&helpers(&[ordering(INT)]));
        is_the_halves_disagreeing(
            &helpers(&[ordering(STRING)]),
            &format!("what an application of {key} orders by"),
        );
        // What orders is a fact the kernel settles, and one settling none is not this kernel's.
        is_the_halves_disagreeing(
            &helpers(&[h(
                &[&ints],
                &call(&kernel(key, &[&ints], NO_FACT), &[read(0, &ints)], &answers),
            )]),
            "settles",
        );
    }
}

/// What a pattern is said to mean is what some pattern reads as: parts naming only parts written
/// before them, and runs of scalar values in order and apart. Anything else is not a reading the
/// checker makes, and is refused as the two halves disagreeing. Every reading is one this backend
/// runs, whatever it counts: a count is a number the machine holds, so counts inside counts are no
/// larger a machine than the reading they are written in.
#[test]
fn a_pattern_is_said_to_mean_what_a_pattern_reads_as() {
    let document = |meaning: &str| {
        let matching = node(
            "call",
            &format!(
                r#""reaches":{{"is":"kernel","kernel":"string.matches","takes":[{STRING},{STRING}],"fact":{{"is":"stringmatches","written":"p","meaning":{meaning}}}}},"arguments":[{},{}]"#,
                read(0, STRING),
                read(1, STRING)
            ),
            BOOL,
        );
        helpers(&[h(&[STRING, STRING], &matching)])
    };

    reads_whole(&document(r#"[{"is":"symbols","ranges":[[48,57]]}]"#));
    reads_whole(&document(
        r#"[{"is":"symbols","ranges":[[97,97]]},{"is":"repeated","what":0,"least":0,"most":null}]"#,
    ));
    for unread in [
        r#"[]"#,
        r#"[{"is":"inturn","parts":[0]}]"#,
        r#"[{"is":"symbols","ranges":[[55296,57343]]}]"#,
        r#"[{"is":"symbols","ranges":[[5,9],[10,12]]}]"#,
        r#"[{"is":"nothing"},{"is":"eitherof","arms":[0]}]"#,
        r#"[{"is":"nothing"},{"is":"repeated","what":0,"least":3,"most":2}]"#,
    ] {
        is_the_halves_disagreeing(&document(unread), "no reading of a pattern");
    }

    let counted = |what: usize, times: u32| {
        format!(r#"{{"is":"repeated","what":{what},"least":{times},"most":{times}}}"#)
    };
    reads_whole(&document(&format!(
        r#"[{{"is":"symbols","ranges":[[97,97]]}},{}]"#,
        counted(0, 1 << 20)
    )));
    reads_whole(&document(&format!(
        r#"[{{"is":"symbols","ranges":[[97,97]]}},{},{},{}]"#,
        counted(0, 1000),
        counted(1, 1000),
        counted(2, 1000)
    )));
}

/// A node names a reason to end a run without a value only where its kind has one. A literal, a
/// read, a fork, a call to a helper and every other kind that ends no run of its own naming one is a
/// document the checker does not write, and the lowering would not notice it.
#[test]
fn only_the_kinds_that_can_end_a_run_name_a_reason_to() {
    let with_reason = |body: String| with_outer_aborts(&body, r#""DIVISION_BY_ZERO""#);

    reads_whole(&helpers(&[h(&[INT], &read(0, INT))]));
    for body in [
        int(1),
        read(0, INT),
        node(
            "if",
            &format!(
                r#""cond":{},"then":{},"else":{}"#,
                truth(true),
                int(1),
                int(2)
            ),
            INT,
        ),
        widen(&unit("m.A"), S),
    ] {
        is_the_halves_disagreeing(
            &helpers(&[h(&[INT], &with_reason(body))]),
            "ends no run without a value",
        );
    }
    // A call to a helper ends with what the helper ends with, and names none of its own.
    let g = helper("m.g", &[INT], &read(0, INT));
    let reaches = &format!(r#"{{"is":"helper","reached":{}}}"#, own("m.g"));
    let called = with_reason(call(reaches, &[int(1)], INT));
    is_the_halves_disagreeing(
        &helpers(&[g, h(&[], &called)]),
        "ends no run without a value",
    );
}

/// What a binary operator can end a run for is decided by which operator it is: arithmetic may, and
/// a comparison, a truth operator and a join never do. One of those naming a reason is a document
/// the checker does not write, and is refused as that: the lowering of a comparison does not read
/// a reason at all, so a document that got past here would be made into an object.
#[test]
fn only_arithmetic_names_a_reason_to_end_a_run() {
    let over = |op: &str, ty: &str, answers: &str| {
        node(
            "binary",
            &format!(
                r#""op":"{op}","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
                read(0, ty),
                read(1, ty)
            ),
            answers,
        )
    };
    let with_reason = |body: String| with_outer_aborts(&body, r#""DIVISION_BY_ZERO""#);

    for (op, ty, answers) in [
        ("EQ", INT, BOOL),
        ("NE", INT, BOOL),
        ("LT", INT, BOOL),
        ("GE", INT, BOOL),
        ("AND", BOOL, BOOL),
        ("OR", BOOL, BOOL),
        ("CONCAT", STRING, STRING),
    ] {
        reads_whole(&helpers(&[h(&[ty, ty], &over(op, ty, answers))]));
        is_the_halves_disagreeing(
            &helpers(&[h(&[ty, ty], &with_reason(over(op, ty, answers)))]),
            "ends no run without a value",
        );
    }
}

/// What a negation can end a run for is decided by the type it answers and not by what it negates:
/// the smallest `Int` has no counterpart, a literal's included, so a negation of an `Int` names one
/// reason whatever its operand is. That the lowering folds a literal's sign says nothing of what the
/// checker states.
#[test]
fn a_negation_of_an_int_names_its_reason_whatever_it_negates() {
    let negated = |operand: String, reasons: &str| {
        with_outer_aborts(
            &node("neg", &format!(r#""operand":{operand}"#), INT),
            reasons,
        )
    };
    let one = r#""REQUIRED_FORM_HAS_NO_PLACE""#;

    reads_whole(&helpers(&[h(&[INT], &negated(int(5), one))]));
    reads_whole(&helpers(&[h(&[INT], &negated(read(0, INT), one))]));
    // None, and another reason: what is named is the reason itself and not how many there are, and
    // the lowering turns the one it is given into the status the run ends with.
    for wrong in ["", r#""DIVISION_BY_ZERO""#] {
        is_the_halves_disagreeing(
            &helpers(&[h(&[INT], &negated(int(5), wrong))]),
            "a negation of Int",
        );
        is_the_halves_disagreeing(
            &helpers(&[h(&[INT], &negated(read(0, INT), wrong))]),
            "a negation of Int",
        );
    }
    // A magnitude the checker never writes, which the lowering would negate as it stands.
    is_the_halves_disagreeing(
        &helpers(&[h(&[INT], &negated(int(i64::MIN), one))]),
        "no magnitude the checker writes",
    );
}

/// A type another build builds carries what its clauses are answered under and not the clauses, so
/// whether a construction of one names the one reason a clause can end it for is held as it is for
/// a type of this build's: it names that reason exactly where the type states a clause, and never
/// another.
#[test]
fn a_construction_of_another_builds_type_names_the_reason_its_clauses_give() {
    let document = |headers: &str, aborts: &str| {
        let built = with_outer_aborts(
            &node(
                "construct",
                &format!(r#""declared":"m.R","values":[{}]"#, int(1)),
                r#"{"ref":{"is":"declared","declared":"m.R"}}"#,
            ),
            aborts,
        );
        format!(
            concat!(
                r#"{{"transport":26,"declarations":["#,
                r#"{{"module":"m","name":"R","by":"onthepath","is":"product","#,
                r#""fields":[{}],"headers":[{}]}}],"behaviors":[],"#,
                r#""modules":[{{"name":"m","publishes":[],"helpers":[{}],"values":[],"#,
                r#""entries":[],"definitions":[],"examples":[]}}]}}"#
            ),
            field("count", 0, "INT"),
            headers,
            h(&[], &built)
        )
    };
    let stated = r#"{"name":"counted"}"#;

    reads_whole(&document("", ""));
    reads_whole(&document(stated, r#""INVARIANT_NOT_HELD""#));
    is_the_halves_disagreeing(&document("", r#""INVARIANT_NOT_HELD""#), "states no clause");
    is_the_halves_disagreeing(&document(stated, ""), "states what its values owe");
    is_the_halves_disagreeing(
        &document(stated, r#""DIVISION_BY_ZERO""#),
        "states what its values owe",
    );
}

/// A binding's number is its identity and not its position, so a document numbering its binders with
/// the largest numbers there are is a document like any other. A table sized by the number would take
/// as much room as the largest one, and the writer only keeps them small by counting.
#[test]
fn a_binding_is_an_identity_and_not_a_position() {
    let huge = usize::MAX;
    let bound = let_(huge, INT, &int(1), &read(huge, INT), INT);
    reads_whole(&helpers(&[h(&[], &bound)]));

    // A field a clause reads, and the construction that runs the clause over it.
    let counted = field("count", huge, "INT");
    let holds = clause(Some("counted"), &at_least(&read(huge, INT), &int(0)));
    let built = with_outer_aborts(
        &node(
            "construct",
            &format!(r#""declared":"m.R","values":[{}]"#, int(1)),
            r#"{"ref":{"is":"declared","declared":"m.R"}}"#,
        ),
        r#""INVARIANT_NOT_HELD""#,
    );
    reads_whole(&with_clauses(&counted, &holds, &[h(&[], &built)]));
}

/// A number names one binder in a body. The lowering has one place for a binding to stand in, and
/// a binder that took the number of one in force would leave its value there for what is read after
/// its scope has closed, so `(let 0 = 1 in 0) + 0` would lower as `1 + 1` and not as `1` and the
/// parameter. A document doing it is refused as the two halves disagreeing, for a `let`, for a
/// match arm, and for a function value's parameter alike, and a number in force again once its
/// binder's scope has closed is a number free for another binder to take.
#[test]
fn a_number_names_one_binder_in_force() {
    let adding = |left: String| {
        with_outer_aborts(
            &node(
                "binary",
                &format!(
                    r#""op":"ADD","reading":{{"is":"astheystand"}},"left":{left},"right":{}"#,
                    read(0, INT)
                ),
                INT,
            ),
            r#""REQUIRED_FORM_HAS_NO_PLACE""#,
        )
    };

    // A `let` taking the number of the parameter in force.
    is_the_halves_disagreeing(
        &helpers(&[h(
            &[INT],
            &adding(let_(0, INT, &int(1), &read(0, INT), INT)),
        )]),
        "already in force",
    );
    // The same `let` under a number nothing has, and one taking a number free again after the
    // scope of an earlier `let` closed.
    reads_whole(&helpers(&[h(
        &[INT],
        &adding(let_(1, INT, &int(1), &read(1, INT), INT)),
    )]));
    let twice = with_outer_aborts(
        &node(
            "binary",
            &format!(
                r#""op":"ADD","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
                let_(1, INT, &int(1), &read(1, INT), INT),
                let_(1, INT, &int(2), &read(1, INT), INT)
            ),
            INT,
        ),
        r#""REQUIRED_FORM_HAS_NO_PLACE""#,
    );
    reads_whole(&helpers(&[h(&[INT], &twice)]));
    // A `let` inside another that took its number.
    is_the_halves_disagreeing(
        &helpers(&[h(
            &[INT],
            &let_(
                1,
                INT,
                &int(1),
                &let_(1, INT, &int(2), &read(1, INT), INT),
                INT,
            ),
        )]),
        "already in force",
    );

    // A match arm taking the number of the parameter.
    let optional = option_of(INT);
    let forking = |binding: usize| {
        node(
            "match",
            &format!(
                r#""subject":{},"arms":[{},{}]"#,
                read(1, &optional),
                arm(
                    r#"{"tests":"held"}"#,
                    Some((binding, INT)),
                    &read(binding, INT)
                ),
                arm(r#"{"tests":"nothing"}"#, None, &read(0, INT))
            ),
            INT,
        )
    };
    is_the_halves_disagreeing(
        &helpers(&[h(&[INT, &optional], &forking(0))]),
        "already in force",
    );
    reads_whole(&helpers(&[h(&[INT, &optional], &forking(2))]));

    // Two parameters of one function value under one number.
    let block = |first: usize, second: usize| {
        let function = fn_of(&[INT, INT], INT);
        node(
            "block",
            &format!(
                r#""site":0,"parameters":[{{"binding":{first},"name":"a"}},{{"binding":{second},"name":"b"}}],"body":{}"#,
                read(first, INT)
            ),
            &function,
        )
    };
    is_the_halves_disagreeing(&helpers(&[h(&[], &block(3, 3))]), "already in force");
}

/// An arm says what it reads its value as exactly where it binds one. The writer says both or
/// neither, and an arm saying only `binds` is a statement the lowering would drop.
#[test]
fn an_arm_binds_and_says_what_it_reads_it_as_together() {
    let optional = option_of(INT);
    let forking = |arm_of: String| {
        node(
            "match",
            &format!(
                r#""subject":{},"arms":[{arm_of},{}]"#,
                read(0, &optional),
                arm(r#"{"tests":"nothing"}"#, None, &int(2))
            ),
            INT,
        )
    };
    let document = |arm_of: String| helpers(&[h(&[&optional], &forking(arm_of))]);
    let held = |binding: &str, binds: &str| {
        format!(
            r#"{{"selects":[{{"tests":"held"}}],"binding":{binding},"binds":{binds},"body":{}}}"#,
            int(1)
        )
    };

    reads_whole(&document(held("1", INT)));
    reads_whole(&document(held("null", "null")));
    is_the_halves_disagreeing(&document(held("null", INT)), "binds nothing");
    is_the_halves_disagreeing(&document(held("1", "null")), "does not say");
}

/// What a value is handed is another value this module builds. The lowering reads the type of the
/// handover and never what it carries, so a handover carrying nothing built would be a statement no
/// reader held.
#[test]
fn a_handover_carries_a_value_the_module_builds() {
    let value = |carries: &str| {
        format!(
            r#"{{"transport":26,"declarations":[],"behaviors":[],"modules":[{{"name":"m","publishes":[],"helpers":[],"values":[{{"module":"m","name":"ks","handovers":[],"body":{}}},{{"module":"m","name":"ys","handovers":[{{"parameter":"dep","type":{INT},"carries":{{"module":"m","name":"{carries}"}}}}],"body":{}}}],"entries":[],"definitions":[],"examples":[]}}]}}"#,
            int(1),
            read(0, INT)
        )
    };
    reads_whole(&value("ks"));
    is_the_halves_disagreeing(&value("nothing"), "builds no value of");
}

/// What a target answering as `is` takes, one `input`: under the name `a` where it is declared, which
/// every clause here relates, and by its place alone where it is a composition, which declares none.
fn taking(is: &str, input: &str) -> String {
    if is == "composed" {
        format!(r#"{{"positional":[{input}]}}"#)
    } else {
        format!(r#"{{"named":[{{"name":"a","input":{input}}}]}}"#)
    }
}

/// `m.b`, taking one `Int`, answering what `output` says, answering as `is` says, and whose answer
/// is held as `ensures` says.
fn held(is: &str, output: &str, ensures: &str) -> String {
    format!(
        r#"{{"module":"m","name":"b","is":"{is}","parameters":{},"output":{output},"requirements":[],"ensures":{ensures}}}"#,
        taking(is, r#"{"is":"scalar","scalar":"INT"}"#)
    )
}

const ANSWERS_INT: &str = r#"{"is":"scalar","scalar":"INT"}"#;
const ANSWERS_S: &str = r#"{"is":"nominal","named":{"is":"declared","declared":"m.S"}}"#;
const ALWAYS: &str = r#"{"is":"always"}"#;

/// Held `at` the place named, by these rules over the one parameter `a`.
fn held_at(at: &str, parameters: &[&str], rules: &[String]) -> String {
    let parameters: Vec<String> = parameters.iter().map(|it| format!(r#""{it}""#)).collect();
    format!(
        r#"{{"at":"{at}","contract":{{"parameters":[{}],"rules":[{}]}}}}"#,
        parameters.join(","),
        rules.join(",")
    )
}

fn rule(guard: &str, value: usize, condition: &str) -> String {
    format!(
        r#"{{"guard":{guard},"value":{value},"condition":{condition},"readsanswer":true,"clause":null}}"#
    )
}

/// What the answer read under `value` is at least: the answer against the parameter.
fn answer_at_least_a(value: usize) -> String {
    node(
        "binary",
        &format!(
            r#""op":"GE","reading":{{"is":"astheystand"}},"left":{},"right":{}"#,
            read(value, INT),
            read(0, INT)
        ),
        BOOL,
    )
}

/// `m.b`'s body, answering `body`.
fn defined(body: &str) -> String {
    format!(
        r#"{{"is":"body","declared":"m.b","parameters":["a"],"publication":"kept","body":{body}}}"#
    )
}

/// Where an answer is held is a place the behavior has: the callee, for a body this object holds,
/// and each crossing, for an answer supplied from outside. Either one the other way round is a
/// check nothing would run where it was placed.
#[test]
fn an_answer_is_held_where_the_behavior_has_a_place_for_it() {
    let rules = held_at("callee", &["a"], &[rule(ALWAYS, 1, &answer_at_least_a(1))]);
    reads_whole(&document(
        &[held("body", ANSWERS_INT, &rules)],
        &[],
        &[defined(&read(0, INT))],
    ));
    is_the_halves_disagreeing(
        &document(&[held("injected", ANSWERS_INT, &rules)], &[], &[]),
        "m.b",
    );

    let rules = held_at(
        "crossing",
        &["a"],
        &[rule(ALWAYS, 1, &answer_at_least_a(1))],
    );
    reads_whole(&document(
        &[held("injected", ANSWERS_INT, &rules)],
        &[],
        &[],
    ));
    is_the_halves_disagreeing(
        &document(
            &[held("body", ANSWERS_INT, &rules)],
            &[],
            &[defined(&read(0, INT))],
        ),
        "m.b",
    );
}

/// A composition carries no rule (spec §a-composition-carries-no-ensures): it names no parameter a
/// rule could relate its answer to.
#[test]
fn a_composition_holds_its_answer_to_nothing() {
    let stage = r#"{"module":"m","name":"c","is":"body","parameters":{"named":[{"name":"a","input":{"is":"scalar","scalar":"INT"}}]},"output":{"is":"scalar","scalar":"INT"},"requirements":[],"ensures":{"at":"none"}}"#.to_string();
    let stage_body = format!(
        r#"{{"is":"body","declared":"m.c","parameters":["a"],"publication":"kept","body":{}}}"#,
        read(0, INT)
    );
    let composed = r#"{"is":"composed","declared":"m.b","publication":"kept","stages":[{"behavior":"m.c","routing":{"is":"always"}}]}"#.to_string();
    let rules = held_at("callee", &["a"], &[rule(ALWAYS, 1, &answer_at_least_a(1))]);
    reads_whole(&document(
        &[
            held("composed", ANSWERS_INT, r#"{"at":"none"}"#),
            stage.clone(),
        ],
        &[],
        &[composed.clone(), stage_body.clone()],
    ));
    is_the_halves_disagreeing(
        &document(
            &[held("composed", ANSWERS_INT, &rules), stage],
            &[],
            &[composed, stage_body],
        ),
        "m.b",
    );
}

/// What is done about a behavior's rule is decided for every behavior of a module this document
/// builds and for no other. Undecided for one of its own is a table nobody filled; decided for
/// another build's is this document deciding what that build's clause is.
#[test]
fn a_rule_is_decided_for_this_documents_behaviors_and_no_others() {
    is_the_halves_disagreeing(
        &document(
            &[held("body", ANSWERS_INT, r#"{"at":"undecided"}"#)],
            &[],
            &[defined(&read(0, INT))],
        ),
        "m.b",
    );
    let foreign = |ensures: &str| {
        format!(
            r#"{{"module":"other","name":"b","is":"elsewhere","parameters":{{"named":[]}},"output":{ANSWERS_INT},"requirements":[],"ensures":{ensures}}}"#
        )
    };
    reads_whole(&document(&[foreign(r#"{"at":"undecided"}"#)], &[], &[]));
    is_the_halves_disagreeing(
        &document(&[foreign(r#"{"at":"none"}"#)], &[], &[]),
        "other.b",
    );
}

/// A rule relates the parameters its behavior takes, since it reads each under where it stands
/// among them: the clause's names and the declaration's are one list crossed twice.
#[test]
fn a_rule_names_the_parameters_its_behavior_takes() {
    let rules = held_at(
        "callee",
        &["a", "z"],
        &[rule(ALWAYS, 1, &answer_at_least_a(1))],
    );
    is_the_halves_disagreeing(
        &document(
            &[held("body", ANSWERS_INT, &rules)],
            &[],
            &[defined(&read(0, INT))],
        ),
        r#"its ensures relates ["a", "z"]"#,
    );
}

/// What has to hold is a truth.
#[test]
fn a_rule_is_a_truth() {
    let rules = held_at("callee", &["a"], &[rule(ALWAYS, 1, &read(1, INT))]);
    is_the_halves_disagreeing(
        &document(
            &[held("body", ANSWERS_INT, &rules)],
            &[],
            &[defined(&read(0, INT))],
        ),
        "where a clause is a truth",
    );
}

/// A rule reads the answer under a number of its own, which is not a parameter's.
#[test]
fn a_rule_reads_the_answer_under_a_number_no_parameter_has() {
    let rules = held_at("callee", &["a"], &[rule(ALWAYS, 0, &answer_at_least_a(0))]);
    is_the_halves_disagreeing(
        &document(
            &[held("body", ANSWERS_INT, &rules)],
            &[],
            &[defined(&read(0, INT))],
        ),
        "already in force",
    );
}

/// A rule over a case tests the answer for a case of it, and reads the answer as that case: read
/// as the sum it was tested out of, it is two statements of one binding that disagree. A case of an
/// answer that has none is a test of nothing the answer can be.
#[test]
fn a_rule_over_a_case_reads_the_answer_as_its_guard_says() {
    let case = |binds: &str| {
        format!(
            r#"{{"is":"case","selects":{},"binds":{binds}}}"#,
            which(&["m.A"])
        )
    };
    // The answer read under 1 as `read_as`, and nothing asked of it but that it is there.
    let reading = |read_as: &str| let_(2, read_as, &read(1, read_as), &truth(true), BOOL);
    let rules = |binds: &str, read_as: &str| {
        held_at(
            "callee",
            &["a"],
            &[rule(&case(binds), 1, &reading(read_as))],
        )
    };
    let answer = defined(&widen(&unit("m.A"), S));
    reads_whole(&document(
        &[held("body", ANSWERS_S, &rules(A, A))],
        &[],
        std::slice::from_ref(&answer),
    ));
    is_the_halves_disagreeing(
        &document(&[held("body", ANSWERS_S, &rules(A, S))], &[], &[answer]),
        "m.b",
    );
    is_the_halves_disagreeing(
        &document(
            &[held("body", ANSWERS_INT, &rules(A, A))],
            &[],
            &[defined(&read(0, INT))],
        ),
        "m.b",
    );
}

/// `m.R` attempted from the helper's `Int` parameter, bound under 1 where every clause holds and
/// read back as its `count`, and each of `departures` answering an `Int`.
fn attempted(departures: &[(Option<&str>, &str)]) -> String {
    attempt(&read(0, INT), &field_of_r(1), departures)
}

fn attempt(count: &str, then: &str, departures: &[(Option<&str>, &str)]) -> String {
    let departures: Vec<String> = departures
        .iter()
        .map(|(clause, body)| {
            let clause = clause.map_or("null".to_string(), |it| format!(r#""{it}""#));
            format!(r#"{{"clause":{clause},"body":{body}}}"#)
        })
        .collect();
    node(
        "attempt",
        &format!(
            r#""declared":"m.R","values":[{count}],"binding":1,"binds":{{"ref":{{"is":"declared","declared":"m.R"}}}},"then":{then},"departures":[{}]"#,
            departures.join(",")
        ),
        INT,
    )
}

/// The `count` of the `m.R` bound under `binding`.
fn field_of_r(binding: usize) -> String {
    node(
        "field",
        &format!(
            r#""target":{},"field":"count""#,
            read(binding, r#"{"ref":{"is":"declared","declared":"m.R"}}"#)
        ),
        INT,
    )
}

/// `m.R` stating a named clause and one with no name, and a helper attempting it.
fn attempting(body: &str) -> String {
    attempting_under(&[Some("counted"), None], body)
}

/// `m.R` stating one clause for each of `names`, under that name or none, and a helper attempting
/// it.
fn attempting_under(names: &[Option<&str>], body: &str) -> String {
    let clauses: Vec<String> = names
        .iter()
        .map(|name| clause(*name, &at_least(&read(0, INT), &int(0))))
        .collect();
    with_clauses(
        &field("count", 0, "INT"),
        &clauses.join(","),
        &[h(&[INT], body)],
    )
}

/// Every clause of what is attempted is answered by one departure, by the checker's rule: each
/// clause with a name by the arm naming it, and the clauses with no name by the arm naming none. A
/// departure naming what the type does not state, two answering one clause, and a clause nothing
/// answers are each a document the checker could not have written.
#[test]
fn every_clause_of_what_is_attempted_is_answered_by_one_departure() {
    let minus = |n: i64| node("int", &format!(r#""value":{}"#, -n), INT);
    reads_whole(&attempting(&attempted(&[
        (Some("counted"), &minus(1)),
        (None, &minus(2)),
    ])));
    // The arms in another order are the same arms.
    reads_whole(&attempting(&attempted(&[
        (None, &minus(2)),
        (Some("counted"), &minus(1)),
    ])));

    is_the_halves_disagreeing(
        &attempting(&attempted(&[(Some("roomy"), &minus(1)), (None, &minus(2))])),
        "the clause roomy, which it does not state",
    );
    is_the_halves_disagreeing(
        &attempting(&attempted(&[
            (Some("counted"), &minus(1)),
            (Some("counted"), &minus(3)),
            (None, &minus(2)),
        ])),
        "two departures answer the clause counted",
    );
    is_the_halves_disagreeing(
        &attempting(&attempted(&[
            (Some("counted"), &minus(1)),
            (None, &minus(2)),
            (None, &minus(3)),
        ])),
        "two departures answer the clauses that have no name",
    );
    is_the_halves_disagreeing(
        &attempting(&attempted(&[(Some("counted"), &minus(1))])),
        "its clause 1, which has no name, is answered by no departure",
    );
}

/// The arm naming no clause answers the clauses with no name and nothing else, where arms name
/// clauses beside it: a clause with a name that no arm names is not answered by it, and where every
/// clause has a name it answers nothing. Both are refused by the checker (E2015, E2017), and read
/// as an arm to fall back on they would be run.
#[test]
fn the_arm_naming_no_clause_answers_only_the_clauses_with_no_name() {
    let minus = |n: i64| node("int", &format!(r#""value":{}"#, -n), INT);
    let two_named_and_one_not = [Some("a"), Some("b"), None];
    is_the_halves_disagreeing(
        &attempting_under(
            &two_named_and_one_not,
            &attempted(&[(Some("a"), &minus(1)), (None, &minus(3))]),
        ),
        "its clause b is answered by no departure",
    );
    reads_whole(&attempting_under(
        &two_named_and_one_not,
        &attempted(&[
            (Some("a"), &minus(1)),
            (Some("b"), &minus(2)),
            (None, &minus(3)),
        ]),
    ));

    let every_one_named = [Some("a"), Some("b")];
    is_the_halves_disagreeing(
        &attempting_under(
            &every_one_named,
            &attempted(&[
                (Some("a"), &minus(1)),
                (Some("b"), &minus(2)),
                (None, &minus(3)),
            ]),
        ),
        "every clause has one",
    );
    reads_whole(&attempting_under(
        &every_one_named,
        &attempted(&[(Some("a"), &minus(1)), (Some("b"), &minus(2))]),
    ));
}

/// One departure naming no clause is one value for any failure, which is what `else e` and a lone
/// `| _ -> e` both are to the checker: it answers every clause, named or not.
#[test]
fn one_departure_naming_no_clause_answers_every_clause() {
    let minus = |n: i64| node("int", &format!(r#""value":{}"#, -n), INT);
    reads_whole(&attempting(&attempted(&[(None, &minus(2))])));
    reads_whole(&attempting_under(
        &[Some("a"), Some("b")],
        &attempted(&[(None, &minus(2))]),
    ));
}

/// Two clauses of one declaration under one name would be one arm for two rules, which the checker
/// refuses.
#[test]
fn a_declaration_states_each_clause_name_once() {
    is_the_halves_disagreeing(
        &attempting_under(&[Some("a"), Some("a")], &int(0)),
        "states two clauses both named a",
    );
}

/// What is built is bound where every clause held, and nowhere else: a departure is taken where
/// nothing was, and the fields are worked out before anything is.
#[test]
fn what_an_attempt_builds_is_read_only_where_it_was_built() {
    let taken = |departure: &str| {
        attempting(&attempt(
            &read(0, INT),
            &field_of_r(1),
            &[(Some("counted"), departure), (None, &int(0))],
        ))
    };
    reads_whole(&taken(&int(0)));
    is_the_halves_disagreeing(&taken(&field_of_r(1)), "nothing in scope binds");
    is_the_halves_disagreeing(
        &attempting(&attempt(&field_of_r(1), &int(0), &[(None, &int(0))])),
        "nothing in scope binds",
    );
}

/// What is bound is what is built, at the declaration's own type.
#[test]
fn an_attempt_binds_what_it_builds() {
    let rebound = attempted(&[(None, &int(0))]).replace(
        r#""binds":{"ref":{"is":"declared","declared":"m.R"}}"#,
        r#""binds":{"prim":"INT"}"#,
    );
    is_the_halves_disagreeing(&attempting(&rebound), "binds");
}

/// An attempt ends no run: a clause that does not hold takes a departure, and a clause that does
/// not answer is that clause's own site. So it names no reason, as the checker files it.
#[test]
fn an_attempt_names_no_reason_to_end_a_run() {
    let ends = with_outer_aborts(&attempted(&[(None, &int(0))]), r#""INVARIANT_NOT_HELD""#);
    is_the_halves_disagreeing(&attempting(&ends), "ends no run without a value");
}

/// A type that states no clause has no departure to take, and the checker refuses to attempt one.
#[test]
fn a_type_that_states_no_clause_is_not_attempted() {
    let document = with_clauses(
        &field("count", 0, "INT"),
        "",
        &[h(&[INT], &attempted(&[(None, &int(0))]))],
    );
    is_the_halves_disagreeing(&document, "states no clause");
}

/// A clause builds no value, by attempting one no more than by constructing one.
#[test]
fn a_clause_attempts_nothing() {
    let attempts = clause(
        Some("counted"),
        &node(
            "attempt",
            &format!(
                r#""declared":"m.R","values":[{}],"binding":1,"binds":{{"ref":{{"is":"declared","declared":"m.R"}}}},"then":{},"departures":[{{"clause":null,"body":{}}}]"#,
                read(0, INT),
                truth(true),
                truth(false)
            ),
            BOOL,
        ),
    );
    is_the_halves_disagreeing(
        &with_clauses(&field("count", 0, "INT"), &attempts, &[]),
        "where a clause builds no value",
    );
}

/// What a declaration's clauses are answered under crosses apart from them exactly where another
/// build runs them: a declaration of this build's carries the clauses and not that, and one on the
/// path carries that and not the clauses.
#[test]
fn what_clauses_are_answered_under_crosses_where_another_build_runs_them() {
    let declared = |by: &str, clauses: &str| {
        format!(
            concat!(
                r#"{{"transport":26,"declarations":["#,
                r#"{{"module":"m","name":"R","by":"{}","is":"product","#,
                r#""fields":[{}]{}}}],"behaviors":[],"#,
                r#""modules":[{{"name":"m","publishes":[],"helpers":[],"values":[],"#,
                r#""entries":[],"definitions":[],"examples":[]}}]}}"#
            ),
            by,
            field("count", 0, "INT"),
            clauses
        )
    };
    let headers = r#","headers":[{"name":"counted"}]"#;
    let invariants = r#","invariants":[]"#;

    reads_whole(&declared("onthepath", headers));
    reads_whole(&declared("amodule", invariants));
    is_the_halves_disagreeing(&declared("onthepath", ""), "are not carried apart");
    is_the_halves_disagreeing(
        &declared("amodule", &format!("{invariants}{headers}")),
        "are carried apart",
    );
}

/// What constructing a behavior requires is behaviors the table of targets names, each once and none
/// of them the behavior itself: a host binding it is handed one implementation of each. And a
/// behavior a host implements requires nothing to construct, since Souther does not construct one.
#[test]
fn a_behavior_requires_behaviors_the_document_names_once_each() {
    let injected = format!(
        r#"{{"module":"m","name":"i","is":"injected","parameters":{{"named":[]}},"output":{ANSWERS_INT},"requirements":[],"ensures":{{"at":"none"}}}}"#
    );
    let definition = format!(
        r#"{{"is":"body","declared":"m.b","parameters":["a"],"publication":"kept","body":{}}}"#,
        read(0, INT)
    );
    let requiring = |requirements: &str| {
        [
            held("body", ANSWERS_INT, r#"{"at":"none"}"#).replace(
                r#""requirements":[]"#,
                &format!(r#""requirements":[{requirements}]"#),
            ),
            injected.clone(),
        ]
    };
    let i = r#"{"module":"m","name":"i"}"#;
    reads_whole(&document(
        &requiring(i),
        &[],
        std::slice::from_ref(&definition),
    ));
    is_the_halves_disagreeing(
        &document(
            &requiring(&format!("{i},{i}")),
            &[],
            std::slice::from_ref(&definition),
        ),
        "m.i twice",
    );
    is_the_halves_disagreeing(
        &document(
            &requiring(r#"{"module":"m","name":"b"}"#),
            &[],
            std::slice::from_ref(&definition),
        ),
        "m.b requires itself",
    );
    is_the_halves_disagreeing(
        &document(
            &requiring(r#"{"module":"m","name":"gone"}"#),
            &[],
            std::slice::from_ref(&definition),
        ),
        "m.gone",
    );
    let constructed_host =
        injected.replace(r#""requirements":[]"#, &format!(r#""requirements":[{i}]"#));
    is_the_halves_disagreeing(
        &document(
            &[
                held("body", ANSWERS_INT, r#"{"at":"none"}"#),
                constructed_host,
            ],
            &[],
            std::slice::from_ref(&definition),
        ),
        "a host's is not constructed",
    );
}

/// A field is read off a sum where every case of it lays one of that name out, which is the only
/// read of one the checker writes: `m.S`'s cases are units, and a field read off it is the two
/// halves disagreeing, not something this backend is behind on.
#[test]
fn a_field_is_read_off_a_sum_only_where_every_case_lays_it_out() {
    let read_off = node(
        "field",
        &format!(r#""target":{},"field":"v""#, read(0, S)),
        INT,
    );
    is_the_halves_disagreeing(&helpers(&[h(&[S], &read_off)]), "declares none");
}

/// A set of alternatives naming no case is refused where it is read, wherever it stands: a sum's
/// cases, a union's members, the atoms an arm tests and the cases a stage accepts. Every reader of
/// one tells a value apart by its token and takes the last case for what is tagged by none of the
/// others, which a set with nothing in it gives no last case for: a field read off a sum with no
/// case would branch to nowhere. So it is not a document the checker writes, and not something
/// this backend is behind on.
#[test]
fn a_set_of_alternatives_naming_no_case_is_refused_where_it_is_read() {
    let both = r#""cases":[{"is":"declared","declared":"m.A"},{"is":"declared","declared":"m.B"}],"form":{"is":"enumeration"}"#;
    let none = r#""cases":[],"form":{"is":"discriminated","tag":"type","contents":"value"}"#;
    let read_off = node(
        "field",
        &format!(r#""target":{},"field":"v""#, read(0, S)),
        INT,
    );
    let sum = helpers(&[h(&[S], &read_off)]);
    assert!(sum.contains(both));
    is_the_halves_disagreeing(&sum.replacen(both, none, 1), "no case in it");

    let empty_union = r#"{"union":[]}"#;
    is_the_halves_disagreeing(&helpers(&[h(&[empty_union], &int(1))]), "Ty");

    let testing_nothing = node(
        "match",
        &format!(
            r#""subject":{},"arms":[{}]"#,
            read(0, S),
            arm(r#"{"tests":"which","atoms":[]}"#, None, &int(1))
        ),
        INT,
    );
    is_the_halves_disagreeing(&helpers(&[h(&[S], &testing_nothing)]), "no case in it");

    let scalar = r#"{"is":"scalar","scalar":"INT"}"#;
    let sum_answer = r#"{"is":"nominal","named":{"is":"declared","declared":"m.S"}}"#;
    is_the_halves_disagreeing(
        &routed(
            sum_answer,
            &widen(&unit("m.A"), S),
            sum_answer,
            r#"{"is":"oncases","accepted":[]}"#,
            scalar,
        ),
        "no case in it",
    );
}
