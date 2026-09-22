//! Every word the writer can write, read back as the member it names.
//!
//! A vocabulary the language closed is spelt twice — once by the writer, once by the enum in
//! `transport` — and neither copy holds the other to anything. A member spelt differently on the
//! two sides is loud: this side refuses a word it does not read, and the caller is told the two
//! halves disagree rather than that the backend is behind. A member spelt the way *another member
//! of the same vocabulary* is spelt is not loud at all. The document reads, and the program means
//! something other than what it says.
//!
//! What closes that is not a comment on either side. It is this: the writer writes every spelling
//! it can write to a document, its own test asserts the document is what it writes, and this names
//! the member it expects at each place. The order of each list is the correspondence between the
//! two halves' members — there is no other way to state it, since the two name their members
//! differently (`A_MODULE_ON_THE_PATH` here is `OnThePath`) — so it is written out rather than
//! derived.
//!
//! A member added upstream stops the writer compiling, which forces the document to change, which
//! brings this down until this side answers for it too.

use serde::Deserialize;
use souther_native_driver::transport::{
    AbortKind, DeclaredBy, Op, Prim, Publication, TRANSPORT_VERSION,
};

/// The document the writer wrote, and the one its own test holds it to.
const VOCABULARIES: &str = include_str!("vocabularies.transport.json");

/// Read strictly, as a program is: a vocabulary this does not name is the writer spelling out
/// something this driver has no idea it was told.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vocabularies {
    transport: u32,
    op: Vec<Op>,
    prim: Vec<Prim>,
    publication: Vec<Publication>,
    declaredby: Vec<DeclaredBy>,
    abort: Vec<AbortKind>,
}

fn read() -> Vocabularies {
    serde_json::from_str(VOCABULARIES).expect("every word in it is one this side reads")
}

/// The document is one this driver reads at all, so a spelling moving is told apart from the two
/// halves having gone to different versions.
#[test]
fn the_vocabularies_crossed_at_the_transport_this_driver_reads() {
    assert_eq!(read().transport, TRANSPORT_VERSION);
}

/// Every operator, including the two no lowering exists for. What an operator means is the
/// language's and whether this can write it is this driver's, and the second is not a reason for
/// the first to go unread.
#[test]
fn every_operator_is_read_as_the_operator_it_names() {
    assert_eq!(
        read().op,
        vec![
            Op::Eq,
            Op::Ne,
            Op::Lt,
            Op::Le,
            Op::Gt,
            Op::Ge,
            Op::And,
            Op::Or,
            Op::Add,
            Op::Sub,
            Op::Mul,
            Op::Div,
            Op::Concat,
        ]
    );
}

/// Every primitive, including the ones with no representation. Same reason.
#[test]
fn every_primitive_is_read_as_the_primitive_it_names() {
    assert_eq!(
        read().prim,
        vec![
            Prim::Int,
            Prim::String,
            Prim::Bool,
            Prim::Decimal,
            Prim::Rational,
            Prim::Date,
            Prim::Time,
            Prim::DateTime,
            Prim::Instant,
            Prim::Raw,
        ]
    );
}

/// Both answers a module can give about a name it declares. These two decide what the object's
/// symbol table carries, so the two of them standing for each other would be an object publishing
/// exactly what the module kept.
#[test]
fn both_publications_are_read_as_the_publication_they_name() {
    assert_eq!(
        read().publication,
        vec![Publication::Published, Publication::Kept]
    );
}

/// All three provenances. These decide who defines the token a value of a declared type is tagged
/// by, so `amodule` read as `onthepath` would be an object naming a token no build defines, and
/// `onthepath` read as `amodule` two builds each defining one.
#[test]
fn every_provenance_is_read_as_the_provenance_it_names() {
    assert_eq!(
        read().declaredby,
        vec![
            DeclaredBy::AModule,
            DeclaredBy::TheLanguage,
            DeclaredBy::OnThePath,
        ]
    );
}

/// Every reason a run ends without a value, including the ones no site this driver reads yet ever
/// answers with. What a member means is the checker's; whether any site here can reach it is a
/// separate question this does not ask.
#[test]
fn every_abort_kind_is_read_as_the_abort_kind_it_names() {
    assert_eq!(
        read().abort,
        vec![
            AbortKind::InvariantNotHeld,
            AbortKind::EnsuresNotHeld,
            AbortKind::UnreachableReached,
            AbortKind::DivisionByZero,
            AbortKind::RequiredFormHasNoPlace,
            AbortKind::InvalidBounds,
        ]
    );
}
