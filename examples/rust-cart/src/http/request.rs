//! Request bodies, read into the arguments of a behavior.
//!
//! Two parties read a request, and each owns a different part of what it means. The model owns
//! what a value is: which case an orderer is, the fields each case has, and every rule a type
//! states, a positive quantity, a name that is not blank and a corporate number of thirteen digits
//! among them. Nothing here says any of that again. The boundary owns how a value is written from
//! outside: the canonical form of what a client sends (an id in lower case, an email and a name
//! without the spaces around them, an email in lower case), and the forms the model leaves to it (an
//! id is a UUID, an email is shaped like one). raoh does the boundary's part.
//!
//! The model's decoders are reached in two ways only, and which one is decided by what the
//! boundary's step is about:
//!
//! - [`Model::after`], where the boundary checks and canonicalises one value and the model reads what
//!   it answers. Both speak of that one value, so where the boundary refuses it the model has
//!   nothing to add: its issue would be at the same path.
//! - [`Model::canonicalised`], where the model reads a value whole and the boundary puts some of its
//!   members in their canonical form first. Each member is canonicalised on its own and the model
//!   reads the value whichever of them the boundary refused, so a refused email does not keep the
//!   model from saying that a company name is missing.
//!
//! A request has no type of its own in the model, since a behavior takes its arguments by place, so
//! each argument is a field here and read by its type's decoder. Every field is read whichever of
//! them fails, and together with the two ways above that makes a request answer every issue it has
//! at once, the boundary's and the model's.
//!
//! The model's step is a raoh decoder like any other. It is made from the run it reads in, which a
//! decoder can borrow but not hold, so the decoders here live for one call.

use std::cell::RefCell;

use model::com::example::cart::domain::{Orderer, ProductId, Quantity, UserId};
use model::{Construction, Failure, Reading, Run};
use raoh::json::prelude::*;
use raoh::{Issues, Pointer, decoder_fn};

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
                model.canonicalised(Vec::new(), |run, it| Quantity::decode(run, &it.to_string())),
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
            field("orderer", orderer(model)),
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

/// A UUID's text, as the database keeps one: in lower case, with its hyphens.
fn uuid() -> impl Decoder<Value, Output = String> {
    string().uuid().map(|id| id.to_string())
}

/// An orderer in the model's own encoding of one, read whole by the model once the members whose
/// canonical form is the boundary's are in it. Whether each is there at all, and what it has to
/// be, is the model's to say, as it is of every other member.
fn orderer<'a, 'run>(
    model: &'a Model<'_, 'run>,
) -> impl Decoder<Value, Output = Orderer<'run>> + 'a {
    model.canonicalised(
        vec![
            (
                "email",
                string()
                    .trim()
                    .lowercase()
                    .email()
                    .map(Value::String)
                    .boxed(),
            ),
            ("name", trimmed()),
            ("companyName", trimmed()),
        ],
        |run, it| Orderer::decode(run, &it.to_string()),
    )
}

/// Text without the spaces around it, which is how a name is written whatever a client sent.
fn trimmed() -> BoxDecoder<Value, Value> {
    string().trim().map(Value::String).boxed()
}

/// A member of a value the model reads whole, and how the boundary writes it canonically.
type Canonical = (&'static str, BoxDecoder<Value, Value>);

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

    /// One value, checked and canonicalised by `form`, and then read by the model through `make`
    /// from what `form` answered.
    fn after<'a, O, T, A: Answered<T>>(
        &'a self,
        form: impl Decoder<Value, Output = O> + 'a,
        make: impl Fn(&mut Run<'run>, &O) -> Result<A, Failure> + 'a,
    ) -> impl Decoder<Value, Output = T> + 'a {
        form.pipe(self.of(make))
    }

    /// A value read whole by the model through `make`, with each of `members` put in the
    /// boundary's canonical form first where the value has it.
    ///
    /// Every member is canonicalised on its own, and the model reads the value whichever of them
    /// was refused, with a refused member as it was given. So the issues of both come back
    /// together. Where both speak of one member, the boundary's is the one kept: it says what form
    /// the member was not in, and the model's could only say that the member did not hold.
    fn canonicalised<'a, T, A: Answered<T>>(
        &'a self,
        members: Vec<Canonical>,
        make: impl Fn(&mut Run<'run>, &Value) -> Result<A, Failure> + 'a,
    ) -> impl Decoder<Value, Output = T> + 'a {
        let whole = self.of(make);
        decoder_fn(move |given: &Value, path| {
            let mut canonical = given.clone();
            let mut issues = Issues::new();
            let mut refused: Vec<Pointer> = Vec::new();
            if let Value::Object(object) = &mut canonical {
                for (name, form) in &members {
                    if let Some(member) = object.get_mut(*name) {
                        let at = path.key(name);
                        match form.decode_at(member, &at) {
                            Ok(written) => *member = written,
                            Err(found) => {
                                refused.push(at.to_pointer());
                                issues.merge(found);
                            }
                        }
                    }
                }
            }
            match whole.decode_at(&canonical, path) {
                Ok(read) if issues.is_empty() => Ok(read),
                Ok(_) => Err(issues),
                Err(found) => {
                    for issue in found.iter() {
                        let spoken = refused
                            .iter()
                            .any(|member| issue.path().segments().starts_with(member.segments()));
                        if !spoken {
                            issues.push(issue.clone());
                        }
                    }
                    Err(issues)
                }
            }
        })
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
