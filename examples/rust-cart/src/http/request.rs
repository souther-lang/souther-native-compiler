//! Request bodies, read into the arguments of a behavior.
//!
//! Two parties read a request, and each owns a different part of what it means. The model owns
//! what a value is: which case an orderer is, the fields each case has, and every rule a type
//! states, a positive quantity, a name that is not blank and a corporate number of thirteen digits
//! among them. Nothing here says any of that again. The boundary owns how a client writes a value:
//! an id is a UUID in lower case, an email is trimmed, lowercased and shaped like one, a name is
//! trimmed. Each of those is a raoh decoder, which writes the value in its form and refuses what
//! cannot be written so, as one step.
//!
//! The model's decoders are the binding's, `Quantity::decoder(decoding)`, and a constructor is made
//! into one with `decoding.of(..)`: each reads in the run the request is read in, which a
//! [`Decoding`] lends them for one decode. They are composed with the boundary's in two ways, by
//! what the boundary owns of the value:
//!
//! - piped, where the boundary owns the value the model reads, an id: the model reads what the
//!   boundary's decoder answered, and nothing where it refused, since its issue would be at the same
//!   path.
//! - [`boundary::members`], where the model reads a value whole and the boundary owns some of its
//!   members, an orderer's email and names.
//!
//! Where the boundary owns nothing of a value, a quantity, the model's decoder reads it alone.
//!
//! A request has no type of its own in the model, since a behavior takes its arguments by place, so
//! each argument is a field here and read by its type's decoder. Every field is read whichever of
//! them fails, and with the ways above that makes a request answer every issue it has at once, the
//! boundary's and the model's.

use model::com::example::cart::domain::{Orderer, ProductId, Quantity, UserId};
use model::{Construction, Decoding, Failure, Run};
use raoh::Issues;
use raoh::json::prelude::*;

use super::boundary;

/// What a request comes to: its arguments, or the issues found in it. The run ending while it was
/// read is the `Err` outside.
pub type Read<T> = Result<Result<T, Issues>, Failure>;

/// `{"userId":"…","productId":"…","quantity":n}` as the arguments of `addItemToCart`.
pub fn add_item<'run>(
    run: &mut Run<'run>,
    body: &str,
) -> Read<(UserId<'run>, ProductId<'run>, Quantity<'run>)> {
    Decoding::read(run, |decoding| {
        let arguments = object((
            field("userId", id(decoding, UserId::new)),
            field("productId", id(decoding, ProductId::new)),
            field("quantity", Quantity::decoder(decoding)),
        ));
        from_str(&arguments, body)
    })
}

/// `{"userId":"…","orderer":{…}}` as a user and an orderer.
pub fn checkout<'run>(run: &mut Run<'run>, body: &str) -> Read<(UserId<'run>, Orderer<'run>)> {
    Decoding::read(run, |decoding| {
        let arguments = object((
            field("userId", id(decoding, UserId::new)),
            field(
                "orderer",
                boundary::members(
                    vec![
                        ("email", text(string().trim().lowercase().email())),
                        ("name", text(string().trim())),
                        ("companyName", text(string().trim())),
                    ],
                    Orderer::decoder(decoding),
                ),
            ),
        ));
        from_str(&arguments, body)
    })
}

/// The `userId` of a query, which is absent where the query has none.
pub fn user_id<'run>(run: &mut Run<'run>, given: Option<&str>) -> Read<UserId<'run>> {
    let given = given.map_or(Node::Null, |it| Node::String(it.to_owned()));
    Decoding::read(run, |decoding| id(decoding, UserId::new).decode(&given))
}

/// An id the boundary decodes as a UUID, made into a value of the model by `make`, its
/// constructor, from what the boundary answered.
fn id<'a, 'run: 'a, T: 'a>(
    decoding: &'a Decoding<'_, 'run>,
    make: fn(&mut Run<'run>, &str) -> Result<Construction<T>, Failure>,
) -> impl Decoder<Json, Output = T> + 'a {
    uuid().pipe(decoding.of(move |run, id: &String| make(run, id)))
}

/// A UUID as this API writes one, and as the database keeps it: in lower case, with its hyphens.
/// Another notation of one (braced, a URN, without hyphens) is not how a client writes an id here.
fn uuid() -> impl Decoder<Json, Output = String> {
    string().lowercase().refine(
        |text: &String| hyphenated(text),
        "invalid_format",
        "not a valid UUID",
    )
}

/// Whether `text` is 32 hexadecimal digits in lower case, in groups of 8, 4, 4, 4 and 12 joined by
/// hyphens.
fn hyphenated(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(at, digit)| match at {
            8 | 13 | 18 | 23 => digit == '-',
            _ => digit.is_ascii_digit() || ('a'..='f').contains(&digit),
        })
}

/// A member the boundary writes as text, `decoder` saying how.
fn text(
    decoder: impl Decoder<Json, Output = String> + Send + Sync + 'static,
) -> BoxDecoder<Json, Node> {
    decoder.map(Node::String).boxed()
}
