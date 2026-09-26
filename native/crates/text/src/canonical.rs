//! The one form a string is kept in: Unicode 18.0.0's Normalization Form C (spec
//! §string-canonical).
//!
//! Text arriving from outside is put in it where it arrives, and an operation that builds a string
//! out of others puts what it built in it again, since NFC is not closed under joining two strings
//! or under mapping case: a base letter and a combining mark written apart compose into one code
//! point. The algorithm is UAX #15's three steps — decompose canonically, order the combining marks
//! by class, compose what nothing blocks — over the tables [`crate::tables`] was generated with, and
//! Hangul's syllables by the formula Unicode states for them rather than by a table.

use crate::Text;
use crate::tables::{COMBINING_CLASS, COMPOSITION, DECOMPOSITION, SECOND_OF_A_PAIR};
use alloc::string::String;
use alloc::vec::Vec;

const S_BASE: u32 = 0xac00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11a7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

/// The text in NFC.
///
/// Text of ASCII alone is in NFC already, which is most of what a program handles, so it is
/// answered as it is without being decomposed.
pub(crate) fn nfc(text: &str) -> String {
    if text.is_ascii() {
        return String::from(text);
    }
    normalized(text.chars())
}

/// These characters in NFC.
pub(crate) fn normalized(characters: impl Iterator<Item = char>) -> String {
    let mut decomposed = Vec::new();
    for character in characters {
        decompose(u32::from(character), &mut decomposed);
    }
    put_in_canonical_order(&mut decomposed);
    composed(decomposed)
        .into_iter()
        .map(|point| char::from_u32(point).expect("NFC of scalar values is scalar values"))
        .collect()
}

/// Text in NFC made by joining runs of text each in NFC already.
///
/// NFC is not closed under joining, but what joining two runs can change is only where they meet:
/// the code points from the last stable starter of the first run to the first one of the second
/// that nothing before it can compose with. So each run added puts that much in NFC again and
/// copies the rest, and joining many runs costs what copying them does rather than normalizing all
/// that came before once more for each.
///
/// A stable starter is one nothing written after it is reordered in front of it. The first run's text before its last one is
/// already what NFC makes of it and nothing after can reach it. The second run's text from its
/// first one that is also no second of a pair is out of reach of anything before it.
pub(crate) struct Joined(String);

impl Joined {
    pub(crate) fn new() -> Joined {
        Joined(String::new())
    }

    /// `next` joined on.
    pub(crate) fn push(&mut self, next: Text) {
        let next = next.as_str();
        let reach = out_of_reach(next);
        if reach == 0 {
            self.0.push_str(next);
            return;
        }
        let from = last_stable_starter(&self.0);
        let mut seam = self.0.split_off(from);
        seam.push_str(&next[..reach]);
        self.0.push_str(&nfc(&seam));
        self.0.push_str(&next[reach..]);
    }

    pub(crate) fn finished(self) -> String {
        self.0
    }
}

/// Whether nothing written after this code point is reordered in front of it, in text in NFC.
///
/// A starter, since marks are reordered among themselves and never past one. A starter whose
/// decomposition begins with a mark would not do, but every such code point is excluded from
/// composition and so never stands in text in NFC.
fn stable_starter(point: u32) -> bool {
    combining_class(point) == 0
}

/// Whether a starter written before this code point may compose with it: the second of a pair
/// the table composes, or a Hangul vowel or trailing consonant.
fn second_of_a_pair(point: u32) -> bool {
    (V_BASE..V_BASE + V_COUNT).contains(&point)
        || (T_BASE + 1..T_BASE + T_COUNT).contains(&point)
        || SECOND_OF_A_PAIR.binary_search(&point).is_ok()
}

/// Where the text stops being within reach of what is joined before it: the first code point
/// that is a stable starter and no second of a pair, or the end.
fn out_of_reach(text: &str) -> usize {
    text.char_indices()
        .find(|(_, character)| {
            let point = u32::from(*character);
            stable_starter(point) && !second_of_a_pair(point)
        })
        .map_or(text.len(), |(at, _)| at)
}

/// Where the text's last stable starter begins, or nought where it has none.
fn last_stable_starter(text: &str) -> usize {
    text.char_indices()
        .rev()
        .find(|(_, character)| stable_starter(u32::from(*character)))
        .map_or(0, |(at, _)| at)
}

/// The canonical combining class of a code point: nought for a starter, and for every code point
/// the table does not name.
fn combining_class(point: u32) -> u8 {
    COMBINING_CLASS
        .binary_search_by_key(&point, |(it, _)| *it)
        .map_or(0, |at| COMBINING_CLASS[at].1)
}

/// The full canonical decomposition of a code point, added to `into`.
fn decompose(point: u32, into: &mut Vec<u32>) {
    if (S_BASE..S_BASE + S_COUNT).contains(&point) {
        let index = point - S_BASE;
        into.push(L_BASE + index / N_COUNT);
        into.push(V_BASE + (index % N_COUNT) / T_COUNT);
        let trailing = index % T_COUNT;
        if trailing != 0 {
            into.push(T_BASE + trailing);
        }
        return;
    }
    match DECOMPOSITION.binary_search_by_key(&point, |(it, _)| *it) {
        Ok(at) => {
            for part in DECOMPOSITION[at].1 {
                decompose(*part, into);
            }
        }
        Err(_) => into.push(point),
    }
}

/// Each run of combining marks sorted by class, a mark keeping its place among marks of its own
/// class: the order of two marks of different classes says nothing, and of one class it says which
/// is written over which.
fn put_in_canonical_order(points: &mut [u32]) {
    let mut at = 0;
    while at < points.len() {
        if combining_class(points[at]) == 0 {
            at += 1;
            continue;
        }
        let from = at;
        while at < points.len() && combining_class(points[at]) != 0 {
            at += 1;
        }
        // Insertion, which keeps marks of one class in the order they came, and a run of marks is
        // a handful long.
        let run = &mut points[from..at];
        for next in 1..run.len() {
            let mut back = next;
            while back > 0 && combining_class(run[back - 1]) > combining_class(run[back]) {
                run.swap(back - 1, back);
                back -= 1;
            }
        }
    }
}

/// The primary composite two code points compose into, where they compose.
fn composite(first: u32, second: u32) -> Option<u32> {
    if (L_BASE..L_BASE + L_COUNT).contains(&first) && (V_BASE..V_BASE + V_COUNT).contains(&second) {
        let l = first - L_BASE;
        let v = second - V_BASE;
        return Some(S_BASE + (l * V_COUNT + v) * T_COUNT);
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&first)
        && (first - S_BASE).is_multiple_of(T_COUNT)
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&second)
    {
        return Some(first + (second - T_BASE));
    }
    COMPOSITION
        .binary_search_by_key(&(first, second), |(pair, _)| *pair)
        .ok()
        .map(|at| COMPOSITION[at].1)
}

/// Decomposed text with every pair that composes and is not blocked composed.
///
/// A code point is blocked from the starter before it where something stands between them whose
/// class is nought or no lower than its own. What stands between is marks of a class above nought,
/// in order, so the last of them is the one that decides.
fn composed(points: Vec<u32>) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::with_capacity(points.len());
    let mut starter: Option<usize> = None;
    let mut last_class: Option<u8> = None;
    for point in points {
        let class = combining_class(point);
        if let Some(at) = starter {
            let blocked = last_class.is_some_and(|last| last >= class);
            if !blocked && let Some(joined) = composite(out[at], point) {
                out[at] = joined;
                continue;
            }
        }
        if class == 0 {
            starter = Some(out.len());
            last_class = None;
        } else {
            last_class = Some(class);
        }
        out.push(point);
    }
    out
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::tables::UNICODE_VERSION;
    use std::string::String;
    use std::vec::Vec;

    fn in_nfc(text: &str) -> String {
        nfc(text)
    }

    #[test]
    fn the_tables_are_the_version_the_language_names() {
        assert_eq!(UNICODE_VERSION, (18, 0, 0));
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
        let pool: [u32; 24] = [
            0x61, 0x65, 0x41, 0x3c9, 0x1100, 0x1161, 0x11a8, 0xac00, 0xac01, 0x301, 0x302, 0x323,
            0x308, 0x345, 0x304b, 0x3099, 0x915, 0x93c, 0xb47, 0xb3e, 0xf71, 0xf72, 0x212b, 0x344,
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
            let mut joined = Joined::new();
            let mut whole = String::new();
            for run in &runs {
                joined.push(Text::held(run));
                whole.push_str(run);
            }
            assert_eq!(joined.finished(), nfc(&whole), "{runs:?}");
        }
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

    /// Unicode's own conformance file, run where it has been downloaded: every line's NFC column is
    /// what NFC makes of each of the first three, and the fourth and fifth are NFC's own NFKD
    /// column's NFC.
    #[test]
    #[ignore = "reads NormalizationTest.txt from the directory SOUTHER_UCD names"]
    fn unicodes_conformance_file_holds() {
        let directory = std::env::var("SOUTHER_UCD").expect("SOUTHER_UCD names a directory");
        let file =
            std::fs::read_to_string(std::path::Path::new(&directory).join("NormalizationTest.txt"))
                .expect("NormalizationTest.txt is there");
        assert!(file.starts_with("# NormalizationTest-18.0.0.txt"));
        let column = |written: &str| -> String {
            written
                .split_whitespace()
                .map(|hex| char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap())
                .collect()
        };
        let mut read = 0;
        for line in file.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('@') {
                continue;
            }
            let columns: Vec<String> = line.split(';').take(5).map(column).collect();
            for source in &columns[..3] {
                assert_eq!(in_nfc(source), columns[1], "{line}");
            }
            for source in &columns[3..5] {
                assert_eq!(in_nfc(source), columns[3], "{line}");
            }
            read += 1;
        }
        assert!(read > 10_000, "{read}");
    }
}
