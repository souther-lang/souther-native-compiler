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
//! The model's decoders are reached in three ways, by what the boundary owns of the value:
//!
//! - [`Model::after`], where the boundary owns the value the model reads, an id: the model reads
//!   what the boundary's decoder answered, and nothing where it refused, since its issue would be
//!   at the same path.
//! - [`Model::members`], where the model reads a value whole and the boundary owns some of its
//!   members, an orderer's email and names: [`boundary::members`] says how the two are put together.
//! - [`Model::as_given`], where the boundary owns nothing of it, a quantity.
//!
//! A request has no type of its own in the model, since a behavior takes its arguments by place, so
//! each argument is a field here and read by its type's decoder. Every field is read whichever of
//! them fails, and with the ways above that makes a request answer every issue it has at once, the
//! boundary's and the model's.
//!
//! The model's step is a raoh decoder like any other. It is made from the run it reads in, which a
//! decoder can borrow but not hold, so the decoders here live for one call.

use std::cell::RefCell;

use model::com::example::cart::domain::{Orderer, ProductId, Quantity, UserId};
use model::{Construction, Failure, Reading, Run};
use raoh::json::prelude::*;
use raoh::{Issues, decoder_fn};

use super::boundary::{self, Member};

/// What a request comes to: its arguments, or the issues found in it. The run ending while it was
/// read is the `Err` outside.
pub type Read<T> = Result<Result<T, Issues>, Failure>;

/// `{"userId":"…","productId":"…","quantity":n}` as the arguments of `addItemToCart`.
pub fn add_item<'run>(
    run: &mut Run<'run>,
    body: &str,
) -> Read<(UserId<'run>, ProductId<'run>, Quantity<'run>)> {
    Model::read(run, |model| {
        let arguments = object((
            field(
                "userId",
                model.after(uuid(), |run, id: &String| UserId::new(run, id)),
            ),
            field(
                "productId",
                model.after(uuid(), |run, id: &String| ProductId::new(run, id)),
            ),
            field(
                "quantity",
                model.as_given(|run, it| Quantity::decode(run, &it.to_string())),
            ),
        ));
        from_str(&arguments, body)
    })
}

/// `{"userId":"…","orderer":{…}}` as a user and an orderer.
pub fn checkout<'run>(run: &mut Run<'run>, body: &str) -> Read<(UserId<'run>, Orderer<'run>)> {
    Model::read(run, |model| {
        let arguments = object((
            field(
                "userId",
                model.after(uuid(), |run, id: &String| UserId::new(run, id)),
            ),
            field(
                "orderer",
                model.members(
                    vec![
                        ("email", text(string().trim().lowercase().email())),
                        ("name", text(string().trim())),
                        ("companyName", text(string().trim())),
                    ],
                    |run, it| Orderer::decode(run, &it.to_string()),
                ),
            ),
        ));
        from_str(&arguments, body)
    })
}

/// The `userId` of a query, which is absent where the query has none.
pub fn user_id<'run>(run: &mut Run<'run>, given: Option<&str>) -> Read<UserId<'run>> {
    let given = given.map_or(Value::Null, |it| Value::String(it.to_owned()));
    Model::read(run, |model| {
        model
            .after(uuid(), |run, id: &String| UserId::new(run, id))
            .decode(&given)
    })
}

/// A UUID as this API writes one, and as the database keeps it: in lower case, with its hyphens.
/// Another notation of one (braced, a URN, without hyphens) is not how a client writes an id here.
fn uuid() -> impl Decoder<Value, Output = String> {
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
    decoder: impl Decoder<Value, Output = String> + Send + Sync + 'static,
) -> BoxDecoder<Value, Value> {
    decoder.map(Value::String).boxed()
}

/// The run the model's steps read in, and the first failure one of them answered.
struct Model<'m, 'run> {
    run: RefCell<&'m mut Run<'run>>,
    failed: RefCell<Option<Failure>>,
}

impl<'m, 'run> Model<'m, 'run> {
    /// What `decode` comes to over `run`, unless one of the model's steps ended the run.
    fn read<T>(run: &'m mut Run<'run>, decode: impl FnOnce(&Self) -> Result<T, Issues>) -> Read<T> {
        let model = Model {
            run: RefCell::new(run),
            failed: RefCell::new(None),
        };
        let read = decode(&model);
        match model.failed.into_inner() {
            Some(failure) => Err(failure),
            None => Ok(read),
        }
    }

    /// A value the boundary owns, decoded by `boundary`, and then read by the model through `make`
    /// from what `boundary` answered.
    fn after<'a, O, T, A: Answered<T>>(
        &'a self,
        boundary: impl Decoder<Value, Output = O> + 'a,
        make: impl Fn(&mut Run<'run>, &O) -> Result<A, Failure> + 'a,
    ) -> impl Decoder<Value, Output = T> + 'a {
        boundary.pipe(self.of(make))
    }

    /// A value read whole by the model through `make`, once each of `members` it has is decoded by
    /// the boundary.
    fn members<'a, T, A: Answered<T>>(
        &'a self,
        members: Vec<Member>,
        make: impl Fn(&mut Run<'run>, &Value) -> Result<A, Failure> + 'a,
    ) -> impl Decoder<Value, Output = T> + 'a {
        boundary::members(members, self.of(make))
    }

    /// A value the boundary owns nothing of, read by the model through `make` as it came.
    fn as_given<'a, T, A: Answered<T>>(
        &'a self,
        make: impl Fn(&mut Run<'run>, &Value) -> Result<A, Failure> + 'a,
    ) -> impl Decoder<Value, Output = T> + 'a {
        self.of(make)
    }

    /// A decoder reading its input into a value of the model through `make`, a constructor or a
    /// type's decoder. What the model refuses is an issue where the decoder is.
    fn of<'a, I: ?Sized, T, A: Answered<T>>(
        &'a self,
        make: impl Fn(&mut Run<'run>, &I) -> Result<A, Failure> + 'a,
    ) -> impl Decoder<I, Output = T> + 'a {
        decoder_fn(move |input: &I, path| {
            let answered = make(&mut self.run.borrow_mut(), input);
            match answered.map(Answered::into_result) {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(issues)) => Err(issues.rebase(path)),
                Err(failure) => {
                    // The run has ended for a reason of its own, which is answered once the
                    // decoder is done. An empty set of issues stops this field and no other.
                    self.failed.borrow_mut().get_or_insert(failure);
                    Err(Issues::new())
                }
            }
        })
    }
}

/// What a constructor or a decoder of the model answers: a value, or what it was refused for.
trait Answered<T> {
    fn into_result(self) -> Result<T, Issues>;
}

impl<T> Answered<T> for Construction<T> {
    fn into_result(self) -> Result<T, Issues> {
        Construction::into_result(self).map_err(Issues::from)
    }
}

impl<T> Answered<T> for Reading<T> {
    fn into_result(self) -> Result<T, Issues> {
        Reading::into_result(self)
    }
}
