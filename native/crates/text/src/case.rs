//! `lowercase` and `uppercase`: Unicode 18.0.0's default case conversion, untailored (spec
//! §string-case).
//!
//! The full mapping, so one code point may become several (`ß` uppercases to `SS`), and no locale:
//! a Turkish `i` uppercases to `I`, never to `İ`. The one condition read is `Final_Sigma`, which is
//! about the text around a capital sigma and not about who is reading it. What either answers is
//! put in NFC again, since mapping case can leave text that is not.

use crate::canonical::normalized;
use crate::scalar_values;
use crate::tables::{CASE_IGNORABLE, CASED, FINAL_SIGMA, LOWERCASE, UPPERCASE};
use alloc::vec::Vec;

/// The text lowercased (`String.lowercase`).
pub fn lowercase(text: &[u8]) -> Vec<u8> {
    let points: Vec<u32> = scalar_values(text).collect();
    let mut out = Vec::with_capacity(points.len());
    for (at, point) in points.iter().enumerate() {
        let mapped = mapping(FINAL_SIGMA, *point)
            .filter(|_| final_sigma_holds(&points, at))
            .or_else(|| mapping(LOWERCASE, *point));
        match mapped {
            Some(mapped) => out.extend_from_slice(mapped),
            None => out.push(*point),
        }
    }
    normalized(out)
}

/// The text uppercased (`String.uppercase`).
pub fn uppercase(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for point in scalar_values(text) {
        match mapping(UPPERCASE, point) {
            Some(mapped) => out.extend_from_slice(mapped),
            None => out.push(point),
        }
    }
    normalized(out)
}

/// What a table maps a code point to, where it maps it at all: a code point it does not name maps
/// to itself.
fn mapping(table: &'static [(u32, &'static [u32])], point: u32) -> Option<&'static [u32]> {
    table
        .binary_search_by_key(&point, |(it, _)| *it)
        .ok()
        .map(|at| table[at].1)
}

fn within(runs: &[(u32, u32)], point: u32) -> bool {
    match runs.binary_search_by_key(&point, |(from, _)| *from) {
        Ok(_) => true,
        Err(0) => false,
        Err(after) => point <= runs[after - 1].1,
    }
}

/// Whether the code point at `at` stands where `Final_Sigma` holds: a cased code point before it
/// and none after it, each looked for past the case-ignorable code points between.
fn final_sigma_holds(points: &[u32], at: usize) -> bool {
    let cased = |point: &&u32| !within(CASE_IGNORABLE, **point);
    let before = points[..at].iter().rev().find(cased);
    let after = points[at + 1..].iter().find(cased);
    before.is_some_and(|it| within(CASED, *it)) && !after.is_some_and(|it| within(CASED, *it))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::string::String;

    fn lower(text: &str) -> String {
        String::from_utf8(lowercase(text.as_bytes())).unwrap()
    }

    fn upper(text: &str) -> String {
        String::from_utf8(uppercase(text.as_bytes())).unwrap()
    }

    /// The full mapping widens where Unicode says it does.
    #[test]
    fn a_code_point_may_map_to_several() {
        assert_eq!(upper("straße"), "STRASSE");
        assert_eq!(lower("STRASSE"), "strasse");
        assert_eq!(upper("ﬁ"), "FI");
        assert_eq!(lower("\u{130}"), "i\u{307}");
    }

    /// No locale narrows it: a Turkish dotless or dotted i is mapped as everyone's.
    #[test]
    fn no_locale_is_read() {
        assert_eq!(upper("i"), "I");
        assert_eq!(lower("I"), "i");
        assert_eq!(upper("ı"), "I");
    }

    /// A capital sigma is final at the end of a cased run and not inside one.
    #[test]
    fn a_final_sigma_is_read_off_the_text_around_it() {
        assert_eq!(lower("ΟΣ"), "ος");
        assert_eq!(lower("ΟΣΑ"), "οσα");
        assert_eq!(lower("Σ"), "σ");
        assert_eq!(lower("ΟΣ."), "ος.");
        assert_eq!(lower("ΟΣ'Α"), "οσ'α");
    }

    /// What neither mapping names is left as it is, and what comes out is in NFC.
    #[test]
    fn what_is_not_mapped_stays_and_what_comes_out_is_canonical() {
        assert_eq!(upper("日本 123"), "日本 123");
        assert_eq!(lower("日本 123"), "日本 123");
        assert_eq!(upper("\u{1f0}"), "J\u{30c}");
    }
}
