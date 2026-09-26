//! How much text a carrier holds, as a budget a construction spends and not a check made on what
//! was built.
//!
//! The language says a string holds a bounded amount of text and that the bound is the carrier's
//! (spec §what-a-string-holds). So the bound is not written here: text semantics is shared by every
//! target, and a number that comes from one target's array limit belongs to that target. Whoever
//! calls an operation that builds text says how much its carrier holds, in the units the carrier
//! keeps text in, and the operation spends that as it builds, so that text no carrier holds is
//! never made: an operation ends as soon as what it would go on to write is more than is left.

/// How many UTF-16 code units of text a carrier holds.
///
/// UTF-16 units and not bytes or code points, whatever this crate keeps text in itself: a bound
/// stated in the units a carrier counts is one the language can state the same for every target
/// that counts them, and a text longer in bytes than the bound and no longer in units has a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacity(i64);

impl Capacity {
    /// A carrier that holds this many units.
    pub const fn of_units(units: i64) -> Capacity {
        Capacity(units)
    }

    /// For text that is already held, put in NFC where it comes in: a door builds nothing a
    /// carrier has not been handed.
    pub(crate) const UNBOUNDED: Capacity = Capacity(i64::MAX);

    /// How many units.
    pub const fn units(self) -> i64 {
        self.0
    }

    /// Whether this many units are held.
    pub(crate) const fn holds(self, units: i64) -> bool {
        units <= self.0
    }
}

/// How many UTF-16 code units the text is written in.
pub(crate) fn units(text: &str) -> i64 {
    if text.is_ascii() {
        return text.len() as i64;
    }
    text.chars().map(|it| it.len_utf16() as i64).sum()
}
