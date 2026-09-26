//! What the language says text means, over the bytes it is kept in and nothing else.
//!
//! A Souther string is a sequence of Unicode scalar values, kept as UTF-8 in NFC, and what the
//! language asks of it — how long it is, which of two comes first, what `trim` or `lowercase` or
//! `matches` answers — is a question about the text and not about where it stands. So everything
//! here takes slices and answers numbers, orderings, pieces of what it was handed, or the bytes of
//! text it built: no arena, no address of either runtime's width, no layout. That is what lets
//! this move, as it is, into what the native and the wasm runtimes both read (#17), the way
//! `souther-json-syntax` is shaped to. What the runtime adds is where the answer is kept.
//!
//! The text is read one code point at a time by [`decoded`], and every operation here that reads
//! code points is written over it and not over a reading of its own. Two readings agree on
//! well-formed text and part on the rest — one counts the bytes that start a code point, another
//! steps by what a first byte says — and the difference is found by nobody, since a Souther string
//! is never ill-formed. One reading means that where they would part, they cannot.
//!
//! Where the language names a Unicode version — for NFC, for case — it is the one [`tables`] was
//! generated from, and not whichever a dependency was last released at.

#![no_std]

extern crate alloc;

mod canonical;
mod case;
mod integer;
mod operations;
pub mod pattern;
mod tables;

pub use canonical::nfc;
pub use case::{lowercase, uppercase};
pub use integer::{integer, written};
pub use operations::{
    MOST, append, characters, code_points_of, contains, ends_with, is_whitespace, join, lines,
    pad_left, pad_right, repeat, replace, reverse, slice, split, starts_with, trim, words,
};
pub use tables::UNICODE_VERSION;

use alloc::vec::Vec;
use core::cmp::Ordering;

/// The code point that starts at `at`, and how many bytes it takes, or nothing past the end.
///
/// The width is what the first byte says. A byte the text does not have reads as a continuation
/// byte of nought, so a sequence cut short at the end answers a code point and never reads past the
/// slice. What it answers for bytes that are not UTF-8 means nothing; that it answers without
/// reading past them is the one thing it promises.
fn decoded(text: &[u8], at: usize) -> Option<(u32, usize)> {
    let first = u32::from(*text.get(at)?);
    let trailing = |offset: usize| u32::from(text.get(at + offset).copied().unwrap_or(0)) & 0x3f;
    Some(if first < 0x80 {
        (first, 1)
    } else if first < 0xe0 {
        (((first & 0x1f) << 6) | trailing(1), 2)
    } else if first < 0xf0 {
        (((first & 0x0f) << 12) | (trailing(1) << 6) | trailing(2), 3)
    } else {
        (
            ((first & 0x07) << 18) | (trailing(1) << 12) | (trailing(2) << 6) | trailing(3),
            4,
        )
    })
}

/// The scalar values the text writes, in order.
///
/// A code point [`decoded`] reads out of bytes that are not UTF-8 may be none, and is answered as
/// U+FFFD: what such bytes mean is nothing, and this answers without reading past them.
pub(crate) fn scalar_values(text: &[u8]) -> impl Iterator<Item = u32> + '_ {
    let mut at = 0;
    core::iter::from_fn(move || {
        let (point, width) = decoded(text, at)?;
        at += width;
        Some(char::from_u32(point).map_or(0xfffd, u32::from))
    })
}

/// These scalar values as UTF-8.
pub(crate) fn encoded(points: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(points.len());
    let mut written = [0u8; 4];
    for point in points {
        let character = char::from_u32(*point).unwrap_or(char::REPLACEMENT_CHARACTER);
        out.extend_from_slice(character.encode_utf8(&mut written).as_bytes());
    }
    out
}

/// How long the text is as the language counts it, in code points (`String.length`).
///
/// Not the bytes it is kept in, and not the characters a reader sees: `𠮷` is one code point and
/// four bytes, and `🇯🇵` is two code points and one flag.
pub fn code_points(text: &[u8]) -> usize {
    let mut at = 0;
    let mut counted = 0;
    while let Some((_, width)) = decoded(text, at) {
        at += width;
        counted += 1;
    }
    counted
}

/// Two runs of text, in the order the language gives text.
///
/// A string is a sequence of Unicode scalar values and is ordered lexicographically over them: the
/// first value where the two differ decides, and a run that begins the other comes before it (spec
/// §equality). UTF-8 writes scalar values in an order its bytes keep — a greater value is written
/// with a greater first byte, or the same first byte and a greater byte after it — so the order of
/// the bytes is that order, and nothing is decoded to answer it. A carrier holding UTF-16 would
/// have to correct its units where a surrogate meets a unit from E000 up; this one has nothing to
/// correct.
pub fn compare(left: &[u8], right: &[u8]) -> Ordering {
    left.cmp(right)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    /// A length is counted in code points: not bytes, and not what a reader sees as one character.
    #[test]
    fn a_length_is_counted_in_code_points() {
        for (text, counted) in [
            ("", 0),
            ("cart", 4),
            ("é", 1),
            ("日本語", 3),
            ("𠮷", 1),
            ("🇯🇵", 2),
            ("a𠮷b", 3),
        ] {
            assert_eq!(code_points(text.as_bytes()), counted, "{text}");
        }
    }

    /// Text is ordered by scalar value, which puts a character past the basic plane after one from
    /// E000 up: the order a JVM string's UTF-16 units are in is the other way round there.
    #[test]
    fn text_is_ordered_by_scalar_value() {
        assert_eq!(compare("￥".as_bytes(), "𠮷".as_bytes()), Ordering::Less);
        assert_eq!(compare("a".as_bytes(), "ab".as_bytes()), Ordering::Less);
        assert_eq!(compare("b".as_bytes(), "ab".as_bytes()), Ordering::Greater);
        assert_eq!(
            compare("日本".as_bytes(), "日本".as_bytes()),
            Ordering::Equal
        );
        assert_eq!(compare("".as_bytes(), "a".as_bytes()), Ordering::Less);
    }

    /// The order of the bytes is the order of the scalar values they write, for every pair of
    /// characters one byte-width boundary apart and on either side of the surrogates.
    #[test]
    fn the_bytes_are_in_the_order_of_the_scalar_values_they_write() {
        let points: [char; 12] = [
            '\u{0}',
            '\u{7f}',
            '\u{80}',
            '\u{7ff}',
            '\u{800}',
            '\u{d7ff}',
            '\u{e000}',
            '\u{ffe5}',
            '\u{ffff}',
            '\u{10000}',
            '\u{20bb7}',
            '\u{10ffff}',
        ];
        let mut written = [0u8; 4];
        let mut other = [0u8; 4];
        for one in points {
            for another in points {
                let a = one.encode_utf8(&mut written).as_bytes();
                let b = another.encode_utf8(&mut other).as_bytes();
                assert_eq!(compare(a, b), one.cmp(&another), "{one:?} {another:?}");
            }
        }
    }

    /// A sequence cut short at the end is read as far as the slice goes and no further.
    #[test]
    fn a_sequence_cut_short_reads_nothing_past_the_end() {
        let whole = "日".as_bytes();
        for cut in 1..whole.len() {
            let part: Vec<u8> = whole[..cut].to_vec();
            assert_eq!(code_points(&part), 1);
        }
    }
}
