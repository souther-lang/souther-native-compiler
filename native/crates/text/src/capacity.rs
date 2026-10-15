//! How much text a `String` holds, as a budget a construction spends and not a check made on what
//! was built.
//!
//! The language says a string holds at most [`LONGEST_TEXT`] code points (spec
//! §what-a-string-holds, ADR-0096): a number of the language, not a carrier's, counted in what
//! `String.length` counts, so it is the same on every carrier. Whoever calls an operation that
//! builds text spends that budget as it builds, so that text no `String` holds is never made: an
//! operation ends as soon as what it would go on to write is more than is left.

/// A budget of Unicode code points of text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacity(i64);

impl Capacity {
    /// A carrier that holds this many code points.
    pub const fn of_code_points(code_points: i64) -> Capacity {
        Capacity(code_points)
    }

    /// How many code points.
    pub const fn code_points(self) -> i64 {
        self.0
    }

    /// Whether this many code points are held.
    pub(crate) const fn holds(self, code_points: i64) -> bool {
        code_points <= self.0
    }
}

/// The longest text, in code points, a Souther `String` holds (spec §what-a-string-holds): a
/// number of the language, not a carrier's, so it is the same on every carrier and does not
/// depend on which characters a text holds (ADR-0096).
pub const LONGEST_TEXT: i64 = (1 << 28) - 1;

/// How many Unicode code points the text is written in.
pub(crate) fn code_points(text: &str) -> i64 {
    notation199x::scalar_count(text) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bound is a number of the language and not this carrier's own, so every carrier states
    /// the same one: the JVM's `souther.runtime.Strings.LONGEST_TEXT` is `(1L << 28) - 1`
    /// (souther-lang/souther PR #2022, ADR-0096), and the specification's own
    /// `what-a-string-holds` paragraph states `268435455` — the two crates agree by stating the
    /// same arithmetic rather than by copying one crate's decimal literal into the other's.
    #[test]
    fn the_bound_is_the_languages_own_and_not_this_carriers() {
        assert_eq!(LONGEST_TEXT, (1i64 << 28) - 1);
        assert_eq!(LONGEST_TEXT, 268_435_455);
    }
}
