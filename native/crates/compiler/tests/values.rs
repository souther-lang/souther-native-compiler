//! What the Java half writes for a value, its handovers, and the entry it publishes for one —
//! read back and, since this backend does not build any of it yet, refused rather than silently
//! left out of the object (review of #20: `values`/`entries` went unread by `object_for` before
//! this, so a document naming one still produced a "successful" object with no published surface
//! for it).

use souther_native_driver::transport::{Program, Reaches};
use souther_native_driver::{NotLowered, object_for};

/// A value that names another value at its root: `ks`, kept and handed nothing, and `ys`,
/// published and handed one `ks`.
const VALUES: &str = include_str!("values.transport.json");

/// A behavior in one module answering with a value another module publishes.
const PUBLISHED_VALUE: &str = include_str!("published_value.transport.json");

#[test]
fn a_value_and_its_handover_read_back_as_the_checker_wrote_them() {
    let program: Program = serde_json::from_str(VALUES).expect("every field this carries reads");
    let module = &program.modules[0];

    assert_eq!(module.values.len(), 2, "ks and ys");
    let ks = module.values.iter().find(|it| it.name == "ks").expect("ks is a value");
    let ys = module.values.iter().find(|it| it.name == "ys").expect("ys is a value");

    assert!(ks.handovers.is_empty(), "ks is handed nothing");
    assert_eq!(ys.handovers.len(), 1, "ys names ks at its root");
    assert_eq!(ys.handovers[0].carries.declared(), "m.ks");

    assert_eq!(module.entries.len(), 1, "only ys is published");
    assert_eq!(module.entries[0].value.declared(), "m.ys");
}

/// The same document, refused whole rather than silently missing the value it carries: this
/// backend has nowhere yet to put the once-semantics a value needs (#10), and a module with one
/// must say so rather than hand back an object that answers for everything else and simply has
/// no entry for `m.ys`.
#[test]
fn a_document_carrying_a_value_is_refused_rather_than_silently_missing_it() {
    let refused = object_for(VALUES).expect_err("this backend builds no value yet");

    assert!(
        refused.downcast_ref::<NotLowered>().is_some(),
        "a value going unbuilt is this backend not there yet, not the halves disagreeing: \
         {refused}"
    );
    assert!(refused.to_string().contains('m'), "{refused}");
}

/// A call reaching a value declared in the same module, and a call reaching one published by
/// another, are different `Reaches` variants on the wire — never the one tag doing for both, which
/// is exactly the distinction a typo in either spelling would erase without a test reading a real
/// occurrence of each back.
#[test]
fn a_local_value_reach_and_a_published_one_read_as_different_variants() {
    let program: Program = serde_json::from_str(PUBLISHED_VALUE)
        .expect("every field this carries reads");

    let publisher = program.modules.iter().find(|it| it.name == "publisher").unwrap();
    let entry = &publisher.entries[0];
    // The entry's own body reaches ks and then ys, both same-module: Reaches::Value.
    let mut local_reaches = 0;
    walk(&entry.body, &mut |reaches| {
        if matches!(reaches, Reaches::Value { .. }) {
            local_reaches += 1;
        }
        assert!(
            !matches!(reaches, Reaches::PublishedValue { .. }),
            "the entry's own body never crosses a module boundary"
        );
    });
    assert_eq!(local_reaches, 2, "the entry builds ks, then ys");

    let reader = program.modules.iter().find(|it| it.name == "reader").unwrap();
    let mut published_reaches = 0;
    for definition in &reader.definitions {
        walk(definition_body(definition), &mut |reaches| {
            if let Reaches::PublishedValue { module, name } = reaches {
                assert_eq!(module, "publisher");
                assert_eq!(name, "ys");
                published_reaches += 1;
            }
            assert!(
                !matches!(reaches, Reaches::Value { .. }),
                "reader holds no value of its own to reach locally"
            );
        });
    }
    assert_eq!(published_reaches, 1, "reader.g reaches publisher.ys across the boundary");
}

/// This backend has nowhere to lower a call to either kind of value yet, so a program naming one
/// across a module boundary is refused the same as one that only names a value locally.
#[test]
fn a_published_value_call_is_also_refused_rather_than_silently_missing_it() {
    let refused = object_for(PUBLISHED_VALUE).expect_err("this backend builds no value yet");

    assert!(refused.downcast_ref::<NotLowered>().is_some(), "{refused}");
}

fn definition_body(
    definition: &souther_native_driver::transport::Definition,
) -> &souther_native_driver::transport::Node {
    match definition {
        souther_native_driver::transport::Definition::Body { body, .. } => body,
        souther_native_driver::transport::Definition::Composed { .. } => {
            panic!("reader.g is a body, not a composition")
        }
    }
}

/// Every `Reaches` a `Node` tree's calls carry, depth first — the same walk `refusals.rs`'s own
/// documents are small enough not to need, but this one's isn't.
fn walk(node: &souther_native_driver::transport::Node, into: &mut impl FnMut(&Reaches)) {
    use souther_native_driver::transport::Node;
    match node {
        Node::Call { reaches, arguments, .. } => {
            into(reaches);
            for argument in arguments {
                walk(argument, into);
            }
        }
        Node::Let { value, body, .. } => {
            walk(value, into);
            walk(body, into);
        }
        _ => {}
    }
}
