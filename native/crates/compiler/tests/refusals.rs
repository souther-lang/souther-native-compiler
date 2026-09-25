//! What this driver says no to, and which no it is.
//!
//! Two of them and they are not the same. A program the language admits and this backend does not
//! write yet is the backend's shortcoming; a document this driver cannot read is the two halves
//! disagreeing about what they are saying to each other. The half that started the driver reports
//! them to different people, so what is checked here is that they arrive apart.

use souther_native_driver::transport::Program;
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
    // What the operator answers: a truth for a comparison, and otherwise what it is written over.
    let answers = match op {
        "EQ" | "NE" | "LT" | "LE" | "GT" | "GE" | "AND" | "OR" => r#"{"prim":"BOOL"}"#,
        "DIV" => r#"{"prim":"RATIONAL"}"#,
        _ => left,
    };
    over_answering(op, left, right, answers)
}

/// The same, answering what is given. `counting.Amount` is declared, a newtype over an `Int`.
fn over_answering(op: &str, left: &str, right: &str, answers: &str) -> String {
    let read =
        |at: u32, ty: &str| format!(r#"{{"core":"read","binding":{at},"type":{ty},"aborts":[]}}"#);
    // What the operator owes, as the checker names it: arithmetic ends a run that leaves its range,
    // a quotient one with a zero divisor, and nothing else ends one.
    let owes = match op {
        "ADD" | "SUB" | "MUL" => r#""REQUIRED_FORM_HAS_NO_PLACE""#,
        "DIV" => r#""DIVISION_BY_ZERO""#,
        _ => "",
    };
    let body = format!(
        r#"{{"core":"binary","op":"{op}","reading":{{"is":"astheystand"}},"left":{},"right":{},"type":{answers},"aborts":[{owes}]}}"#,
        read(0, left),
        read(1, right)
    );
    let held = format!(
        r#"{{"reached":{{"is":"own","module":"calculation","name":"f"}},"parameters":[{{"name":"a","type":{left}}},{{"name":"b","type":{right}}}],"body":{body}}}"#
    );
    format!(
        r#"{{"transport":18,"declarations":[{{"module":"counting","name":"Amount","by":"amodule","is":"newtype","field":{{"name":"value","binding":0,"codec":{{"is":"scalar","scalar":"INT"}}}},"invariants":[]}}],"behaviors":[],"modules":[{{"name":"calculation","publishes":[],"helpers":[{held}],"values":[],"entries":[],"definitions":[],"examples":[]}}]}}"#
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
    assert!(refused.to_string().contains("Rational"), "{refused}");
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

/// Refused, and refused as not lowered: what is under test is that nothing is emitted for it.
///
/// Every pair below is read as it stands and is one the checker never writes the operator over:
/// which types an operator orders is the checker's rule, and a reading says what the operands were
/// taken as, not whether the operator admits them. So these are refused the way any pair this
/// backend has no lowering for is.
fn is_refused_and_not_lowered(document: &str, naming: &str) {
    let refused = object_for(document).expect_err("nothing is emitted for this pair");
    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
    assert!(refused.to_string().contains(naming), "{refused}");
}

/// A sum of two values of a declared type, answering one.
///
/// No Souther source produces this: what crosses for `a + b` over a newtype is a construction of
/// the newtype over the sum of the two wrapped numbers, and a sum answers a number wherever the
/// checker builds one. An `iadd` over two addresses would have answered an address that points at
/// neither.
#[test]
fn a_sum_answering_other_than_a_number_is_the_halves_disagreeing() {
    let amount = r#"{"declared":"counting.Amount"}"#;
    let refused = object_for(&over("ADD", amount, amount)).expect_err("a sum answers a number");
    assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
    assert!(refused.to_string().contains("calculation.f"), "{refused}");
}

/// A number and an address added as they stand, in both orders. Operands read as they stand are
/// one type, so this is a pair the checker never writes, and an `iadd` over them would have
/// answered an address that points at neither: refused as the two halves disagreeing.
#[test]
fn a_sum_of_a_number_and_an_address_is_the_halves_disagreeing_whichever_side_it_is_on() {
    let amount = r#"{"declared":"counting.Amount"}"#;
    let number = r#"{"prim":"INT"}"#;
    for (left, right) in [(number, amount), (amount, number)] {
        let refused = object_for(&over_answering("ADD", left, right, number))
            .expect_err("operands read as they stand are one type");
        assert!(refused.downcast_ref::<NotLowered>().is_none(), "{refused}");
        assert!(
            refused.to_string().contains("read as it stands"),
            "{refused}"
        );
    }
}

/// An ordering over two tuples, and over two optionals: a tuple and an optional have equality and
/// no order.
#[test]
fn an_ordering_over_what_has_equality_and_no_order_is_refused() {
    let pair = r#"{"tuple":[{"prim":"INT"},{"prim":"INT"}]}"#;
    let held = r#"{"option":{"prim":"INT"}}"#;
    for ty in [pair, held] {
        is_refused_and_not_lowered(&over("LT", ty, ty), "<");
    }
}

/// And equality over the same two, which the language does have: they are compared by what they
/// hold.
#[test]
fn equality_over_what_has_equality_and_no_order_is_lowered() {
    let pair = r#"{"tuple":[{"prim":"INT"},{"prim":"INT"}]}"#;
    let held = r#"{"option":{"prim":"INT"}}"#;

    for ty in [pair, held] {
        for op in ["EQ", "NE"] {
            if let Err(refused) = object_for(&over(op, ty, ty)) {
                panic!("{op} over {ty} is refused: {refused}");
            }
        }
    }
}

/// An ordering over two truths: `Bool` is not one of the ordered types. A comparison decided by
/// the machine width would have run it as an `icmp` over two bytes and answered something.
#[test]
fn an_ordering_over_two_truths_is_refused() {
    is_refused_and_not_lowered(&document("LT", "BOOL"), "Bool");
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
    let later = document("ADD", "INT").replace(r#""transport":18"#, r#""transport":19"#);

    let refused = object_for(&later).expect_err("a version this does not read");

    assert!(refused.to_string().contains("18"), "{refused}");
}

/// A behavior's parameter is a boundary shape, and a function is not one: the language gives a
/// function no external representation, so nothing the checker settles puts one at a behavior's
/// boundary. A document that does is not a program this backend is behind on; it is one this
/// driver does not read.
#[test]
fn a_function_at_a_behaviors_boundary_is_not_a_document_this_driver_reads() {
    let document = concat!(
        r#"{"transport":18,"declarations":[],"#,
        r#""behaviors":[{"module":"m","name":"choose","is":"injected","parameters":{"named":[{"name":"f","input":"#,
        r#"{"fn":{"takes":[{"prim":"INT"}],"answers":{"prim":"INT"}}}}]},"#,
        r#""output":{"is":"scalar","scalar":"INT"},"ensures":{"at":"none"}}],"#,
        r#""modules":[{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("a function at a behavior's boundary");

    assert!(
        refused.downcast_ref::<NotLowered>().is_none(),
        "a shape the boundary has no word for is the halves disagreeing: {refused}"
    );
}

/// What a behavior takes is named where it is declared and unnamed where it is a composition, and
/// each kind of answer is one or the other: a body, a behavior a host implements and one not
/// written are declared, a composition is not, and one another build implements may be either.
/// Every pair is asked, read as a document and nothing further, so the answer is the reader's.
#[test]
fn what_a_behavior_takes_is_named_as_its_kind_of_answer_declares() {
    let named = r#"{"named":[{"name":"n","input":{"is":"scalar","scalar":"INT"}}]}"#;
    let positional = r#"{"positional":[{"is":"scalar","scalar":"INT"}]}"#;
    for (is, reads_named, reads_positional) in [
        ("body", true, false),
        ("injected", true, false),
        ("unwritten", true, false),
        ("composed", false, true),
        ("elsewhere", true, true),
    ] {
        for (parameters, reads) in [(named, reads_named), (positional, reads_positional)] {
            let document = format!(
                concat!(
                    r#"{{"transport":18,"declarations":[],"#,
                    r#""behaviors":[{{"module":"m","name":"f","is":"{}","parameters":{},"#,
                    r#""output":{{"is":"scalar","scalar":"INT"}},"ensures":{{"at":"none"}}}}],"#,
                    r#""modules":[]}}"#,
                ),
                is, parameters
            );
            let read = Program::read(&document);
            assert_eq!(
                read.is_ok(),
                reads,
                "{is} with {parameters}: {:?}",
                read.err()
            );
            if let Err(refused) = read {
                assert!(
                    refused.to_string().contains("the two halves disagree"),
                    "{refused}"
                );
            }
        }
    }
}

/// The names an `ensures` relates are the parameters the behavior declares, crossed twice: a
/// clause relating others is refused, and so is one on a behavior that declares none.
#[test]
fn an_ensures_relates_the_parameters_the_behavior_declares() {
    let contract = |parameters: &str| {
        format!(r#"{{"at":"crossing","contract":{{"parameters":{parameters},"rules":[]}}}}"#)
    };
    let document = |parameters: &str, ensures: &str| {
        format!(
            concat!(
                r#"{{"transport":18,"declarations":[],"#,
                r#""behaviors":[{{"module":"m","name":"f","is":"injected","parameters":{},"#,
                r#""output":{{"is":"scalar","scalar":"INT"}},"ensures":{}}}],"#,
                r#""modules":[]}}"#,
            ),
            parameters, ensures
        )
    };
    let named = r#"{"named":[{"name":"n","input":{"is":"scalar","scalar":"INT"}}]}"#;

    assert!(Program::read(&document(named, &contract(r#"["n"]"#))).is_ok());
    let other = Program::read(&document(named, &contract(r#"["m"]"#)))
        .expect_err("a clause relating a parameter the behavior does not take");
    assert!(
        other.to_string().contains(r#"its ensures relates ["m"]"#),
        "{other}"
    );
    let composed = document(
        r#"{"positional":[{"is":"scalar","scalar":"INT"}]}"#,
        &contract(r#"["n"]"#),
    )
    .replace(r#""is":"injected""#, r#""is":"elsewhere""#);
    let none =
        Program::read(&composed).expect_err("a clause on a behavior that declares no parameters");
    assert!(
        none.to_string().contains("declares no parameters"),
        "{none}"
    );
}

/// A document of an earlier transport is refused as that, and not as whichever member moved
/// since: 17 wrote a helper under where it was declared, which 18 does not read.
#[test]
fn a_transport_of_an_earlier_shape_is_refused_by_its_version() {
    let earlier = concat!(
        r#"{"transport":17,"declarations":[],"behaviors":[],"#,
        r#""modules":[{"name":"m","publishes":[],"#,
        r#""helpers":[{"declared":"m.f","parameters":[],"#,
        r#""body":{"core":"int","value":1,"type":{"prim":"INT"},"aborts":[]}}],"#,
        r#""values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );

    let refused = object_for(earlier).expect_err("a transport of another version");

    assert!(
        refused
            .to_string()
            .contains("this driver reads transport 18 and was handed 17"),
        "{refused}"
    );
}

/// What a behavior answers crosses whole, and a collection is one of the things it can answer.
/// Reading it is not laying it out: a `Set` has no representation here yet, which is this backend
/// being behind and not the document being unreadable.
#[test]
fn an_answer_that_is_a_set_is_read_and_not_lowered() {
    let document = concat!(
        r#"{"transport":18,"declarations":[],"#,
        r#""behaviors":[{"module":"m","name":"many","is":"injected","parameters":{"named":[]},"#,
        r#""output":{"is":"setof","element":{"is":"scalar","scalar":"INT"}},"ensures":{"at":"none"}}],"#,
        r#""modules":[{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("no layout for a set");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "a set read whole and not laid out is the backend being behind: {refused}"
    );
    assert!(refused.to_string().contains("Set"), "{refused}");
}

/// A primitive standing as a member of an answer is a case the transport carries, and a value of
/// the union has a representation here: the `Int` is carried with the runtime's token for it. What
/// answers the behavior is the object of the build that declares it, reached from every other
/// object, and a union with a case no declaration names is not yet one an object is run reading
/// from another; nor is a host handed a way to make one. So the behavior is not lowered.
#[test]
fn an_answer_with_a_primitive_among_its_cases_is_read_and_not_lowered() {
    let document = concat!(
        r#"{"transport":18,"declarations":["#,
        r#"{"module":"m","name":"NotFound","by":"amodule","is":"unit"}],"#,
        r#""behaviors":[{"module":"m","name":"lengthOf","is":"injected","parameters":{"named":[]},"#,
        r#""output":{"is":"cases","type":{"union":[{"is":"primitive","prim":"INT"},"#,
        r#"{"is":"declared","declared":"m.NotFound"}]},"#,
        r#""cases":[{"is":"primitive","prim":"INT"},{"is":"declared","declared":"m.NotFound"}],"#,
        r#""form":{"is":"discriminated","tag":"type","contents":"value"}},"ensures":{"at":"none"}}],"#,
        r#""modules":[{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
    );

    let refused = object_for(document).expect_err("no object reads a carried Int from another");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "a union no object reads from another is the backend being behind: {refused}"
    );
    assert!(
        refused.to_string().contains("reached across objects"),
        "{refused}"
    );
}

/// Two calls reaching one published value at two different types is not a document this backend
/// is behind on — a value is one declaration and answers one way, so this is the checker and this
/// reading of its document disagreeing about something more basic than a lowering not written
/// yet, and is refused the way any other such disagreement here is, before either call is ever
/// declared a symbol for.
#[test]
fn a_published_value_reached_at_two_different_types_is_the_halves_disagreeing() {
    let document = concat!(
        r#"{"transport":18,"declarations":[],"#,
        r#""behaviors":[{"module":"m","name":"f","is":"body","parameters":{"named":[]},"output":{"is":"scalar","scalar":"INT"},"ensures":{"at":"none"}},"#,
        r#"{"module":"m","name":"g","is":"body","parameters":{"named":[]},"output":{"is":"scalar","scalar":"BOOL"},"ensures":{"at":"none"}}],"#,
        r#""modules":[{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":["#,
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
        r#"{"transport":18,"declarations":[],"#,
        r#""behaviors":[{"module":"m","name":"inner","is":"body","parameters":{"named":[{"name":"p0","input":{"is":"scalar","scalar":"INT"}}]},"output":{"is":"scalar","scalar":"INT"},"ensures":{"at":"none"}},"#,
        r#"{"module":"m","name":"outer","is":"composed","parameters":{"positional":[{"is":"scalar","scalar":"INT"}]},"output":{"is":"scalar","scalar":"INT"},"ensures":{"at":"none"}}],"#,
        r#""modules":[{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":["#,
        r#"{"is":"body","declared":"m.inner","parameters":["a"],"publication":"kept","body":{"core":"read","binding":0,"type":{"prim":"INT"},"aborts":[]}},"#,
        r#"{"is":"composed","declared":"m.outer","publication":"published","stages":["#,
        r#"{"behavior":"m.inner","routing":{"is":"always"}}]}"#,
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
        r#""name":"outer","is":"composed","parameters":{"positional":[{"is":"scalar","scalar":"INT"}]}"#,
        r#""name":"outer","is":"composed","parameters":{"positional":[{"is":"scalar","scalar":"INT"},{"is":"scalar","scalar":"INT"}]}"#,
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
        r#"{"transport":18,"declarations":["#,
        r#"{"module":"m","name":"A","by":"amodule","is":"unit"},"#,
        r#"{"module":"m","name":"B","by":"amodule","is":"unit"}],"#,
        r#""behaviors":[],"#,
        r#""modules":[{"name":"m","publishes":[],"#,
        r#""helpers":[{"reached":{"is":"own","module":"m","name":"f"},"#,
        r#""parameters":[{"name":"f","type":{"fn":{"takes":[],"answers":{"declared":"m.A"}}}}],"#,
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
        r#"{"transport":18,"declarations":["#,
        r#"{"module":"m","name":"Priced","by":"amodule","is":"product","#,
        r#""fields":[{"name":"amount","binding":0,"codec":{"is":"scalar","scalar":"DECIMAL"}}],"invariants":[]}],"#,
        r#""behaviors":[{"module":"m","name":"same","is":"body","#,
        r#""parameters":{"named":[{"name":"p0","input":{"is":"nominal","declared":"m.Priced"}}]},"#,
        r#""output":{"is":"nominal","declared":"m.Priced"},"ensures":{"at":"none"}}],"#,
        r#""modules":[{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":["#,
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
            r#"{{"transport":18,"declarations":["#,
            r#"{{"module":"m","name":"C","by":"amodule","is":"product","fields":{},"invariants":[]}},"#,
            r#"{{"module":"m","name":"S","by":"amodule","is":"sum","#,
            r#""cases":[{{"is":"declared","declared":"m.C"}}],"form":{}}}],"#,
            r#""behaviors":[{{"module":"m","name":"same","is":"body","#,
            r#""parameters":{{"named":[{{"name":"p0","input":{{"is":"nominal","declared":"m.S"}}}}]}},"#,
            r#""output":{{"is":"nominal","declared":"m.S"}},"ensures":{{"at":"none"}}}}],"#,
            r#""modules":[{{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":["#,
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
        r#"[{"name":"n","binding":0,"codec":{"is":"scalar","scalar":"INT"}}]"#,
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
        r#"{"transport":18,"declarations":["#,
        r#"{"module":"m","name":"P","by":"amodule","is":"product","#,
        r#""fields":[{"name":"n","binding":0,"codec":{"is":"scalar","scalar":"STRING"}}],"invariants":[]}],"#,
        r#""behaviors":[{"module":"m","name":"make","is":"body","parameters":{"named":[]},"#,
        r#""output":{"is":"nominal","declared":"m.P"},"ensures":{"at":"none"}}],"#,
        r#""modules":[{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":["#,
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
            r#"{{"transport":18,"declarations":["#,
            r#"{{"module":"m","name":"A","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"B","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"U","by":"amodule","is":"unit"}},"#,
            r#"{{"module":"m","name":"S","by":"amodule","is":"sum","#,
            r#""cases":[{{"is":"declared","declared":"m.A"}},{{"is":"declared","declared":"m.B"}}],"#,
            r#""form":{{"is":"enumeration"}}}},"#,
            r#"{{"module":"m","name":"P","by":"amodule","is":"product","#,
            r#""fields":[{{"name":"f","binding":0,"codec":{}}}],"invariants":[]}}],"#,
            r#""behaviors":[{{"module":"m","name":"make","is":"body","parameters":{{"named":[]}},"#,
            r#""output":{{"is":"nominal","declared":"m.P"}},"ensures":{{"at":"none"}}}}],"#,
            r#""modules":[{{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":["#,
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

/// A field of a sum is given a value of one of its cases standing as the sum, which is the same
/// field and the same value as far as the language is concerned. What is refused is a value no case
/// of the sum is.
#[test]
fn a_field_of_a_sum_given_a_value_of_one_of_its_cases_is_built() {
    let named_sum = r#"{"is":"named","declared":"m.S"}"#;
    let standing = |value: &str| {
        format!(r#"{{"core":"widen","value":{value},"type":{{"declared":"m.S"}},"aborts":[]}}"#)
    };

    object_for(&building(named_sum, &standing(&unit("m.B")))).expect("m.B is a case of m.S");
    let refused = object_for(&building(named_sum, &standing(&unit("m.U"))))
        .expect_err("m.U is no case of m.S");
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
        r#"{"module":"m","name":"U","by":"amodule","is":"newtype","fields":[],"invariants":[]}"#,
    );
    let unit_with_a_field = building(r#"{"is":"scalar","scalar":"INT"}"#, &unit("m.U")).replace(
        r#"{"module":"m","name":"U","by":"amodule","is":"unit"}"#,
        r#"{"module":"m","name":"U","by":"amodule","is":"unit","fields":[],"invariants":[]}"#,
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
        r#"[{"name":"type","binding":0,"codec":{"is":"scalar","scalar":"INT"}}]"#,
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
        r#"{"transport":18,"declarations":["#,
        r#"{"module":"m","name":"A","by":"amodule","is":"unit"},"#,
        r#"{"module":"m","name":"B","by":"amodule","is":"unit"}],"#,
        r#""behaviors":[{"module":"m","name":"either","is":"injected","parameters":{"named":[]},"#,
        r#""output":{"is":"cases","type":{"union":[{"is":"declared","declared":"m.A"},"#,
        r#"{"is":"declared","declared":"m.B"}]},"#,
        r#""cases":[{"is":"declared","declared":"m.A"}],"form":{"is":"enumeration"}},"ensures":{"at":"none"}}],"#,
        r#""modules":[{"name":"m","publishes":[],"helpers":[],"values":[],"entries":[],"definitions":[],"examples":[]}]}"#,
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
        r#""output":{"is":"nominal","declared":"m.P"},"ensures":{"at":"none"}"#,
        r#""output":{"is":"nominal","declared":"m.U"},"ensures":{"at":"none"}"#,
    );
    is_the_halves_disagreeing(&document, "m.make");
}

/// Two targets under one name would leave whichever was read last answering for both.
#[test]
fn two_targets_written_the_same_are_the_halves_disagreeing() {
    let inner = r#"{"module":"m","name":"inner","is":"body","parameters":{"named":[{"name":"p0","input":{"is":"scalar","scalar":"INT"}}]},"output":{"is":"scalar","scalar":"INT"},"ensures":{"at":"none"}}"#;
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
