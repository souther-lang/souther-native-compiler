//! The one form a string is kept in: Unicode 18.0.0's Normalization Form C (spec
//! §string-canonical).
//!
//! Text arriving from outside is put in it where it arrives, and an operation that builds a string
//! out of others puts what it built in it again, since NFC is not closed under joining two strings
//! or under mapping case: a base letter and a combining mark written apart compose into one code
//! point. The algorithm is UAX #15's three steps — decompose canonically, order the combining marks
//! by class, compose what nothing blocks — over the tables [`crate::tables`] was generated with, and
//! Hangul's syllables by the formula Unicode states for them rather than by a table.

use crate::tables::{COMBINING_CLASS, COMPOSITION, DECOMPOSITION};
use crate::{encoded, scalar_values};
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
/// answered as it is without being decoded.
pub fn nfc(text: &[u8]) -> Vec<u8> {
    if text.is_ascii() {
        return text.to_vec();
    }
    normalized(scalar_values(text).collect())
}

/// These code points in NFC, as UTF-8.
pub(crate) fn normalized(points: Vec<u32>) -> Vec<u8> {
    let mut decomposed = Vec::with_capacity(points.len());
    for point in points {
        decompose(point, &mut decomposed);
    }
    put_in_canonical_order(&mut decomposed);
    encoded(&composed(decomposed))
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
        String::from_utf8(nfc(text.as_bytes())).expect("NFC of text is text")
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
