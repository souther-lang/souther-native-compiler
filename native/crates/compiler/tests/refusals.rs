//! What this driver says no to, and which no it is.
//!
//! Two of them and they are not the same. A program the language admits and this backend does not
//! write yet is the backend's shortcoming; a document this driver cannot read is the two halves
//! disagreeing about what they are saying to each other. The half that started the driver reports
//! them to different people, so what is checked here is that they arrive apart.

use souther_native_driver::{NotLowered, object_for};

/// One behavior over one primitive, joined by one operator — the smallest document that reaches
/// the lowering, with the two things under test as its only variables.
fn document(op: &str, ty: &str) -> String {
    let prim = format!(r#"{{"prim":"{ty}"}}"#);
    over(op, &prim, &prim)
}

/// The same over any pair of types the writer can name, for the ones no Souther source reaches
/// this way.
///
/// A pair and not one type, because an operator here is not given two values of one type: a bare
/// literal takes the newtype of what it is compared with, and a case value is a value of its sum.
/// A builder that wrote one type twice could not say what either of those looks like on the wire.
///
/// Held by a helper and not a behavior. A behavior's parameters are boundary shapes, and a tuple
/// or an optional is not one; a helper takes any type, and is lowered by the same walk.
fn over(op: &str, left: &str, right: &str) -> String {
    let read =
        |at: u32, ty: &str| format!(r#"{{"core":"read","binding":{at},"type":{ty},"aborts":[]}}"#);
    let body = format!(
        r#"{{"core":"binary","op":"{op}","left":{},"right":{},"type":{left},"aborts":[]}}"#,
        read(0, left),
        read(1, right)
    );
    let held = format!(
        r#"{{"declared":"calculation.f","parameters":["a","b"],"takes":[{left},{right}],"answers":{left},"body":{body}}}"#
    );
    format!(
        r#"{{"transport":9,"declarations":[],"behaviors":[],"modules":[{{"name":"calculation","helpers":[{held}],"values":[],"entries":[],"definitions":[],"examples":[]}}]}}"#
    )
}

/// An operator crosses whether or not there is a lowering for it. What it means is the language's
/// and the writer's business; whether this can write it is this driver's, and it is answered here.
#[test]
fn an_operator_with_no_lowering_is_not_lowered_rather_than_unreadable() {
    let refused = object_for(&document("DIV", "INT")).expect_err("no lowering for it");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "read as something other than a lowering this driver has not got: {refused}"
    );
    assert!(refused.to_string().contains('/'), "{refused}");
}

/// The same for a primitive. A type with no machine representation yet is a lowering this driver
/// has not got, not a document it failed to understand.
#[test]
fn a_primitive_with_no_representation_is_not_lowered() {
    let refused = object_for(&document("ADD", "DECIMAL")).expect_err("no representation for it");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "read as something other than a lowering this driver has not got: {refused}"
    );
    assert!(refused.to_string().contains("Decimal"), "{refused}");
}

/// A sum of two values of a declared type, which is the address each of them is held as.
///
/// No Souther source produces this: what crosses for `a + b` over a newtype is a construction of
/// the newtype over the sum of the two wrapped numbers, so the operands arrive as `Int`. Written
/// by hand for that reason — the arm that answers it is what keeps this driver from depending on
/// an arrangement made on the other side of the transport, and an `iadd` over two addresses would
/// have answered an address that points at neither.
#[test]
fn a_sum_of_two_addresses_is_the_halves_disagreeing() {
    let amount = r#"{"declared":"counting.Amount"}"#;
    let document = over("ADD", amount, amount);

    let refused = object_for(&document).expect_err("nothing here adds two addresses");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("counting.Amount"), "{refused}");
}

/// The same with a number on one side, which is the pair a reading of the left operand alone lets
/// through. Both orders, because a reading that answered from either one of them answers one of
/// these and not the other.
#[test]
fn a_sum_of_a_number_and_an_address_is_the_halves_disagreeing_whichever_side_it_is_on() {
    let amount = r#"{"declared":"counting.Amount"}"#;
    let number = r#"{"prim":"INT"}"#;

    for document in [over("ADD", number, amount), over("ADD", amount, number)] {
        let refused = object_for(&document).expect_err("nothing here adds a number to an address");

        assert!(
            refused.downcast_ref::<NotLowered>().is_none(),
            "the halves disagreeing is not the backend being behind: {refused}"
        );
    }
}

/// An ordering over two tuples, and over two optionals.
///
/// What the language orders is a number, text, an amount, a moment, an enumeration and a newtype
/// over one of those. A tuple and an optional have equality and no order, so `<` over either is
/// the halves disagreeing — the same answer a `Bool` gets, and the reason is the same.
///
/// Written by hand because the checker refuses both, which is what makes them the disagreement
/// they are read as here.
#[test]
fn an_ordering_over_what_has_equality_and_no_order_is_the_halves_disagreeing() {
    let pair = r#"{"tuple":[{"prim":"INT"},{"prim":"INT"}]}"#;
    let held = r#"{"option":{"prim":"INT"}}"#;

    for ty in [pair, held] {
        let refused = object_for(&over("LT", ty, ty)).expect_err("nothing orders these");

        assert!(
            refused.downcast_ref::<NotLowered>().is_none(),
            "the halves disagreeing is not the backend being behind: {refused}"
        );
    }
}

/// And equality over the same two, which is a comparison still to be written rather than a
/// disagreement: the language does compare them, by what they hold.
#[test]
fn equality_over_what_has_equality_and_no_order_is_a_lowering_this_has_not_got() {
    let pair = r#"{"tuple":[{"prim":"INT"},{"prim":"INT"}]}"#;
    let held = r#"{"option":{"prim":"INT"}}"#;

    for ty in [pair, held] {
        let refused = object_for(&over("EQ", ty, ty)).expect_err("no comparison for these yet");

        assert!(
            refused.downcast_ref::<NotLowered>().is_some(),
            "the backend being behind is not the halves disagreeing: {refused}"
        );
    }
}

/// An ordering over two truths, which is not a program the language admits: `Bool` is not one of
/// the ordered types. So it is the two halves disagreeing about what they are saying to each other
/// rather than this backend being behind, and the author of the program is not the one who can do
/// anything about it.
///
/// There is no Souther source that produces this, which is why it is written as a document by
/// hand. What it holds is the arm that answers it — a comparison decided by the machine width
/// would have run it as an `icmp` over two bytes and answered something.
#[test]
fn an_ordering_over_two_truths_is_the_halves_disagreeing_and_not_a_lowering_this_has_not_got() {
    let refused = object_for(&document("LT", "BOOL")).expect_err("nothing orders two truths");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("Bool"), "{refused}");
}

/// A field nothing here names is the writer saying something this driver has no idea it was told.
/// Reading past it would be compiling a program that means more than what was understood of it.
#[test]
fn a_field_this_driver_does_not_know_is_refused_rather_than_skipped() {
    let said_more = document("ADD", "INT").replace(
        r#""core":"binary""#,
        r#""core":"binary","somethingNewlySettled":true"#,
    );

    let refused = object_for(&said_more).expect_err("a document saying more than this reads");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
}

/// A transport this driver does not read is refused whole. Reading the parts that happen to parse
/// would be reading a document written to mean something else.
#[test]
fn a_transport_from_another_version_is_refused() {
    let later = document("ADD", "INT").replace(r#""transport":9"#, r#""transport":10"#);

    let refused = object_for(&later).expect_err("a version this does not read");

    assert!(refused.to_string().contains("10"), "{refused}");
}

/// A behavior's parameter is a boundary shape, and a function is not one: the language gives a
/// function no external representation, so nothing the checker settles puts one at a behavior's
/// boundary. A document that does is not a program this backend is behind on; it is one this
/// driver does not read.
#[test]
fn a_function_at_a_behaviors_boundary_is_not_a_document_this_driver_reads() {
    let document = concat!(
        r#"{"transport":9,"declarations":[],"#,
        r#""behaviors":[{"module":"m","name":"choose","is":"injected","inputs":["#,
        r#"{"fn":{"takes":[{"prim":"INT"}],"answers":{"prim":"INT"}}}],"#,
        r#""output":{"is":"scalar","scalar":"INT"}}],"#,
        r#""modules":[{"name":"m","helpers":[],"values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("a function at a behavior's boundary");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "a shape the boundary has no word for is the halves disagreeing: {refused}"
    );
}

/// What a behavior answers crosses whole, and a collection is one of the things it can answer.
/// Reading it is not laying it out: a `List` has no representation here yet, which is this
/// backend being behind and not the document being unreadable.
#[test]
fn an_answer_that_is_a_list_is_read_and_not_lowered() {
    let document = concat!(
        r#"{"transport":9,"declarations":[],"#,
        r#""behaviors":[{"module":"m","name":"many","is":"injected","inputs":[],"#,
        r#""output":{"is":"listof","element":{"is":"scalar","scalar":"INT"}}}],"#,
        r#""modules":[{"name":"m","helpers":[],"values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("no layout for a list");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "a list read whole and not laid out is the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("List"), "{refused}");
}

/// A primitive standing as a member of an answer is a case the transport carries. What holds a
/// union here says which member it is by a token, and an `Int` carries none, so a value of that
/// union has no representation yet.
#[test]
fn an_answer_with_a_primitive_among_its_cases_is_read_and_not_lowered() {
    let document = concat!(
        r#"{"transport":9,"declarations":["#,
        r#"{"module":"m","name":"NotFound","by":"amodule","is":"unit"}],"#,
        r#""behaviors":[{"module":"m","name":"lengthOf","is":"injected","inputs":[],"#,
        r#""output":{"is":"cases","type":{"union":[{"is":"primitive","prim":"INT"},"#,
        r#"{"is":"declared","declared":"m.NotFound"}]},"#,
        r#""cases":[{"is":"primitive","prim":"INT"},{"is":"declared","declared":"m.NotFound"}],"#,
        r#""form":{"is":"discriminated","tag":"type","contents":"value"}}}],"#,
        r#""modules":[{"name":"m","helpers":[],"values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("no token for an Int to say which case it is");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "a union read whole and not laid out is the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("Int"), "{refused}");
}

/// Two calls reaching one published value at two different types is not a document this backend
/// is behind on — a value is one declaration and answers one way, so this is the checker and this
/// reading of its document disagreeing about something more basic than a lowering not written
/// yet, and is refused the way any other such disagreement here is, before either call is ever
/// declared a symbol for.
#[test]
fn a_published_value_reached_at_two_different_types_is_the_halves_disagreeing() {
    let document = concat!(
        r#"{"transport":9,"declarations":[],"#,
        r#""behaviors":[{"module":"m","name":"f","is":"body","inputs":[],"output":{"is":"scalar","scalar":"INT"}},"#,
        r#"{"module":"m","name":"g","is":"body","inputs":[],"output":{"is":"scalar","scalar":"BOOL"}}],"#,
        r#""modules":[{"name":"m","helpers":[],"values":[],"entries":[],"definitions":["#,
        r#"{"is":"body","declared":"m.f","parameters":[],"publication":"kept","#,
        r#""body":{"core":"call","reaches":{"is":"publishedvalue","module":"other","name":"x"},"#,
        r#""arguments":[],"type":{"prim":"INT"},"aborts":[]}},"#,
        r#"{"is":"body","declared":"m.g","parameters":[],"publication":"kept","#,
        r#""body":{"core":"call","reaches":{"is":"publishedvalue","module":"other","name":"x"},"#,
        r#""arguments":[],"type":{"prim":"BOOL"},"aborts":[]}}"#,
        r#"],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("one value does not answer two ways");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("other"), "{refused}");
}

/// The smallest composition this driver can be handed: one behavior with a body, one composed of
/// a single stage reaching it, and every fact the two of them share stated once so each of the
/// tests below has one place to make disagree with the other.
fn composed_document() -> String {
    concat!(
        r#"{"transport":9,"declarations":[],"#,
        r#""behaviors":[{"module":"m","name":"inner","is":"body","inputs":[{"is":"scalar","scalar":"INT"}],"output":{"is":"scalar","scalar":"INT"}},"#,
        r#"{"module":"m","name":"outer","is":"composed","inputs":[{"is":"scalar","scalar":"INT"}],"output":{"is":"scalar","scalar":"INT"}}],"#,
        r#""modules":[{"name":"m","helpers":[],"values":[],"entries":[],"definitions":["#,
        r#"{"is":"body","declared":"m.inner","parameters":["a"],"publication":"kept","body":{"core":"read","binding":0,"type":{"prim":"INT"},"aborts":[]}},"#,
        r#"{"is":"composed","declared":"m.outer","publication":"published","stages":["#,
        r#"{"behavior":"m.inner","answers":{"prim":"INT"},"routing":{"is":"always"}}],"answers":{"prim":"INT"}}"#,
        r#"],"examples":[]}]}"#,
    )
    .to_string()
}

/// A composition's own document validates: this is the control every test below makes one fact
/// of disagree with another, so a failure there is a failure of the setup and not of what is
/// under test.
#[test]
fn the_composed_document_the_other_tests_perturb_is_itself_well_formed() {
    object_for(&composed_document()).expect("a document where nothing has been made to disagree");
}

/// What a target says a name answers with — `body` or `composed` — and what its local definition
/// actually is are two readings of one fact once a composition is a local definition beside a
/// body. A document where they disagree is the two halves disagreeing, the same as any other
/// closed set spelt one way in one place and another elsewhere.
#[test]
fn a_target_and_its_local_definition_disagreeing_about_which_it_is_is_the_halves_disagreeing() {
    let document = composed_document().replace(
        r#""name":"outer","is":"composed""#,
        r#""name":"outer","is":"body""#,
    );

    let refused = object_for(&document)
        .expect_err("a target saying `body` where its local definition is a composition");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("m.outer"), "{refused}");
}

/// The same disagreement, the other way round: a target saying a name is `injected`, `elsewhere`
/// or (as here) `unwritten` has no local definition to speak of — and one sitting under its name
/// regardless is not this backend being behind on a program the language admits. `Unwritten` is
/// Souther's own answer for a behavior nobody has written, and a composition sitting under that
/// name says the opposite: the two halves disagree about whether this object defines the name at
/// all, which is checked from the local definition's side and not left for whichever branch of
/// the declaration loop the target's own `is` happens to route through.
#[test]
fn a_local_definition_whose_target_says_unwritten_is_the_halves_disagreeing() {
    let document = composed_document().replace(
        r#""name":"outer","is":"composed""#,
        r#""name":"outer","is":"unwritten""#,
    );

    let refused = object_for(&document)
        .expect_err("a composition sitting under a name its target says is unwritten");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend admitting a program it has not gotten round \
         to: {refused}"
    );
    assert!(refused.to_string().contains("m.outer"), "{refused}");
}

/// A composition's own `answers` and its target's `answers` are the same fact, crossed twice —
/// once as what the composition itself carries, once as what every caller reaching it by name is
/// told. A document where they disagree is refused rather than read as whichever one happened to
/// be asked.
#[test]
fn a_compositions_own_answer_disagreeing_with_its_target_is_the_halves_disagreeing() {
    let document = composed_document().replace(
        r#"],"answers":{"prim":"INT"}}],"examples":[]}]}"#,
        r#"],"answers":{"prim":"BOOL"}}],"examples":[]}]}"#,
    );

    let refused = object_for(&document)
        .expect_err("a composition answering BOOL where its target answers INT");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("m.outer"), "{refused}");
}

/// A stage's own `answers` and the `answers` of the target it names are likewise one fact crossed
/// twice. Nothing here recomputes a stage's answer from its behavior — the checker already
/// settled it — but a stage that spelt it differently from the target it reaches is not read as
/// either spelling; it is refused.
#[test]
fn a_stages_own_answer_disagreeing_with_the_target_it_reaches_is_the_halves_disagreeing() {
    let document = composed_document().replace(
        r#""behavior":"m.inner","answers":{"prim":"INT"}"#,
        r#""behavior":"m.inner","answers":{"prim":"BOOL"}"#,
    );

    let refused = object_for(&document)
        .expect_err("a stage answering BOOL where the target it reaches answers INT");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("m.inner"), "{refused}");
}

/// The first stage of a composition takes the composition's own arguments, so nothing is routed
/// into it (spec §sequential-composition) — every composition a real checker settles carries
/// `Routing::Always` there. A document that carries something else for it is not a program this
/// backend has not gotten round to; it is a document this driver does not read as a composition at
/// all, because reading past it would mean working the first stage's routing out again rather
/// than reading what the checker wrote.
#[test]
fn a_compositions_first_stage_routed_rather_than_always_applied_is_the_halves_disagreeing() {
    let document = composed_document().replace(
        r#""routing":{"is":"always"}"#,
        r#""routing":{"is":"oncases","accepted":[{"is":"declared","declared":"m.Nothing"}]}"#,
    );

    let refused =
        object_for(&document).expect_err("a first stage that is routed rather than always applied");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("m.outer"), "{refused}");
}

/// A composition takes whatever its first stage takes (spec §sequential-composition) — that is
/// where a composition's own parameters are read off in the first place — so a composition's own
/// `takes` and its first stage's target's `takes` are one fact as well. A document where they
/// disagree names two different arities for what is, upstream, a single signature.
#[test]
fn a_compositions_own_takes_disagreeing_with_its_first_stages_target_is_the_halves_disagreeing() {
    let document = composed_document().replace(
        r#""name":"outer","is":"composed","inputs":[{"is":"scalar","scalar":"INT"}]"#,
        r#""name":"outer","is":"composed","inputs":[{"is":"scalar","scalar":"INT"},{"is":"scalar","scalar":"INT"}]"#,
    );

    let refused = object_for(&document)
        .expect_err("a composition taking two `Int`s where its first stage's target takes one");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("m.outer"), "{refused}");
}

/// A hand-written document where an `Apply`'s own applied function answers one declared type and
/// the `Apply` node itself is typed as a different one — both `Ty::Declared`, so both share one
/// machine representation (`POINTER`). A check at machine representation alone, the same as
/// `Apply`'s own arguments are held to, would pass this silently: the closure would store one
/// type's address into `out` and the caller would read the same address back as the other type,
/// which is not a crash — Cranelift has nothing to object to — just the wrong type read from a
/// real address from then on. Souther's checker never writes this (`Core.Apply`'s own `type` is
/// built straight from the applied value's `Type.FnOf` result, with no assignability in between),
/// so this is exactly the kind of malformed or version-skewed document this strict reader exists
/// to refuse rather than execute. The function arrives as a helper's parameter, the one place a
/// function can be handed over: a behavior's parameters are boundary shapes and a function is not
/// one.
#[test]
fn an_applys_answer_disagreeing_with_its_functions_own_type_is_the_halves_disagreeing_even_though_both_are_pointers()
 {
    let document = concat!(
        r#"{"transport":9,"declarations":["#,
        r#"{"module":"m","name":"A","by":"amodule","is":"unit"},"#,
        r#"{"module":"m","name":"B","by":"amodule","is":"unit"}],"#,
        r#""behaviors":[],"#,
        r#""modules":[{"name":"m","#,
        r#""helpers":[{"declared":"m.f","parameters":["f"],"#,
        r#""takes":[{"fn":{"takes":[],"answers":{"declared":"m.A"}}}],"answers":{"declared":"m.B"},"#,
        r#""body":{"core":"apply","function":{"core":"read","binding":0,"#,
        r#""type":{"fn":{"takes":[],"answers":{"declared":"m.A"}}},"aborts":[]},"#,
        r#""arguments":[],"type":{"declared":"m.B"},"aborts":[]}}],"#,
        r#""values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );

    let refused = object_for(document)
        .expect_err("an Apply answering a different declared type than its own function's type");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(
        refused.to_string().contains("m.B") || refused.to_string().contains("m.A"),
        "{refused}"
    );
}

/// A field whose scalar has no representation here refuses the boundary that would write it, and
/// not the behavior: the value itself only passes through, and nothing lowers a `Decimal` until the
/// entry that has to write one out.
#[test]
fn a_published_answer_with_a_decimal_field_is_not_lowered_where_it_is_written() {
    let document = concat!(
        r#"{"transport":9,"declarations":["#,
        r#"{"module":"m","name":"Priced","by":"amodule","is":"product","#,
        r#""fields":[{"name":"amount","codec":{"is":"scalar","scalar":"DECIMAL"}}],"invariants":0}],"#,
        r#""behaviors":[{"module":"m","name":"same","is":"body","#,
        r#""inputs":[{"is":"nominal","declared":"m.Priced"}],"#,
        r#""output":{"is":"nominal","declared":"m.Priced"}}],"#,
        r#""modules":[{"name":"m","helpers":[],"values":[],"entries":[],"definitions":["#,
        r#"{"is":"body","declared":"m.same","parameters":["p"],"publication":"published","#,
        r#""body":{"core":"read","binding":0,"type":{"declared":"m.Priced"},"aborts":[]}}"#,
        r#"],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("no way yet to write a Decimal out");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "a scalar this backend cannot write yet is the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("Decimal"), "{refused}");

    let kept = document.replace(r#""publication":"published""#, r#""publication":"kept""#);
    object_for(&kept).expect("a kept behavior has no boundary, so nothing is written out");
}

/// A published behavior answering `m.S`, whose one case `m.C` is declared with the fields and
/// alternatives form given. The smallest document that makes a boundary write a sum.
fn answering_a_sum(case_fields: &str, form: &str) -> String {
    format!(
        concat!(
            r#"{{"transport":9,"declarations":["#,
            r#"{{"module":"m","name":"C","by":"amodule","is":"product","fields":{},"invariants":0}},"#,
            r#"{{"module":"m","name":"S","by":"amodule","is":"sum","#,
            r#""cases":[{{"is":"declared","declared":"m.C"}}],"form":{}}}],"#,
            r#""behaviors":[{{"module":"m","name":"same","is":"body","#,
            r#""inputs":[{{"is":"nominal","declared":"m.S"}}],"#,
            r#""output":{{"is":"nominal","declared":"m.S"}}}}],"#,
            r#""modules":[{{"name":"m","helpers":[],"values":[],"entries":[],"definitions":["#,
            r#"{{"is":"body","declared":"m.same","parameters":["s"],"publication":"published","#,
            r#""body":{{"core":"read","binding":0,"type":{{"declared":"m.S"}},"aborts":[]}}}}"#,
            r#"],"examples":[]}}]}}"#
        ),
        case_fields, form
    )
}

/// An enumeration is a set of alternatives that carry nothing but which one they are, so a case
/// carrying fields under one is the two halves disagreeing. Written as a bare name, its fields
/// would be dropped without a word.
#[test]
fn an_enumeration_over_a_case_with_fields_is_the_halves_disagreeing() {
    let document = answering_a_sum(
        r#"[{"name":"n","codec":{"is":"scalar","scalar":"INT"}}]"#,
        r#"{"is":"enumeration"}"#,
    );

    let refused = object_for(&document).expect_err("a case with fields in an enumeration");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("m.C"), "{refused}");
}

/// The tag and a wrapped case's contents stand side by side in one object, so one key for both
/// would leave whichever was written second.
#[test]
fn a_discriminated_form_with_one_key_for_tag_and_contents_is_the_halves_disagreeing() {
    let document = answering_a_sum(
        "[]",
        r#"{"is":"discriminated","tag":"type","contents":"type"}"#,
    );

    let refused = object_for(&document).expect_err("one key for the tag and the contents");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("type"), "{refused}");
}

/// What a field carries and what a construction puts in it are one fact crossed twice: the
/// encoder reads the slot as the codec says, and the construction wrote it as its value is. An
/// `Int` and a `String` are both one slot wide, so nothing about the machine would notice the two
/// disagreeing; the value would be written out as whatever the codec took it for.
#[test]
fn a_construction_disagreeing_with_what_its_field_carries_is_the_halves_disagreeing() {
    let document = concat!(
        r#"{"transport":9,"declarations":["#,
        r#"{"module":"m","name":"P","by":"amodule","is":"product","#,
        r#""fields":[{"name":"n","codec":{"is":"scalar","scalar":"STRING"}}],"invariants":0}],"#,
        r#""behaviors":[{"module":"m","name":"make","is":"body","inputs":[],"#,
        r#""output":{"is":"nominal","declared":"m.P"}}],"#,
        r#""modules":[{"name":"m","helpers":[],"values":[],"entries":[],"definitions":["#,
        r#"{"is":"body","declared":"m.make","parameters":[],"publication":"kept","#,
        r#""body":{"core":"construct","declared":"m.P","#,
        r#""values":[{"core":"int","value":42,"type":{"prim":"INT"},"aborts":[]}],"#,
        r#""type":{"declared":"m.P"},"aborts":[]}}"#,
        r#"],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("an Int put where the field carries a String");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("m.P"), "{refused}");
}

/// A product `m.P` with one field carrying `codec`, built in a kept behavior's body from `value`,
/// beside the sum `m.S = m.A | m.B` and the unrelated unit `m.U`.
fn building(codec: &str, value: &str) -> String {
    format!(
        concat!(
            r#"{{"transport":9,"declarations":["#,
            r#"{{"module":"m","name":"A","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"B","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"U","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"S","by":"amodule","is":"sum","#,
            r#""cases":[{{"is":"declared","declared":"m.A"}},{{"is":"declared","declared":"m.B"}}],"#,
            r#""form":{{"is":"enumeration"}}}},"#,
            r#"{{"module":"m","name":"P","by":"amodule","is":"product","#,
            r#""fields":[{{"name":"f","codec":{}}}],"invariants":0}}],"#,
            r#""behaviors":[{{"module":"m","name":"make","is":"body","inputs":[],"#,
            r#""output":{{"is":"nominal","declared":"m.P"}}}}],"#,
            r#""modules":[{{"name":"m","helpers":[],"values":[],"entries":[],"definitions":["#,
            r#"{{"is":"body","declared":"m.make","parameters":[],"publication":"kept","#,
            r#""body":{{"core":"construct","declared":"m.P","values":[{}],"#,
            r#""type":{{"declared":"m.P"}},"aborts":[]}}}}"#,
            r#"],"examples":[]}}]}}"#
        ),
        codec, value
    )
}

fn unit(declared: &str) -> String {
    format!(
        r#"{{"core":"unit","declared":"{declared}","type":{{"declared":"{declared}"}},"aborts":[]}}"#
    )
}

/// A field naming one declaration given a value of an unrelated one. Both are pointers, so the
/// machine would not tell them apart; the encoder would read the value's slots as the other
/// declaration lays its fields out.
#[test]
fn a_field_of_one_declaration_given_a_value_of_another_is_the_halves_disagreeing() {
    let document = building(r#"{"is":"named","declared":"m.A"}"#, &unit("m.U"));

    let refused = object_for(&document).expect_err("m.U put where the field carries m.A");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "the halves disagreeing is not the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("m.P"), "{refused}");
}

/// A field of a sum is given a value of one of its cases, which is the same field and the same
/// value as far as the language is concerned. What is refused is a value no case of the sum is.
#[test]
fn a_field_of_a_sum_given_a_value_of_one_of_its_cases_is_built() {
    let named_sum = r#"{"is":"named","declared":"m.S"}"#;

    object_for(&building(named_sum, &unit("m.B"))).expect("m.B is a case of m.S");
    let refused =
        object_for(&building(named_sum, &unit("m.U"))).expect_err("m.U is no case of m.S");
    assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
}

/// Shapes the checker has no way to build are not values on this side either: an optional of an
/// optional, a newtype of two fields, a unit with a field. Each is refused as a document this
/// driver does not read, before anything is asked of it.
#[test]
fn a_shape_the_checker_cannot_build_is_not_a_document_this_driver_reads() {
    let option_of_option = building(
        r#"{"is":"optionof","present":{"is":"optionof","present":{"is":"scalar","scalar":"INT"}}}"#,
        r#"{"core":"none","type":{"option":{"option":{"prim":"INT"}}},"aborts":[]}"#,
    );
    let newtype_of_two = building(r#"{"is":"scalar","scalar":"INT"}"#, &unit("m.U")).replace(
        r#"{"module":"m","name":"U","by":"amodule","is":"unit"}"#,
        r#"{"module":"m","name":"U","by":"amodule","is":"newtype","fields":[],"invariants":0}"#,
    );
    let unit_with_a_field = building(r#"{"is":"scalar","scalar":"INT"}"#, &unit("m.U")).replace(
        r#"{"module":"m","name":"U","by":"amodule","is":"unit"}"#,
        r#"{"module":"m","name":"U","by":"amodule","is":"unit","fields":[],"invariants":0}"#,
    );

    for document in [option_of_option, newtype_of_two, unit_with_a_field] {
        let refused = object_for(&document).expect_err("a shape nothing settles");
        assert!(
            refused.downcast_ref::<NotLowered>().is_none(),
            "a shape the checker cannot build is the halves disagreeing: {refused}"
        );
        assert!(
            refused.to_string().contains("line") || refused.to_string().contains("optional"),
            "refused where it was read: {refused}"
        );
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

/// Enumeration exactly when every case is a unit, in both directions: a set of units written
/// discriminated would put `{"type":"A"}` where the checker settled `"A"`.
#[test]
fn a_discriminated_form_over_nothing_but_units_is_the_halves_disagreeing() {
    let document = building(r#"{"is":"scalar","scalar":"INT"}"#, &unit("m.U")).replace(
        r#""form":{"is":"enumeration"}"#,
        r#""form":{"is":"discriminated","tag":"type","contents":"value"}"#,
    );
    is_the_halves_disagreeing(&document, "m.S");
}

/// A product case's fields stand in the object that carries the tag, so a field under the tag's
/// key would leave two members of one name — the checker refuses the field, and so is a document
/// that has one.
#[test]
fn a_case_with_a_field_under_the_tags_key_is_the_halves_disagreeing() {
    let document = answering_a_sum(
        r#"[{"name":"type","codec":{"is":"scalar","scalar":"INT"}}]"#,
        r#"{"is":"discriminated","tag":"type","contents":"value"}"#,
    );
    is_the_halves_disagreeing(&document, "m.C");
}

/// What an answer union's type is and which cases it is written by are two crossings of one
/// answer. A case the type has and the cases leave out would be a value the boundary has no arm
/// for.
#[test]
fn an_answer_whose_cases_are_not_what_its_type_descends_to_is_the_halves_disagreeing() {
    let document = concat!(
        r#"{"transport":9,"declarations":["#,
        r#"{"module":"m","name":"A","by":"amodule","is":"unit"},"#,
        r#"{"module":"m","name":"B","by":"amodule","is":"unit"}],"#,
        r#""behaviors":[{"module":"m","name":"either","is":"injected","inputs":[],"#,
        r#""output":{"is":"cases","type":{"union":[{"is":"declared","declared":"m.A"},"#,
        r#"{"is":"declared","declared":"m.B"}]},"#,
        r#""cases":[{"is":"declared","declared":"m.A"}],"form":{"is":"enumeration"}}}],"#,
        r#""modules":[{"name":"m","helpers":[],"values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );
    is_the_halves_disagreeing(document, "m.either");
}

/// A body's parameters and its target's inputs are one list crossed twice.
#[test]
fn a_body_naming_more_parameters_than_its_target_takes_is_the_halves_disagreeing() {
    let document = composed_document().replace(
        r#""declared":"m.inner","parameters":["a"]"#,
        r#""declared":"m.inner","parameters":["a","b"]"#,
    );
    is_the_halves_disagreeing(&document, "m.inner");
}

/// What a body answers is a value of what its target answers, which the boundary writes it as.
#[test]
fn a_body_answering_other_than_its_target_is_the_halves_disagreeing() {
    let document = building(r#"{"is":"named","declared":"m.A"}"#, &unit("m.A")).replace(
        r#""output":{"is":"nominal","declared":"m.P"}"#,
        r#""output":{"is":"nominal","declared":"m.U"}"#,
    );
    is_the_halves_disagreeing(&document, "m.make");
}

/// What a helper answers is on its signature for every caller and on its body's root for the
/// lowering, and the writer takes the first from the second. A `Bool` signature over an `Int` body
/// would have the helper write eight bytes where each caller reads one.
#[test]
fn a_helper_answering_other_than_its_body_is_the_halves_disagreeing() {
    let int = r#"{"prim":"INT"}"#;
    let document = over("ADD", int, int).replace(
        r#""answers":{"prim":"INT"}"#,
        r#""answers":{"prim":"BOOL"}"#,
    );
    is_the_halves_disagreeing(&document, "calculation.f");
}

/// The same with one address and one number, which are one machine word each. Held to the types
/// themselves and not to their widths, or a caller would read a number as where a value is kept.
#[test]
fn a_helper_answering_a_type_of_the_same_width_as_its_body_is_still_the_halves_disagreeing() {
    let int = r#"{"prim":"INT"}"#;
    let document = over("ADD", int, int).replace(
        r#""answers":{"prim":"INT"}"#,
        r#""answers":{"prim":"STRING"}"#,
    );
    is_the_halves_disagreeing(&document, "calculation.f");
}

/// A signature naming a type this backend has no layout for, over a body it does, is still two
/// statements of one answer that disagree, and is refused as that: that the backend is behind on a
/// `Decimal` is not what is wrong with this document.
#[test]
fn a_helper_answering_what_has_no_layout_over_a_body_that_has_one_is_the_halves_disagreeing() {
    let int = r#"{"prim":"INT"}"#;
    let document = over("ADD", int, int).replace(
        r#""answers":{"prim":"INT"}"#,
        r#""answers":{"prim":"DECIMAL"}"#,
    );
    is_the_halves_disagreeing(&document, "calculation.f");
}

/// A helper's parameter names and the types it takes are one list, written in one loop.
#[test]
fn a_helper_naming_more_parameters_than_it_takes_is_the_halves_disagreeing() {
    let int = r#"{"prim":"INT"}"#;
    let document =
        over("ADD", int, int).replace(r#""parameters":["a","b"]"#, r#""parameters":["a","b","c"]"#);
    is_the_halves_disagreeing(&document, "calculation.f");
}

/// What a value answers is what its body was checked to answer, crossed twice. A declared value
/// and a number are both one word, so nothing on the machine would notice.
#[test]
fn a_value_answering_other_than_its_body_is_the_halves_disagreeing() {
    let document = include_str!("values.transport.json").replace(
        r#""name":"ks","handovers":[],"answers":{"declared":"m.P"}"#,
        r#""name":"ks","handovers":[],"answers":{"prim":"INT"}"#,
    );
    is_the_halves_disagreeing(&document, "m.ks");
}

/// Two targets under one name would leave whichever was read last answering for both.
#[test]
fn two_targets_written_the_same_are_the_halves_disagreeing() {
    let inner = r#"{"module":"m","name":"inner","is":"body","inputs":[{"is":"scalar","scalar":"INT"}],"output":{"is":"scalar","scalar":"INT"}}"#;
    let document = composed_document().replace(
        inner,
        &format!("{inner},{}", inner.replace("INT}", "BOOL}")),
    );
    is_the_halves_disagreeing(&document, "m.inner");
}

/// Two local definitions under one name would have one checked against its target and the other
/// defined.
#[test]
fn two_local_definitions_written_the_same_are_the_halves_disagreeing() {
    let inner = r#"{"is":"body","declared":"m.inner","parameters":["a"],"publication":"kept","body":{"core":"read","binding":0,"type":{"prim":"INT"},"aborts":[]}}"#;
    let document = composed_document().replace(inner, &format!("{inner},{inner}"));
    is_the_halves_disagreeing(&document, "m.inner");
}
