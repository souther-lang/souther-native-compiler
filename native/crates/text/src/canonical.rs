//! The one form a string is kept in: Unicode 18.0.0's Normalization Form C (spec
//! §string-canonical).
//!
//! Text arriving from outside is put in it where it arrives, and an operation that builds a string
//! out of others puts what it built in it again, since NFC is not closed under joining two strings
//! or under mapping case: a base letter and a combining mark written apart compose into one code
//! point. What NFC makes of text is 199x-notation's, the one implementation every Souther and Raoh
//! runtime reads, so the tables and the algorithm are its and not a copy kept here.

use crate::Text;
use crate::capacity::{Capacity, code_points};
use alloc::string::String;
use notation199x::Form;

/// The text in NFC.
pub(crate) fn nfc(text: &str) -> String {
    notation199x::normalize(Form::Nfc, text)
}

/// Text arriving from outside in NFC, within `capacity`: text that comes in from outside is a
/// `String` only where its canonical value has a place (spec §what-a-string-holds), so admission
/// measures what NFC makes of the text and not the text as it arrived.
pub(crate) fn nfc_of_input(text: &str, capacity: Capacity) -> Option<String> {
    notation199x::normalize_within(Form::Nfc, text, within(capacity))
}

/// The most code points a budget holds, as 199x-notation's bounded rules take it.
fn within(capacity: Capacity) -> usize {
    usize::try_from(capacity.code_points()).unwrap_or(0)
}

/// Text in NFC made by joining runs of text each in NFC already, within what a carrier holds.
///
/// NFC is not closed under joining, but what joining two runs can change is only where they meet,
/// so each run added puts that much in NFC again and copies the rest
/// (`notation199x::append_normalized`), and joining many runs costs what copying them does rather
/// than normalizing all that came before once more for each.
///
/// The one way text is built here, so the one place a carrier's bound is held, and it is held as a
/// budget spent before each run is written: the runs as they were handed over are what the language
/// defines an operation as canonicalizing (spec §what-a-string-holds), so they are what must be
/// held. What they come to in NFC is never longer, since runs in NFC only compose where they meet,
/// so holding the one holds the other.
pub(crate) struct Joined {
    text: String,
    /// What has been handed over, in code points.
    handed: i64,
    capacity: Capacity,
}

impl Joined {
    pub(crate) fn new(capacity: Capacity) -> Joined {
        Joined {
            text: String::new(),
            handed: 0,
            capacity,
        }
    }

    /// `next` joined on, or nothing where what has been handed over would then be more than is
    /// held. Nothing is written for a run that does not fit.
    pub(crate) fn push(&mut self, next: Text) -> Option<()> {
        let next = next.as_str();
        let handed = self.handed.saturating_add(code_points(next));
        if !self.capacity.holds(handed) {
            return None;
        }
        self.handed = handed;
        notation199x::append_normalized(Form::Nfc, &mut self.text, next);
        Some(())
    }

    pub(crate) fn finished(self) -> String {
        debug_assert!(
            code_points(&self.text) <= self.handed,
            "runs in NFC only compose where they meet, so what they come to is not longer than what was handed over"
        );
        self.text
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::capacity::LONGEST_TEXT;
    use std::string::String;
    use std::vec::Vec;

    fn in_nfc(text: &str) -> String {
        nfc(text)
    }

    /// A base letter and the mark over it compose, and marks of two classes are put in order
    /// before they do.
    #[test]
    fn what_composes_is_composed() {
        assert_eq!(in_nfc("e\u{301}"), "\u{e9}");
        assert_eq!(in_nfc("\u{e9}"), "\u{e9}");
        assert_eq!(in_nfc("か\u{3099}"), "が");
        assert_eq!(in_nfc("a\u{323}\u{302}"), "\u{1ead}");
        assert_eq!(in_nfc("a\u{302}\u{323}"), "\u{1ead}");
        assert_eq!(in_nfc("plain"), "plain");
    }

    /// A composition exclusion stays decomposed, and a singleton becomes what it stands for.
    #[test]
    fn what_is_excluded_is_not_composed() {
        assert_eq!(in_nfc("\u{958}"), "\u{915}\u{93c}");
        assert_eq!(in_nfc("\u{212b}"), "\u{c5}");
        assert_eq!(in_nfc("\u{2126}"), "\u{3a9}");
    }

    /// Hangul composes by formula, both a leading and a vowel jamo and a syllable and a trailing one.
    #[test]
    fn hangul_composes_by_its_formula() {
        assert_eq!(in_nfc("\u{1100}\u{1161}"), "\u{ac00}");
        assert_eq!(in_nfc("\u{1100}\u{1161}\u{11a8}"), "\u{ac01}");
        assert_eq!(in_nfc("\u{ac00}\u{11a8}"), "\u{ac01}");
        assert_eq!(in_nfc("\u{ac01}"), "\u{ac01}");
    }

    /// A mark of the same class as the one before it is blocked from the starter.
    #[test]
    fn a_mark_behind_one_of_its_own_class_is_blocked() {
        assert_eq!(in_nfc("a\u{301}\u{301}"), "\u{e1}\u{301}");
    }

    /// Joining runs in NFC one after another answers what NFC makes of all of them joined, over
    /// runs made of what composes, reorders and is excluded: letters and marks of several classes,
    /// Hangul jamo and syllables, and starters that are the second of a pair.
    #[test]
    fn joining_at_the_seam_answers_what_nfc_of_the_whole_does() {
        let pool: [u32; 28] = [
            0x61, 0x65, 0x41, 0x3c9, 0x1100, 0x1161, 0x11a8, 0xac00, 0xac01, 0x301, 0x302, 0x323,
            0x308, 0x345, 0x304b, 0x3099, 0x915, 0x93c, 0xb47, 0xb3e, 0xf71, 0xf72, 0x212b, 0x344,
            0x1611e, 0x1611f, 0x16121, 0x16123,
        ];
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = |bound: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % bound as u64) as usize
        };
        for _ in 0..3_000 {
            let runs: Vec<String> = (0..1 + next(3))
                .map(|_| {
                    let written: String = (0..next(5))
                        .map(|_| char::from_u32(pool[next(pool.len())]).unwrap())
                        .collect();
                    nfc(&written)
                })
                .collect();
            let mut joined = Joined::new(Capacity::of_code_points(LONGEST_TEXT));
            let mut whole = String::new();
            for run in &runs {
                joined.push(Text::held(run)).unwrap();
                whole.push_str(run);
            }
            assert_eq!(joined.finished(), nfc(&whole), "{runs:?}");
        }
    }

    /// A run that begins with a composite whose first member composes with what is before it is
    /// within reach of the seam: `U+16123` is `U+1611E U+1611F`, and `U+1611E` composes with the
    /// `U+1611E` before it, so the two join to `U+16126`. Asking the code point alone, as this
    /// crate once did, answered `U+1611E U+16123`, which is not NFC.
    #[test]
    fn a_composite_that_begins_with_what_composes_backwards_is_joined_at_the_seam() {
        let mut joined = Joined::new(Capacity::of_code_points(LONGEST_TEXT));
        joined.push(Text::held("\u{1611e}")).unwrap();
        joined.push(Text::held("\u{16123}")).unwrap();
        assert_eq!(joined.finished(), "\u{16126}");
    }

    /// NFC of NFC is itself.
    #[test]
    fn nfc_is_idempotent() {
        for text in [
            "e\u{301}",
            "\u{958}",
            "\u{1100}\u{1161}\u{11a8}",
            "日本語",
            "a\u{323}\u{302}",
        ] {
            let once = in_nfc(text);
            assert_eq!(in_nfc(&once), once, "{text:?}");
        }
    }
}
