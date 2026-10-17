//! The rows of a behavior a host implements, and what one row came to, run against the host's own
//! implementation.
//!
//! Two questions and no third, as the JVM's `BoundExamples` asks them: which rows there are, and
//! what one of them came to. A binding answers the first with the rows it was generated with and the
//! second with one call into the library, and nothing here runs every row. An implementation answers
//! out of what a test arranges around it, the rows of a table, what a clock says, and that is the
//! test's, changed between rows where this cannot see it; whoever changes it owns the loop.
//!
//! Whether an answer keeps a row is the library's to say, by the language's equality: a `Decimal`
//! at another scale, a set in another order are what the row states. The answer's external form
//! comes back beside the verdict for a report, and is never compared here.

use crate::failure::Failure;
use crate::native::Word;
use std::fmt;

/// What a row names itself: the name it was written with, or which of its behavior's rows it is,
/// counted from one, where it was written without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowIdentity {
    /// The name it was written with.
    Named(&'static str),
    /// Which row it is, from one.
    Unnamed(usize),
}

impl RowIdentity {
    /// What a report calls the row: its name, or `row <n>`.
    pub fn shown(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for RowIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowIdentity::Named(name) => f.write_str(name),
            RowIdentity::Unnamed(ordinal) => write!(f, "row {ordinal}"),
        }
    }
}

/// What a host does with a row. A row that states an answer and one whose answer is owed are run;
/// the other two are listed and have nothing to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKind {
    /// The row states an answer, and the library says whether what the implementation answers
    /// keeps it.
    Asserted,
    /// The row states an answer the library has no way to compare one with, so there is nothing
    /// to run.
    NotComparable,
    /// The row's answer is owed (`<?>`): it is run, and states nothing an answer could keep.
    Owed,
    /// The compile could not read the row, for the reason it says, so there is nothing to run.
    Unreproducible(&'static str),
}

/// One row of a behavior a host implements, as a binding lists it.
///
/// Made by the binding and by nothing else, and evaluated only by the implementation of the
/// behavior it is a row of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Row {
    behavior: &'static str,
    at: usize,
    identity: RowIdentity,
    kind: RowKind,
}

impl Row {
    /// The row standing `at` among the rows of `behavior` (`module.name`), naming itself
    /// `identity`, which a host does with as `kind` says.
    #[doc(hidden)]
    pub const fn new(
        behavior: &'static str,
        at: usize,
        identity: RowIdentity,
        kind: RowKind,
    ) -> Self {
        Row {
            behavior,
            at,
            identity,
            kind,
        }
    }

    /// The behavior it is a row of, as the model names it: `module.name`.
    pub fn behavior(&self) -> &'static str {
        self.behavior
    }

    /// Where it stands among its behavior's rows, from nought, in the order they are written. What
    /// the library's function for it is spelt under, and not a key: a row written in another place
    /// is another row.
    pub fn at(&self) -> usize {
        self.at
    }

    /// What it names itself.
    pub fn identity(&self) -> RowIdentity {
        self.identity
    }

    /// What a report calls it.
    pub fn shown(&self) -> String {
        self.identity.shown()
    }

    /// What a host does with it.
    pub fn kind(&self) -> RowKind {
        self.kind
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.behavior, self.identity)
    }
}

/// Why a row was not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotRun {
    /// The library has no way to compare an answer with what the row states.
    NotComparable,
    /// The compile could not read the row, for the reason it says.
    Unreproducible(&'static str),
}

impl fmt::Display for NotRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotRun::NotComparable => f.write_str(
                "not run: the library has no way to compare an answer with what the row states",
            ),
            NotRun::Unreproducible(why) => {
                write!(f, "not run: the compile could not read it: {why}")
            }
        }
    }
}

/// What one row came to, run against a host's implementation.
///
/// Five ends and not a verdict with parts that may be missing: a row whose answer is owed has no
/// verdict to read, and a row that ended without an answer did not fail to keep anything.
#[derive(Debug)]
pub enum RowOutcome {
    /// The implementation's answer keeps the row. `answer` is its external form, for a report.
    Kept {
        /// The answer in the language's external form.
        answer: String,
    },
    /// The implementation's answer does not keep the row. `answer` is its external form, for a
    /// report: which value differed is the language's to say, and it is not said by comparing two
    /// texts.
    NotKept {
        /// The answer in the language's external form.
        answer: String,
    },
    /// The row's answer is owed, and this is what the implementation answered. Nothing is said of
    /// whether it keeps the row, since the row states nothing it could keep.
    Owed {
        /// The answer in the language's external form.
        answer: String,
    },
    /// The row ended without an answer, as a call of the behavior would have: the implementation's
    /// own failure, a clause the answer breaks, a requirement handed nothing.
    Ended(Failure),
    /// The row has nothing to run.
    NotRun(NotRun),
}

impl RowOutcome {
    /// The room a row's function writes whether the answer keeps the row into, before the call:
    /// neither of the two values it writes, so that a room left as it was is told from a verdict.
    #[doc(hidden)]
    pub const UNWRITTEN: u8 = 2;

    /// What a row stating an answer came to, where its function answered `called`, wrote `kept`
    /// and left `answer` in the room for the answer, which `read` reads.
    ///
    /// Nought or one is what the library writes, and an answer always: a function that answered
    /// and left either room as it was is no verdict and no answer, and comes to
    /// [`Failure::ProtocolViolation`], as a call answering what it should not does.
    #[doc(hidden)]
    pub fn asserted(
        called: Result<(), Failure>,
        kept: u8,
        answer: Word,
        read: impl FnOnce(Word) -> String,
    ) -> Self {
        match called {
            Err(failure) => RowOutcome::Ended(failure),
            Ok(()) if answer.is_null() || kept > 1 => RowOutcome::Ended(Failure::ProtocolViolation),
            Ok(()) if kept == 1 => RowOutcome::Kept {
                answer: read(answer),
            },
            Ok(()) => RowOutcome::NotKept {
                answer: read(answer),
            },
        }
    }

    /// What a row whose answer is owed came to, where its function answered `called` and left
    /// `answer` in the room for the answer, which `read` reads; a room left as it was comes to
    /// [`Failure::ProtocolViolation`], as for [`RowOutcome::asserted`].
    #[doc(hidden)]
    pub fn owed(
        called: Result<(), Failure>,
        answer: Word,
        read: impl FnOnce(Word) -> String,
    ) -> Self {
        match called {
            Err(failure) => RowOutcome::Ended(failure),
            Ok(()) if answer.is_null() => RowOutcome::Ended(Failure::ProtocolViolation),
            Ok(()) => RowOutcome::Owed {
                answer: read(answer),
            },
        }
    }

    /// What a row with nothing to run comes to, or `Ended` with the failure where nothing could
    /// be handed to the row's function.
    ///
    /// # Panics
    ///
    /// Where `row` is one that runs: the binding runs every such row, and one reaching here is the
    /// binding listing a row it has no function for.
    #[doc(hidden)]
    pub fn not_run(row: &Row) -> Self {
        match row.kind {
            RowKind::NotComparable => RowOutcome::NotRun(NotRun::NotComparable),
            RowKind::Unreproducible(why) => RowOutcome::NotRun(NotRun::Unreproducible(why)),
            RowKind::Asserted | RowKind::Owed => {
                panic!("{row} is listed as one that runs, and the binding has nothing running it")
            }
        }
    }

    /// Whether the implementation's answer keeps the row: false for every other end, an owed row
    /// among them, which keeps nothing and breaks nothing.
    pub fn kept(&self) -> bool {
        matches!(self, RowOutcome::Kept { .. })
    }

    /// The implementation's answer in external form, where it answered.
    pub fn answer(&self) -> Option<&str> {
        match self {
            RowOutcome::Kept { answer }
            | RowOutcome::NotKept { answer }
            | RowOutcome::Owed { answer } => Some(answer),
            RowOutcome::Ended(_) | RowOutcome::NotRun(_) => None,
        }
    }

    /// Panics, saying which row and what it came to, unless the answer keeps `row`: what a test
    /// asserts of a row that states an answer.
    ///
    /// Which rows a suite holds to this is the suite's. A row whose answer is owed is never kept,
    /// and one with nothing to run is never run, so a loop over every row asks [`Row::kind`] first.
    ///
    /// # Panics
    ///
    /// Where the outcome is anything but [`RowOutcome::Kept`].
    #[track_caller]
    pub fn assert_kept(&self, row: &Row) {
        assert!(self.kept(), "{row}: {self}");
    }
}

impl fmt::Display for RowOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowOutcome::Kept { answer } => write!(f, "kept, answering {answer}"),
            RowOutcome::NotKept { answer } => write!(f, "not kept, answering {answer}"),
            RowOutcome::Owed { answer } => write!(f, "owed, answering {answer}"),
            RowOutcome::Ended(failure) => write!(f, "ended without an answer: {failure}"),
            RowOutcome::NotRun(not_run) => write!(f, "{not_run}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A room for the answer a function wrote into; what it points at is never read here.
    const WRITTEN: Word = c"written".as_ptr().cast();

    const NAMED: Row = Row::new(
        "held.priced",
        0,
        RowIdentity::Named("a price"),
        RowKind::Asserted,
    );
    const UNNAMED: Row = Row::new("held.priced", 1, RowIdentity::Unnamed(2), RowKind::Owed);

    /// A row is shown as the manifest's `shown()` says it, after its behavior: its name, or which
    /// row it is, from one.
    #[test]
    fn a_row_is_shown_by_its_name_or_by_which_row_it_is() {
        assert_eq!(NAMED.to_string(), "held.priced a price");
        assert_eq!(UNNAMED.shown(), "row 2");
    }

    /// A row its function answered for comes to what the function wrote, and a call that ended
    /// comes to the failure, whatever room was written, and nothing of the answer is read then.
    #[test]
    fn a_row_comes_to_what_its_function_wrote_and_never_reads_an_answer_it_did_not() {
        let kept = RowOutcome::asserted(Ok(()), 1, WRITTEN, |_| "1.50".to_owned());
        assert!(kept.kept());
        assert_eq!(kept.answer(), Some("1.50"));
        let not = RowOutcome::asserted(Ok(()), 0, WRITTEN, |_| "2".to_owned());
        assert!(!not.kept());
        assert_eq!(not.to_string(), "not kept, answering 2");
        let ended = RowOutcome::asserted(Err(Failure::Unbound), 1, WRITTEN, |_| unreachable!());
        assert!(matches!(ended, RowOutcome::Ended(Failure::Unbound)));
        let owed = RowOutcome::owed(Ok(()), WRITTEN, |_| "5".to_owned());
        assert!(!owed.kept(), "an owed row has no verdict, and is not kept");
        assert_eq!(owed.answer(), Some("5"));
    }

    /// A function that answered and left a room as it was wrote no verdict and no answer, which
    /// is the library breaking the protocol and not a row that failed, and nothing is read.
    #[test]
    fn a_room_left_as_it_was_is_a_protocol_violation_and_not_a_verdict() {
        for (kept, answer) in [
            (RowOutcome::UNWRITTEN, WRITTEN),
            (1, std::ptr::null()),
            (0, std::ptr::null()),
        ] {
            let came = RowOutcome::asserted(Ok(()), kept, answer, |_| unreachable!());
            assert!(
                matches!(came, RowOutcome::Ended(Failure::ProtocolViolation)),
                "{came}"
            );
        }
        let owed = RowOutcome::owed(Ok(()), std::ptr::null(), |_| unreachable!());
        assert!(
            matches!(owed, RowOutcome::Ended(Failure::ProtocolViolation)),
            "{owed}"
        );
    }

    /// A row with nothing to run says why, and the binding handing one that runs here is refused.
    #[test]
    fn a_row_with_nothing_to_run_says_why() {
        let unread = Row::new(
            "held.priced",
            2,
            RowIdentity::Unnamed(3),
            RowKind::Unreproducible("a value it cannot build"),
        );
        assert_eq!(
            RowOutcome::not_run(&unread).to_string(),
            "not run: the compile could not read it: a value it cannot build"
        );
        let refused = std::panic::catch_unwind(|| RowOutcome::not_run(&NAMED));
        assert!(refused.is_err());
    }

    /// What a test asserts says which row and what it came to.
    #[test]
    fn asserting_a_row_kept_says_which_row_and_what_it_came_to() {
        let not = RowOutcome::asserted(Ok(()), 0, WRITTEN, |_| "{\"amount\":1.51}".to_owned());
        let said =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| not.assert_kept(&NAMED)))
                .unwrap_err();
        assert_eq!(
            said.downcast_ref::<String>().map(String::as_str),
            Some("held.priced a price: not kept, answering {\"amount\":1.51}")
        );
    }
}
