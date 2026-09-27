//! Request bodies, read into the arguments of a behavior in two steps.
//!
//! raoh checks the form of each field and normalises it: a UUID in lower case, a positive
//! quantity, an email trimmed, lowercased and shaped like one, a corporate number of thirteen
//! digits, which the model has no regular expression to say. What it hands on is read by the
//! model: which case an orderer is, the fields each case has, and what each type states. Either
//! failing is an issue under the field's path, and every field is read, so a request answers every
//! issue it has at once.
//!
//! The model's step is a raoh decoder like any other, piped after the form's. It is made from the
//! run it reads in, which a decoder can borrow but not hold, so the decoders here live for one call.

use std::cell::RefCell;

use model::com::example::cart::domain::{Orderer, ProductId, Quantity, UserId};
use model::{Construction, Failure, Reading, Run};
use raoh::json::prelude::*;
use raoh::{Issues, decoder_fn};

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
                uuid().pipe(model.of(|run, id: &String| UserId::new(run, id))),
            ),
            field(
                "productId",
                uuid().pipe(model.of(|run, id: &String| ProductId::new(run, id))),
            ),
            field(
                "quantity",
                i64()
                    .positive()
                    .pipe(model.of(|run, n: &i64| Quantity::new(run, *n))),
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
                uuid().pipe(model.of(|run, id: &String| UserId::new(run, id))),
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
        uuid()
            .pipe(model.of(|run, id: &String| UserId::new(run, id)))
            .decode(&given)
    })
}

/// A UUID's text, as the database keeps one: in lower case, with its hyphens.
fn uuid() -> impl Decoder<Value, Output = String> {
    string().uuid().map(|id| id.to_string())
}

/// An orderer, `{"type":"Individual","email":"…","name":"…"}` or
/// `{"type":"Corporation","email":"…","companyName":"…","corporateNumber":"…"}`: the model's own
/// encoding of one. raoh checks the fields that are there, and the model reads the whole.
fn orderer<'a, 'run>(
    model: &'a Model<'_, 'run>,
) -> impl Decoder<Value, Output = Orderer<'run>> + 'a {
    object((
        field("type", string()),
        field("email", string().trim().lowercase().email()),
        optional_field("name", string().trim().non_blank().max_length(100)),
        optional_field("companyName", string().trim().non_blank().max_length(200)),
        optional_field("corporateNumber", string().pattern(r"^\d{13}$")),
    ))
    .map(|(kind, email, name, company_name, corporate_number)| {
        let mut normalised = serde_json::Map::new();
        normalised.insert("type".into(), Value::String(kind));
        normalised.insert("email".into(), Value::String(email));
        for (key, given) in [
            ("name", name),
            ("companyName", company_name),
            ("corporateNumber", corporate_number),
        ] {
            if let Some(given) = given {
                normalised.insert(key.into(), Value::String(given));
            }
        }
        Value::Object(normalised)
    })
    .pipe(model.of(|run, it: &Value| Orderer::decode(run, &it.to_string())))
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
