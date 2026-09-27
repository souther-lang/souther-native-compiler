//! A library as a shared library a process loaded, and the runtime's own functions in it.

use crate::decimal::Decimal;
use crate::failure::{Failure, Statuses, UnnamedStatus};
use crate::run::{Loaded, MarkFn, ResetFn, Run, Runtime};
use crate::temporal::{Date, DateTime, Instant, Time};
use std::fmt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

/// One word the library hands over or is handed that is an address: a value, a string, a list and
/// the rest, which only the library reads behind.
pub type Word = *const u8;

/// A shared library a Souther library was built into, loaded by path, and its functions looked up
/// through it and no other.
///
/// Every Souther library exports the same runtime functions and names its behaviors without
/// naming the library, so two loaded into one process would be told apart by nothing a link could
/// say. Each is reached through its own handle here instead, as the PHP runtime reaches each
/// through its own FFI handle.
pub struct NativeLibrary {
    library: libloading::Library,
    path: PathBuf,
}

impl NativeLibrary {
    /// The library at `path`.
    ///
    /// # Safety
    ///
    /// Loading a library runs what it runs when it is loaded, and every function looked up in it
    /// is called as what its binding says it is: `path` is a library built by souther-native-
    /// compiler from the program the binding was generated from.
    ///
    /// # Errors
    ///
    /// [`LoadError::Open`] where the loader has no library there.
    pub unsafe fn load(path: impl AsRef<Path>) -> Result<Self, LoadError> {
        let path = path.as_ref().to_path_buf();
        // SAFETY: what the caller says.
        match unsafe { libloading::Library::new(&path) } {
            Ok(library) => Ok(NativeLibrary { library, path }),
            Err(source) => Err(LoadError::Open {
                path,
                reason: source.to_string(),
            }),
        }
    }

    /// Where it was loaded from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The function `name`, as `F`.
    ///
    /// # Safety
    ///
    /// `F` is the function pointer type the function is, and it is called only while this library
    /// is loaded.
    ///
    /// # Errors
    ///
    /// [`LoadError::Missing`] where the library exports nothing of that name.
    pub unsafe fn function<F: Copy>(&self, name: &str) -> Result<F, LoadError> {
        // SAFETY: what the caller says.
        match unsafe { self.library.get::<F>(name.as_bytes()) } {
            Ok(symbol) => Ok(*symbol),
            Err(source) => Err(LoadError::Missing {
                name: name.to_owned(),
                reason: source.to_string(),
            }),
        }
    }

    /// The runtime this library was built with, numbering its statuses as `statuses` does.
    ///
    /// # Safety
    ///
    /// `statuses` is what the library's manifest numbers them, and the runtime is used only while
    /// this library is loaded.
    ///
    /// # Errors
    ///
    /// Where the library has no `souther_mark` or `souther_reset`, or `statuses` does not name one
    /// a host has to tell apart.
    pub unsafe fn runtime(
        &self,
        statuses: &'static [(&'static str, u32)],
    ) -> Result<Runtime, LoadError> {
        let statuses = Statuses::new(statuses).map_err(LoadError::Status)?;
        // SAFETY: both are the runtime's, as the ABI states them, and the caller keeps this
        // library loaded while the runtime is used.
        unsafe {
            let mark: MarkFn = self.function("souther_mark")?;
            let reset: ResetFn = self.function("souther_reset")?;
            Ok(Runtime::new(mark, reset, statuses))
        }
    }
}

/// Why a library could not be loaded as its binding says it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The loader has no library at the path.
    Open { path: PathBuf, reason: String },
    /// The library exports no function of the name the binding calls.
    Missing { name: String, reason: String },
    /// The binding names no status a host has to tell apart.
    Status(UnnamedStatus),
    /// The binding names no outcome a reading comes to.
    Outcome(&'static str),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Open { path, reason } => {
                write!(
                    f,
                    "no library could be loaded from {}: {reason}",
                    path.display()
                )
            }
            LoadError::Missing { name, reason } => write!(
                f,
                "the library has no function {name}, which its binding calls: {reason}"
            ),
            LoadError::Status(unnamed) => write!(f, "{unnamed}"),
            LoadError::Outcome(name) => write!(f, "the binding numbers no outcome {name}"),
        }
    }
}

impl std::error::Error for LoadError {}

type Of<A, R> = unsafe extern "C" fn(A) -> R;
type Of2<A, B, R> = unsafe extern "C" fn(A, B) -> R;

/// The runtime's functions a binding reads and makes the words of a value through that are not
/// the model's own: text, a `Decimal`, and what a reading came to.
pub struct Words {
    string_of_utf8: Of2<*const u8, i64, Word>,
    string_length: Of<Word, i64>,
    string_bytes: Of<Word, *const u8>,
    decimal_of_parts: Of2<Word, i64, Word>,
    decimal_unscaled: Of<Word, Word>,
    decimal_scale: Of<Word, i64>,
    date_of_iso: Of<Word, Word>,
    date_iso: Of<Word, Word>,
    time_of_iso: Of<Word, Word>,
    time_iso: Of<Word, Word>,
    datetime_of_iso: Of<Word, Word>,
    datetime_iso: Of<Word, Word>,
    instant_of_iso: Of<Word, Word>,
    instant_iso: Of<Word, Word>,
    decoded_outcome: Of<Word, i32>,
    decoded_value: Of<Word, Word>,
    decoded_malformed_at: Of<Word, i64>,
    decoded_issue_count: Of<Word, i64>,
    decoded_issue: Of2<Word, i64, Word>,
    issue_code: Of<Word, Word>,
    issue_path: Of<Word, Word>,
    issue_meta_count: Of<Word, i64>,
    issue_meta_key: Of2<Word, i64, Word>,
    issue_meta_value: Of2<Word, i64, Word>,
    value: i32,
    issues: i32,
}

impl Words {
    /// The runtime's functions in `library`, and a reading's outcomes numbered as `outcomes` does.
    ///
    /// # Safety
    ///
    /// `outcomes` is what the library's manifest numbers them, and the functions are called only
    /// while `library` is loaded.
    ///
    /// # Errors
    ///
    /// Where the library has none of one of them, or `outcomes` does not name one.
    pub unsafe fn load(
        library: &NativeLibrary,
        outcomes: &'static [(&'static str, i32)],
    ) -> Result<Self, LoadError> {
        let outcome = |name: &'static str| {
            outcomes
                .iter()
                .find(|(it, _)| *it == name)
                .map(|(_, number)| *number)
                .ok_or(LoadError::Outcome(name))
        };
        // SAFETY: each is the runtime's function of that name, which the manifest says is of this
        // type, and the caller keeps the library loaded.
        unsafe {
            Ok(Words {
                string_of_utf8: library.function("souther_string_of_utf8")?,
                string_length: library.function("souther_string_length")?,
                string_bytes: library.function("souther_string_bytes")?,
                decimal_of_parts: library.function("souther_decimal_of_parts")?,
                decimal_unscaled: library.function("souther_decimal_unscaled")?,
                decimal_scale: library.function("souther_decimal_scale")?,
                date_of_iso: library.function("souther_date_of_iso")?,
                date_iso: library.function("souther_date_iso")?,
                time_of_iso: library.function("souther_time_of_iso")?,
                time_iso: library.function("souther_time_iso")?,
                datetime_of_iso: library.function("souther_datetime_of_iso")?,
                datetime_iso: library.function("souther_datetime_iso")?,
                instant_of_iso: library.function("souther_instant_of_iso")?,
                instant_iso: library.function("souther_instant_iso")?,
                decoded_outcome: library.function("souther_decoded_outcome")?,
                decoded_value: library.function("souther_decoded_value")?,
                decoded_malformed_at: library.function("souther_decoded_malformed_at")?,
                decoded_issue_count: library.function("souther_decoded_issue_count")?,
                decoded_issue: library.function("souther_decoded_issue")?,
                issue_code: library.function("souther_issue_code")?,
                issue_path: library.function("souther_issue_path")?,
                issue_meta_count: library.function("souther_issue_meta_count")?,
                issue_meta_key: library.function("souther_issue_meta_key")?,
                issue_meta_value: library.function("souther_issue_meta_value")?,
                value: outcome("VALUE")?,
                issues: outcome("ISSUES")?,
            })
        }
    }

    /// `text` as the library holds text, made in `run`: for a call about to be made in it.
    ///
    /// The library puts text in NFC where it takes it, by the Unicode version the language names,
    /// and a Rust string is always UTF-8, which is all it asks.
    pub fn string<L: Loaded>(&self, _run: &mut Run<'_, L>, text: &str) -> Word {
        let length = i64::try_from(text.len()).expect("a string's length is a 64-bit count");
        // SAFETY: the bytes are `length` bytes that may be read, and UTF-8; the function is the
        // library's, loaded while `self` is.
        unsafe { (self.string_of_utf8)(text.as_ptr(), length) }
    }

    /// The text of a string the library answered.
    ///
    /// # Safety
    ///
    /// `at` is a string the library answered, in a run that is still open.
    pub unsafe fn text(&self, at: Word) -> String {
        // SAFETY: what the caller says, and the library answers a length and the bytes of it.
        unsafe {
            let length = usize::try_from((self.string_length)(at))
                .expect("a string's length is never below nought");
            if length == 0 {
                return String::new();
            }
            let bytes = std::slice::from_raw_parts((self.string_bytes)(at), length);
            String::from_utf8(bytes.to_vec()).expect("the library's text is UTF-8")
        }
    }

    /// `decimal` as the library holds one, made in `run`.
    pub fn decimal<L: Loaded>(&self, run: &mut Run<'_, L>, decimal: &Decimal) -> Word {
        let unscaled = self.string(run, decimal.unscaled());
        // SAFETY: the integer text is one `Decimal` has held to what the library takes, and the
        // scale is a 32-bit number.
        unsafe { (self.decimal_of_parts)(unscaled, i64::from(decimal.scale())) }
    }

    /// A `Decimal` the library answered.
    ///
    /// # Safety
    ///
    /// `at` is a `Decimal` the library answered, in a run that is still open.
    pub unsafe fn amount(&self, at: Word) -> Decimal {
        // SAFETY: what the caller says.
        unsafe {
            let unscaled = self.text((self.decimal_unscaled)(at));
            let scale = i32::try_from((self.decimal_scale)(at))
                .expect("a Decimal's scale is a 32-bit number");
            Decimal::new(&unscaled, scale).expect("the library's Decimal is one")
        }
    }

    /// `date` as the library holds one, made in `run` of the text that names it.
    pub fn date<L: Loaded>(&self, run: &mut Run<'_, L>, date: Date) -> Word {
        let iso = self.string(run, &date.iso());
        // SAFETY: the text is what `LocalDate` writes of a day a `Date` holds, which the library
        // reads.
        unsafe { (self.date_of_iso)(iso) }
    }

    /// A `Date` the library answered.
    ///
    /// # Safety
    ///
    /// `at` is a `Date` the library answered, in a run that is still open.
    pub unsafe fn date_of(&self, at: Word) -> Date {
        // SAFETY: what the caller says.
        Date::written(&unsafe { self.text((self.date_iso)(at)) })
    }

    /// `time` as the library holds one, made in `run`.
    pub fn time<L: Loaded>(&self, run: &mut Run<'_, L>, time: Time) -> Word {
        let iso = self.string(run, &time.iso());
        // SAFETY: as in `date`.
        unsafe { (self.time_of_iso)(iso) }
    }

    /// A `Time` the library answered.
    ///
    /// # Safety
    ///
    /// As [`Words::date_of`].
    pub unsafe fn time_of(&self, at: Word) -> Time {
        // SAFETY: what the caller says.
        Time::written(&unsafe { self.text((self.time_iso)(at)) })
    }

    /// `date_time` as the library holds one, made in `run`.
    pub fn date_time<L: Loaded>(&self, run: &mut Run<'_, L>, date_time: DateTime) -> Word {
        let iso = self.string(run, &date_time.iso());
        // SAFETY: as in `date`.
        unsafe { (self.datetime_of_iso)(iso) }
    }

    /// A `DateTime` the library answered.
    ///
    /// # Safety
    ///
    /// As [`Words::date_of`].
    pub unsafe fn date_time_of(&self, at: Word) -> DateTime {
        // SAFETY: what the caller says.
        DateTime::written(&unsafe { self.text((self.datetime_iso)(at)) })
    }

    /// `instant` as the library holds one, made in `run`.
    pub fn instant<L: Loaded>(&self, run: &mut Run<'_, L>, instant: Instant) -> Word {
        let iso = self.string(run, &instant.iso());
        // SAFETY: as in `date`.
        unsafe { (self.instant_of_iso)(iso) }
    }

    /// An `Instant` the library answered.
    ///
    /// # Safety
    ///
    /// As [`Words::date_of`].
    pub unsafe fn instant_of(&self, at: Word) -> Instant {
        // SAFETY: what the caller says.
        Instant::written(&unsafe { self.text((self.instant_iso)(at)) })
    }

    /// What a reading came to: the value `made` makes of what was read, the issues found in it,
    /// or an `invalid_format` issue where the text is not JSON.
    ///
    /// # Safety
    ///
    /// `decoded` is what a reading the library answered wrote, in a run that is still open.
    pub unsafe fn reading<T>(
        &self,
        decoded: Word,
        made: impl FnOnce(NonNull<u8>) -> T,
    ) -> Reading<T> {
        // SAFETY: what the caller says; the library answers each of these of a reading.
        unsafe {
            let outcome = (self.decoded_outcome)(decoded);
            if outcome == self.value {
                let value = NonNull::new((self.decoded_value)(decoded).cast_mut())
                    .expect("a reading that read a value answers it");
                return Reading::Value(made(value));
            }
            let mut issues = raoh::Issues::new();
            if outcome == self.issues {
                for at in 0..(self.decoded_issue_count)(decoded) {
                    issues.push(self.issue((self.decoded_issue)(decoded, at)));
                }
            } else {
                let at = (self.decoded_malformed_at)(decoded);
                issues.push(
                    raoh::Issue::new(raoh::codes::INVALID_FORMAT)
                        .with_message(format!("the text stops being JSON at byte {at}")),
                );
            }
            Reading::Issues(issues)
        }
    }

    /// One issue a reading found, as Raoh holds one. The codes are Raoh's already; the library
    /// gives no message, so the message is the code's until something resolves it.
    ///
    /// # Safety
    ///
    /// As [`Words::reading`].
    unsafe fn issue(&self, issue: Word) -> raoh::Issue {
        // SAFETY: what the caller says.
        unsafe {
            let code = self.text((self.issue_code)(issue));
            let path = self.text((self.issue_path)(issue));
            let mut made = raoh::Issue::new(code)
                .at(raoh::Pointer::parse(&path).expect("the library writes a JSON Pointer"));
            for entry in 0..(self.issue_meta_count)(issue) {
                made = made.with_meta(
                    self.text((self.issue_meta_key)(issue, entry)),
                    self.text((self.issue_meta_value)(issue, entry)),
                );
            }
            made
        }
    }
}

/// What reading a value out of its external form came to: the value, or the issues found in it.
#[derive(Debug)]
pub enum Reading<T> {
    Value(T),
    Issues(raoh::Issues),
}

impl<T> Reading<T> {
    /// The value, or the issues, as a `Result`.
    pub fn into_result(self) -> Result<T, raoh::Issues> {
        match self {
            Reading::Value(value) => Ok(value),
            Reading::Issues(issues) => Err(issues),
        }
    }
}

/// What constructing a value came to: the value, or an `invariant_violation` where what was handed
/// over does not hold what the type states.
#[derive(Debug)]
pub enum Construction<T> {
    Value(T),
    Rejected(raoh::Issue),
}

impl<T> Construction<T> {
    /// What a constructor's call came to: `made` where it answered, rejected where the library
    /// says the invariant was not held, and the failure otherwise.
    ///
    /// # Errors
    ///
    /// The call's failure, where it is not that the invariant was not held.
    pub fn of(called: Result<(), Failure>, made: impl FnOnce() -> T) -> Result<Self, Failure> {
        match called {
            Ok(()) => Ok(Construction::Value(made())),
            Err(Failure::Abort(abort)) if abort.name() == Some("INVARIANT_NOT_HELD") => Ok(
                Construction::Rejected(raoh::Issue::new("invariant_violation")),
            ),
            Err(failure) => Err(failure),
        }
    }

    /// The value, or the issue, as a `Result`.
    pub fn into_result(self) -> Result<T, raoh::Issue> {
        match self {
            Construction::Value(value) => Ok(value),
            Construction::Rejected(issue) => Err(issue),
        }
    }
}
