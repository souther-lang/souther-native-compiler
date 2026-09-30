//! A library as a shared library a process loaded, and the runtime's own functions in it.

use crate::decimal::Decimal;
use crate::failure::{Failure, Statuses, UnnamedStatus};
use crate::run::{Loaded, Run, Runtime, ScopeCloseFn, ScopeOpenFn};
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
    /// Where the library has no `souther_scope_open` or `souther_scope_close`, or `statuses` does not name one
    /// a host has to tell apart.
    pub unsafe fn runtime(
        &self,
        statuses: &'static [(&'static str, u32)],
    ) -> Result<Runtime, LoadError> {
        let statuses = Statuses::new(statuses).map_err(LoadError::Status)?;
        // SAFETY: both are the runtime's, as the ABI states them, and the caller keeps this
        // library loaded while the runtime is used.
        unsafe {
            let open: ScopeOpenFn = self.function("souther_scope_open")?;
            let close: ScopeCloseFn = self.function("souther_scope_close")?;
            Ok(Runtime::new(open, close, statuses))
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
type Of3<A, B, C, R> = unsafe extern "C" fn(A, B, C) -> R;
type Of4<A, B, C, D, R> = unsafe extern "C" fn(A, B, C, D) -> R;
/// What a temporal with `N` numbers is read through: the value, then room for each number.
type Parts3 = unsafe extern "C" fn(Word, *mut i64, *mut i64, *mut i64);
type Parts6 =
    unsafe extern "C" fn(Word, *mut i64, *mut i64, *mut i64, *mut i64, *mut i64, *mut i64);
type Parts2 = unsafe extern "C" fn(Word, *mut i64, *mut i64);

/// The runtime's functions a binding reads and makes the words of a value through that are not
/// the model's own: text, a `Decimal`, and what a reading came to.
pub struct Words {
    string_of_utf8: Of3<*const u8, i64, *mut Word, i8>,
    string_length: Of<Word, i64>,
    string_bytes: Of<Word, *const u8>,
    decimal_of_parts: Of4<*const u8, i64, i64, *mut Word, i8>,
    decimal_unscaled: Of<Word, Word>,
    decimal_scale: Of<Word, i64>,
    date_of_parts: Of4<i64, i64, i64, *mut Word, i8>,
    date_parts: Parts3,
    time_of_parts: Of4<i64, i64, i64, *mut Word, i8>,
    time_parts: Parts3,
    datetime_of_parts: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, *mut Word) -> i8,
    datetime_parts: Parts6,
    instant_of_parts: Of3<i64, i64, *mut Word, i8>,
    instant_parts: Parts2,
    decoded_outcome: Of<Word, i32>,
    decoded_value: Of<Word, Word>,
    decoded_malformed_at: Of<Word, i64>,
    decoded_issue_count: Of<Word, i64>,
    decoded_issue: Of2<Word, i64, Word>,
    issue_code: Of<Word, Word>,
    issue_message_key: Of<Word, Word>,
    issue_path: Of<Word, Word>,
    issue_meta: Of<Word, Word>,
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
                date_of_parts: library.function("souther_date_of_parts")?,
                date_parts: library.function("souther_date_parts")?,
                time_of_parts: library.function("souther_time_of_parts")?,
                time_parts: library.function("souther_time_parts")?,
                datetime_of_parts: library.function("souther_datetime_of_parts")?,
                datetime_parts: library.function("souther_datetime_parts")?,
                instant_of_parts: library.function("souther_instant_of_parts")?,
                instant_parts: library.function("souther_instant_parts")?,
                decoded_outcome: library.function("souther_decoded_outcome")?,
                decoded_value: library.function("souther_decoded_value")?,
                decoded_malformed_at: library.function("souther_decoded_malformed_at")?,
                decoded_issue_count: library.function("souther_decoded_issue_count")?,
                decoded_issue: library.function("souther_decoded_issue")?,
                issue_code: library.function("souther_issue_code")?,
                issue_message_key: library.function("souther_issue_message_key")?,
                issue_path: library.function("souther_issue_path")?,
                issue_meta: library.function("souther_issue_meta")?,
                value: outcome("VALUE")?,
                issues: outcome("ISSUES")?,
            })
        }
    }

    /// `text` as the library holds text, made in `run`: for a call about to be made in it.
    ///
    /// The library puts text in NFC where it takes it, by the Unicode version the language names,
    /// and a Rust string is always UTF-8, which is all it asks.
    ///
    /// # Errors
    ///
    /// [`Failure::Abort`] of `REQUIRED_FORM_HAS_NO_PLACE` where `text`'s canonical value is longer
    /// than a `String` holds (spec §what-a-string-holds).
    pub fn string<L: Loaded>(&self, run: &mut Run<'_, L>, text: &str) -> Result<Word, Failure> {
        let length = i64::try_from(text.len()).expect("a string's length is a 64-bit count");
        let mut word = std::ptr::null();
        // SAFETY: the bytes are `length` bytes that may be read, and UTF-8; `word` is room for a
        // `Word`; the function is the library's, loaded while `self` is.
        let admitted = unsafe { (self.string_of_utf8)(text.as_ptr(), length, &mut word) };
        if admitted != 0 {
            Ok(word)
        } else {
            Err(run
                .library()
                .runtime()
                .statuses()
                .no_place()
                .unwrap_or(Failure::ProtocolViolation))
        }
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
    ///
    /// The unscaled digits are handed over as bytes, not a `String`: they are the integer's text
    /// and never the value's written form, so they are not measured against what a `String` holds
    /// (souther-native-compiler#109).
    ///
    /// # Errors
    ///
    /// [`Failure::ProtocolViolation`] where the library says the parts name no `Decimal`, which a
    /// [`Decimal`] never is: it holds itself to what the library takes where it is made.
    pub fn decimal<L: Loaded>(
        &self,
        _run: &mut Run<'_, L>,
        decimal: &Decimal,
    ) -> Result<Word, Failure> {
        let unscaled = decimal.unscaled();
        let length = i64::try_from(unscaled.len()).expect("an integer's length is a 64-bit count");
        let mut word = std::ptr::null();
        // SAFETY: the bytes are `length` bytes that may be read, `word` is room for a `Word`, and
        // the function is the library's, loaded while `self` is.
        let made = unsafe {
            (self.decimal_of_parts)(
                unscaled.as_ptr(),
                length,
                i64::from(decimal.scale()),
                &mut word,
            )
        };
        made_or_refused(made, word)
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

    /// `date` as the library holds one, made in `run` of its year, month and day.
    ///
    /// # Errors
    ///
    /// [`Failure::ProtocolViolation`] where the library says they name no `Date`, which a [`Date`]
    /// never is: it is held to the days a `Date` holds where it is made.
    pub fn date<L: Loaded>(&self, _run: &mut Run<'_, L>, date: Date) -> Result<Word, Failure> {
        let mut word = std::ptr::null();
        // SAFETY: `word` is room for a `Word`, and the function is the library's.
        let made = unsafe {
            (self.date_of_parts)(
                i64::from(date.year()),
                i64::from(date.month()),
                i64::from(date.day()),
                &mut word,
            )
        };
        made_or_refused(made, word)
    }

    /// A `Date` the library answered.
    ///
    /// # Safety
    ///
    /// `at` is a `Date` the library answered, in a run that is still open.
    pub unsafe fn date_of(&self, at: Word) -> Date {
        let [year, month, day] = unsafe { self.three(self.date_parts, at) };
        Date::new(narrowed(year), narrowed(month), narrowed(day))
            .expect("the library's Date is a day a Date holds")
    }

    /// `time` as the library holds one, made in `run`.
    ///
    /// # Errors
    ///
    /// As [`Words::date`].
    pub fn time<L: Loaded>(&self, _run: &mut Run<'_, L>, time: Time) -> Result<Word, Failure> {
        let mut word = std::ptr::null();
        // SAFETY: as in `date`.
        let made = unsafe {
            (self.time_of_parts)(
                i64::from(time.hour()),
                i64::from(time.minute()),
                i64::from(time.second()),
                &mut word,
            )
        };
        made_or_refused(made, word)
    }

    /// A `Time` the library answered.
    ///
    /// # Safety
    ///
    /// As [`Words::date_of`].
    pub unsafe fn time_of(&self, at: Word) -> Time {
        let [hour, minute, second] = unsafe { self.three(self.time_parts, at) };
        Time::new(narrowed(hour), narrowed(minute), narrowed(second))
            .expect("the library's Time is a time of day")
    }

    /// `date_time` as the library holds one, made in `run`.
    ///
    /// # Errors
    ///
    /// As [`Words::date`].
    pub fn date_time<L: Loaded>(
        &self,
        _run: &mut Run<'_, L>,
        date_time: DateTime,
    ) -> Result<Word, Failure> {
        let (date, time) = (date_time.date(), date_time.time());
        let mut word = std::ptr::null();
        // SAFETY: as in `date`.
        let made = unsafe {
            (self.datetime_of_parts)(
                i64::from(date.year()),
                i64::from(date.month()),
                i64::from(date.day()),
                i64::from(time.hour()),
                i64::from(time.minute()),
                i64::from(time.second()),
                &mut word,
            )
        };
        made_or_refused(made, word)
    }

    /// A `DateTime` the library answered.
    ///
    /// # Safety
    ///
    /// As [`Words::date_of`].
    pub unsafe fn date_time_of(&self, at: Word) -> DateTime {
        let mut parts = [0_i64; 6];
        let [year, month, day, hour, minute, second] = &mut parts;
        // SAFETY: what the caller says, and each is room for an `Int`.
        unsafe { (self.datetime_parts)(at, year, month, day, hour, minute, second) };
        let [year, month, day, hour, minute, second] = parts;
        DateTime::new(
            Date::new(narrowed(year), narrowed(month), narrowed(day))
                .expect("the library's DateTime is on a day a Date holds"),
            Time::new(narrowed(hour), narrowed(minute), narrowed(second))
                .expect("the library's DateTime is at a time of day"),
        )
    }

    /// `instant` as the library holds one, made in `run`.
    ///
    /// # Errors
    ///
    /// As [`Words::date`].
    pub fn instant<L: Loaded>(
        &self,
        _run: &mut Run<'_, L>,
        instant: Instant,
    ) -> Result<Word, Failure> {
        let mut word = std::ptr::null();
        // SAFETY: as in `date`.
        let made = unsafe {
            (self.instant_of_parts)(instant.second(), i64::from(instant.nano()), &mut word)
        };
        made_or_refused(made, word)
    }

    /// An `Instant` the library answered.
    ///
    /// # Safety
    ///
    /// As [`Words::date_of`].
    pub unsafe fn instant_of(&self, at: Word) -> Instant {
        let (mut second, mut nano) = (0, 0);
        // SAFETY: what the caller says, and each is room for an `Int`.
        unsafe { (self.instant_parts)(at, &mut second, &mut nano) };
        Instant::new(second, narrowed(nano)).expect("the library's Instant is a moment one holds")
    }

    /// The three numbers `parts` writes of `at`.
    ///
    /// # Safety
    ///
    /// As [`Words::date_of`], and `parts` is the library's function reading a value of `at`'s type.
    unsafe fn three(&self, parts: Parts3, at: Word) -> [i64; 3] {
        let [mut a, mut b, mut c] = [0; 3];
        // SAFETY: what the caller says, and each is room for an `Int`.
        unsafe { parts(at, &mut a, &mut b, &mut c) };
        [a, b, c]
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

    /// One issue a reading found, as Raoh holds one. The code, the message key and the metadata
    /// are Raoh's already, so this changes how they are held and not what they say; the library
    /// gives no message, and a resolver words one by the key.
    ///
    /// # Safety
    ///
    /// As [`Words::reading`].
    unsafe fn issue(&self, issue: Word) -> raoh::Issue {
        // SAFETY: what the caller says.
        unsafe {
            let code = self.text((self.issue_code)(issue));
            let key = self.text((self.issue_message_key)(issue));
            let path = self.text((self.issue_path)(issue));
            let meta = self.text((self.issue_meta)(issue));
            let serde_json::Value::Object(meta) =
                serde_json::from_str(&meta).expect("the library writes metadata as JSON")
            else {
                panic!("the library writes metadata as a JSON object, and wrote {meta}");
            };
            let mut made = raoh::Issue::new(code)
                .with_message_key(key)
                .at(raoh::Pointer::parse(&path).expect("the library writes a JSON Pointer"));
            for (name, said) in meta {
                made = made.with_meta(name, said);
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

/// The value a function making one wrote, where it answered that it made one.
///
/// A value this crate hands over is one of its own types, each held where it is made to what the
/// library takes, so a refusal is the library and this binding disagreeing about what a value is.
fn made_or_refused(made: i8, word: Word) -> Result<Word, Failure> {
    if made != 0 {
        Ok(word)
    } else {
        Err(Failure::ProtocolViolation)
    }
}

/// A number the library answered for a part of a value, as the narrower type this crate holds it
/// in: the library answers only numbers a value of the type has.
fn narrowed<T: TryFrom<i64>>(number: i64) -> T {
    T::try_from(number)
        .unwrap_or_else(|_| panic!("the library answered {number}, which no part of the value is"))
}
