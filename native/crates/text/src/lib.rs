//! What the language says text means, over the bytes it is kept in and nothing else.
//!
//! A Souther string is UTF-8 in NFC wherever it is kept, and what the language asks of it — how
//! long it is, which of two comes first — is a question about the text and not about where it
//! stands. So everything here takes a slice and answers a number or an ordering: no arena, no
//! address of either runtime's width, no layout. That is what lets this move, as it is, into what
//! the native and the wasm runtimes both read (#17), the way `souther-json-syntax` is shaped to.
//!
//! The text is read one code point at a time by [`decoded`], and every operation here is written
//! over it and not over a reading of its own. Two readings agree on well-formed text and part on
//! the rest — one counts the bytes that start a code point, another steps by what a first byte says
//! — and the difference is found by nobody, since a Souther string is never ill-formed. One reading
//! means that where they would part, they cannot.

#![no_std]

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

/// Two runs of text, compared by UTF-16 code unit.
///
/// Which is what the language says text is ordered by, and it is said there rather than worked out
/// here: `<` `<=` `>` `>=` compare lexicographically over UTF-16 code units, and a carrier that
/// stores a string some other way orders it as if it were that sequence regardless — the
/// representation is this carrier's to choose and the order is not (spec §equality).
///
/// It is not the order the bytes are in, and not the order the code points are in either, which are
/// the same order as each other. A code point past the basic plane is two units beginning at D800
/// and a unit from E000 up is one, so `𠮷` (U+20BB7) comes before `￥` (U+FFE5) here and after it by
/// either of the other two readings.
pub fn compare_utf8_as_utf16(left: &[u8], right: &[u8]) -> Ordering {
    let mut a = Units::over(left);
    let mut b = Units::over(right);
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x != y => return x.cmp(&y),
            _ => {}
        }
    }
}

/// The UTF-16 code units a run of UTF-8 spells, one at a time.
///
/// A pair is answered over two turns, which is what `pending` holds: the second unit of a surrogate
/// pair is never nought, so nought stands for there being none.
struct Units<'a> {
    text: &'a [u8],
    at: usize,
    pending: u16,
}

impl<'a> Units<'a> {
    fn over(text: &'a [u8]) -> Units<'a> {
        Units {
            text,
            at: 0,
            pending: 0,
        }
    }

    fn next(&mut self) -> Option<u16> {
        if self.pending != 0 {
            let low = self.pending;
            self.pending = 0;
            return Some(low);
        }
        let (point, width) = decoded(self.text, self.at)?;
        self.at += width;
        if point > 0xffff {
            let rest = point - 0x10000;
            self.pending = 0xdc00 + (rest & 0x3ff) as u16;
            Some(0xd800 + (rest >> 10) as u16)
        } else {
            Some(point as u16)
        }
    }
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

    /// Text is ordered by UTF-16 code unit, which differs from the order of the code points exactly
    /// where one is past the basic plane and the other is at E000 or above.
    #[test]
    fn text_is_ordered_by_utf16_code_unit() {
        assert_eq!(
            compare_utf8_as_utf16("𠮷".as_bytes(), "￥".as_bytes()),
            Ordering::Less
        );
        assert_eq!(
            compare_utf8_as_utf16("a".as_bytes(), "ab".as_bytes()),
            Ordering::Less
        );
        assert_eq!(
            compare_utf8_as_utf16("b".as_bytes(), "ab".as_bytes()),
            Ordering::Greater
        );
        assert_eq!(
            compare_utf8_as_utf16("日本".as_bytes(), "日本".as_bytes()),
            Ordering::Equal
        );
    }

    /// The length and the order read ill-formed bytes the same way, because both read them through
    /// one decoding: the length is how many code points the order walks over. A reading of its own
    /// for either would part from the other here and nowhere a Souther string can reach.
    #[test]
    fn the_length_and_the_order_read_the_same_code_points() {
        let texts: [&[u8]; 8] = [
            b"",
            b"plain",
            "日本語🇯🇵".as_bytes(),
            b"\x80",
            b"\xe3\x81",
            b"a\xf0\x9f",
            b"\xc3",
            b"\xff\xfe",
        ];
        for text in texts {
            let mut units = Units::over(text);
            let mut walked = 0;
            loop {
                // The second half of a pair is the same code point as the first.
                let starts_one = units.pending == 0;
                if units.next().is_none() {
                    break;
                }
                if starts_one {
                    walked += 1;
                }
            }
            assert_eq!(code_points(text), walked, "{text:?}");
        }
    }

    /// A sequence cut short at the end is read as far as the slice goes and no further.
    #[test]
    fn a_sequence_cut_short_reads_nothing_past_the_end() {
        let whole = "日".as_bytes();
        for cut in 1..whole.len() {
            let part: Vec<u8> = whole[..cut].to_vec();
            assert_eq!(code_points(&part), 1);
            let _ = compare_utf8_as_utf16(&part, whole);
        }
    }
}
