//! What the language says text means, over the text alone.
//!
//! A Souther string is a sequence of Unicode scalar values in NFC (spec §string-code-points,
//! §string-canonical), and what the language asks of it — how long it is, which of two comes first,
//! what `trim` or `lowercase` or `matches` answers — is a question about the text and not about
//! where it stands. So everything here takes [`Text`] and answers numbers, orderings, pieces of what
//! it was handed, or text it built: no arena, no address of either runtime's width, no layout. That
//! is what lets this move, as it is, into what the native and the wasm runtimes both read (#17),
//! the way `souther-json-syntax` is shaped to. What the runtime adds is where the answer is kept.
//!
//! A [`Text`] is what a string holds, and so is already what the language says a string is. Text
//! from outside becomes one through [`admitted`] and nowhere else, which refuses what is not UTF-8
//! and puts the rest in NFC. What is built here is built from texts and is put in NFC where joining
//! can leave it not, so it is one too. Nothing here reads bytes that may be anything, and nothing
//! here puts text in NFC that already is.
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

pub use case::{lowercase, uppercase};
pub use integer::{integer, written};
pub use operations::{
    MOST, append, characters, code_points_of, contains, ends_with, is_whitespace, join, lines,
    pad_left, pad_right, repeat, replace, reverse, slice, split, starts_with, trim, words,
};
pub use tables::UNICODE_VERSION;

use alloc::borrow::Cow;
use core::cmp::Ordering;

/// What a Souther string holds: Unicode scalar values, in NFC.
///
/// Scalar values because it is a `str`. NFC because every door text comes in by admits it in NFC
/// ([`admitted`]) and everything built here from texts is put in it, so a string holds nothing
/// else. That is what lets an operation join two texts by putting only the seam in NFC again, and
/// search one for another as the `str`s they are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Text<'a>(&'a str);

impl<'a> Text<'a> {
    /// The text a string holds.
    ///
    /// Held to be NFC, which is the language's and not a question of memory: a text that was not
    /// would be answered for as the text it is, and two equal texts written two ways would compare
    /// unequal. A debug build checks it, which is what every test runs as.
    pub fn held(text: &'a str) -> Text<'a> {
        debug_assert!(
            text.is_ascii() || canonical::nfc(text) == text,
            "a string holds text in NFC: {text:?}"
        );
        Text(text)
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }

    pub fn as_bytes(self) -> &'a [u8] {
        self.0.as_bytes()
    }
}

/// Text arriving from outside, as a string holds it: in NFC, or nothing where it is not UTF-8.
///
/// The one way text becomes a string, whichever door it came through — a decoder's string leaf, a
/// host handing text in. The doors differ in how they say no and not in what they refuse. Refused
/// and not repaired: bytes that are not UTF-8 are no text, and reading them as U+FFFD would make
/// them and U+FFFD itself one value.
pub fn admitted(bytes: &[u8]) -> Option<Cow<'_, str>> {
    let text = core::str::from_utf8(bytes).ok()?;
    Some(if text.is_ascii() {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(canonical::nfc(text))
    })
}

/// How long the text is as the language counts it, in code points (`String.length`).
///
/// Not the bytes it is kept in, and not the characters a reader sees: `𠮷` is one code point and
/// four bytes, and `🇯🇵` is two code points and one flag.
pub fn code_points(text: Text) -> usize {
    text.0.chars().count()
}

/// Two texts, in the order the language gives text.
///
/// A string is a sequence of Unicode scalar values and is ordered lexicographically over them: the
/// first value where the two differ decides, and a run that begins the other comes before it (spec
/// §equality). UTF-8 writes scalar values in an order its bytes keep — a greater value is written
/// with a greater first byte, or the same first byte and a greater byte after it — so the order of
/// the bytes is that order, and nothing is decoded to answer it. A carrier holding UTF-16 would
/// have to correct its units where a surrogate meets a unit from E000 up; this one has nothing to
/// correct.
pub fn compare(left: Text, right: Text) -> Ordering {
    left.as_bytes().cmp(right.as_bytes())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    fn held(text: &str) -> Text<'_> {
        Text::held(text)
    }

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
            assert_eq!(code_points(held(text)), counted, "{text}");
        }
    }

    /// Text is ordered by scalar value, which puts a character past the basic plane after one from
    /// E000 up: the order a JVM string's UTF-16 units are in is the other way round there.
    #[test]
    fn text_is_ordered_by_scalar_value() {
        assert_eq!(compare(held("￥"), held("𠮷")), Ordering::Less);
        assert_eq!(compare(held("a"), held("ab")), Ordering::Less);
        assert_eq!(compare(held("b"), held("ab")), Ordering::Greater);
        assert_eq!(compare(held("日本"), held("日本")), Ordering::Equal);
        assert_eq!(compare(held(""), held("a")), Ordering::Less);
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
                let a = one.encode_utf8(&mut written);
                let b = another.encode_utf8(&mut other);
                assert_eq!(
                    compare(held(a), held(b)),
                    one.cmp(&another),
                    "{one:?} {another:?}"
                );
            }
        }
    }

    /// Bytes that are not UTF-8 are refused and not repaired, and what is admitted is in NFC.
    #[test]
    fn what_is_admitted_is_utf_8_put_in_nfc() {
        assert_eq!(admitted(b"\xff").as_deref(), None);
        assert_eq!(admitted(b"a\xed\xa0\x80").as_deref(), None);
        assert_eq!(admitted("e\u{301}".as_bytes()).as_deref(), Some("\u{e9}"));
        assert_eq!(admitted(b"plain").as_deref(), Some("plain"));
    }

    /// A text a string holds is in NFC, which a debug build holds it to.
    #[test]
    #[should_panic(expected = "a string holds text in NFC")]
    fn a_text_not_in_nfc_is_not_held() {
        Text::held("e\u{301}");
    }
}
