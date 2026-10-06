//! A type's reading of its external form, and a constructor, as raoh decoders a host composes with
//! its own, as a JVM host composes a type's `decoder()`: the issues of both come back together.
//!
//! A raoh decoder is `Fn` and answers issues or a value, and nothing else. The model's steps read in
//! a run, which is `&mut`, and may end it for a reason of their own, which is no issue of the
//! input. So a run is lent to the decoders of one decode through a [`Decoding`], which holds the
//! run and the first failure a step answered: the step answers no issue then, which stops its
//! field and no other, and the failure is what the decode comes to once it is done.

use std::cell::RefCell;

use raoh::json::{Json, View};
use raoh::{Decoder, Issue, Issues, codes, decoder_fn};

use crate::native::{Construction, Reading};
use crate::{Failure, Loaded, Run};

/// A run lent to the raoh decoders of one decode, and the first failure one of the model's steps
/// answered in it.
pub struct Decoding<'d, 'run, L> {
    run: RefCell<&'d mut Run<'run, L>>,
    failed: RefCell<Option<Failure>>,
}

impl<'d, 'run, L: Loaded> Decoding<'d, 'run, L> {
    /// What `decode` comes to, with the decoders it makes reading in `run`: the value, or the
    /// issues found, unless one of the model's steps ended the run, which is the `Err` outside.
    ///
    /// # Errors
    ///
    /// The first failure a step of the model's answered.
    pub fn read<T>(
        run: &'d mut Run<'run, L>,
        decode: impl FnOnce(&Self) -> Result<T, Issues>,
    ) -> Result<Result<T, Issues>, Failure> {
        let decoding = Decoding {
            run: RefCell::new(run),
            failed: RefCell::new(None),
        };
        let read = decode(&decoding);
        match decoding.failed.into_inner() {
            Some(failure) => Err(failure),
            None => Ok(read),
        }
    }

    /// `make`, which makes a value of the model of its input or answers what it refused it for,
    /// as a raoh decoder: a constructor, or a type's reading. What it refuses is an issue at the
    /// path the decoder is reached at.
    pub fn of<'a, I: ?Sized, T, A: Answered<T>>(
        &'a self,
        make: impl Fn(&mut Run<'run, L>, &I) -> Result<A, Failure> + 'a,
    ) -> impl Decoder<I, Output = T> + 'a {
        decoder_fn(move |input: &I, path| {
            let answered = make(&mut self.run.borrow_mut(), input);
            match answered.map(Answered::into_result) {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(issues)) => Err(issues.rebase(path)),
                Err(failure) => {
                    self.failed.borrow_mut().get_or_insert(failure);
                    Err(Issues::new())
                }
            }
        })
    }

    /// `decode`, a type's reading of its external form, as a raoh decoder of what a host decoded:
    /// the value of raoh's input model is written back as the JSON it is, each number as the text
    /// it was written as, and read by the library. A member that is not there is raoh's to report,
    /// as it is for any field, since there is no text to hand over.
    pub fn reading<'a, T>(
        &'a self,
        decode: impl Fn(&mut Run<'run, L>, &str) -> Result<Reading<T>, Failure> + 'a,
    ) -> impl Decoder<Json, Output = T> + 'a {
        let read = self.of(move |run, input: &Json| {
            let mut text = String::new();
            written(input, &mut text);
            decode(run, &text)
        });
        decoder_fn(move |input: &Json, path| {
            if raoh::json::is_missing(input) {
                return Err(Issue::new(codes::REQUIRED).at(path.to_pointer()).into());
            }
            read.decode_at(input, path)
        })
    }
}

/// What a constructor or a type's reading answers: a value, or what it was refused for.
pub trait Answered<T> {
    /// The value, or the issues it was refused for.
    ///
    /// # Errors
    ///
    /// What the value was refused for.
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

/// `input` as JSON text: a number as its lexeme, so `1.50` reaches the library as `1.50` and not as
/// the float nearest it, and an object's members in the order raoh hands them. A member that is not
/// there writes nothing, and is never handed here: [`Decoding::reading`] reports it first.
fn written(input: &Json, out: &mut String) {
    match input.view() {
        View::Missing => {}
        View::Null => out.push_str("null"),
        View::Bool(truth) => out.push_str(if truth { "true" } else { "false" }),
        View::Number(number) => out.push_str(&number.lexeme()),
        View::String(text) => out.push_str(
            &serde_json::to_string(text).expect("a string is written as JSON whatever it holds"),
        ),
        View::Array(elements) => {
            out.push('[');
            for (at, element) in elements.iter().enumerate() {
                if at > 0 {
                    out.push(',');
                }
                written(element, out);
            }
            out.push(']');
        }
        View::Object(members) => {
            out.push('{');
            let mut first = true;
            members.each(&mut |name, value| {
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str(
                    &serde_json::to_string(name)
                        .expect("a name is written as JSON whatever it holds"),
                );
                out.push(':');
                written(value, out);
            });
            out.push('}');
        }
    }
}
