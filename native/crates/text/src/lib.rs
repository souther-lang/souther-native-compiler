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
//! Where the language names a Unicode version — for NFC, for case, for white space — what it says
//! is notation-199x's, which states the rules for text once for every Souther and Raoh runtime and
//! answers at the version the specification names whatever Rust release it is built with.

#![no_std]

extern crate alloc;

mod canonical;
mod capacity;
mod case;
mod decimal;
mod integer;
mod operations;

pub use capacity::{Capacity, LONGEST_TEXT};
pub use case::{lowercase, uppercase};
pub use decimal::{DecimalText, decimal_text};
pub use integer::{integer, written};
pub use operations::{
    append, characters, code_points_of, contains, ends_with, is_whitespace, join, lines, pad_left,
    pad_right, repeat, replace, reverse, slice, split, starts_with, trim, words,
};

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

/// Why text handed to [`admitted`] is not a string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionRefusal {
    /// The bytes are not a sequence of Unicode scalar values.
    NotText,
    /// The text is scalar values, but its canonical value is longer than a string holds
    /// (spec §what-a-string-holds).
    NoPlace,
}

/// Text arriving from outside, as a string holds it: in NFC and within `capacity`, or refused.
///
/// The one way text becomes a string, whichever door it came through — a decoder's string leaf, a
/// host handing text in. The doors differ in how they say no and not in what they refuse. Refused
/// and not repaired: bytes that are not UTF-8 are no text, and reading them as U+FFFD would make
/// them and U+FFFD itself one value. It is the canonical value that is measured against
/// `capacity`, and not how long the text arrived, since that is the string the text would be
/// (spec §what-a-string-holds): admission establishes every representation invariant a `String`
/// holds at once — scalar values, NFC, within capacity — so nothing after it needs to ask again.
pub fn admitted(bytes: &[u8], capacity: Capacity) -> Result<Cow<'_, str>, AdmissionRefusal> {
    let text = core::str::from_utf8(bytes).map_err(|_| AdmissionRefusal::NotText)?;
    if text.is_ascii() {
        return capacity
            .holds(text.len() as i64)
            .then_some(Cow::Borrowed(text))
            .ok_or(AdmissionRefusal::NoPlace);
    }
    canonical::nfc_of_input(text, capacity)
        .map(Cow::Owned)
        .ok_or(AdmissionRefusal::NoPlace)
}

/// How long the text is as the language counts it, in code points (`String.length`).
///
/// Not the bytes it is kept in, and not the characters a reader sees: `𠮷` is one code point and
/// four bytes, and `🇯🇵` is two code points and one flag.
pub fn code_points(text: Text) -> usize {
    notation199x::scalar_count(text.0)
}

/// Two texts, in the order the language gives text.
///
/// A string is a sequence of Unicode scalar values and is ordered lexicographically over them: the
/// first value where the two differ decides, and a run that begins the other comes before it (spec
/// §equality), which is notation-199x's order of text.
pub fn compare(left: Text, right: Text) -> Ordering {
    notation199x::compare(left.0, right.0)
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

    const PLENTY: Capacity = Capacity::of_code_points(1 << 20);

    /// Bytes that are not UTF-8 are refused as `NotText` and not repaired, and what is admitted is
    /// in NFC.
    #[test]
    fn what_is_admitted_is_utf_8_put_in_nfc() {
        assert_eq!(admitted(b"\xff", PLENTY), Err(AdmissionRefusal::NotText));
        assert_eq!(
            admitted(b"a\xed\xa0\x80", PLENTY),
            Err(AdmissionRefusal::NotText)
        );
        assert_eq!(
            admitted("e\u{301}".as_bytes(), PLENTY).as_deref(),
            Ok("\u{e9}")
        );
        assert_eq!(admitted(b"plain", PLENTY).as_deref(), Ok("plain"));
    }

    /// Capacity is measured against the canonical value, not the input: `e` + a combining acute (2
    /// code points) composes to `é` (1 code point), which fits a capacity the input alone would
    /// not (spec §what-a-string-holds).
    #[test]
    fn a_shrinking_combining_form_is_admitted_within_the_composed_capacity() {
        let one = Capacity::of_code_points(1);
        assert_eq!(
            admitted("e\u{301}".as_bytes(), one).as_deref(),
            Ok("\u{e9}")
        );
    }

    /// U+0344 (COMBINING GREEK DIALYTIKA TONOS) is one code point that decomposes under NFC to two
    /// (U+0308 U+0301), so a capacity that holds the input does not hold its canonical value.
    #[test]
    fn an_expanding_combining_form_is_refused_where_the_canonical_value_has_no_place() {
        let one = Capacity::of_code_points(1);
        assert_eq!(
            admitted("\u{344}".as_bytes(), one),
            Err(AdmissionRefusal::NoPlace)
        );
        assert!(admitted("\u{344}".as_bytes(), Capacity::of_code_points(2)).is_ok());
    }

    /// The ASCII fast path still spends the capacity: it does not build text past what is held
    /// just because normalization is skipped.
    #[test]
    fn ascii_past_capacity_is_refused_too() {
        assert_eq!(
            admitted(b"abcd", Capacity::of_code_points(3)),
            Err(AdmissionRefusal::NoPlace)
        );
        assert_eq!(
            admitted(b"abc", Capacity::of_code_points(3)).as_deref(),
            Ok("abc")
        );
    }

    /// What admission establishes is not only that a text is a `String`, but that `""` is
    /// `append`'s identity for it at the same capacity: the root cause `souther-native-compiler#109`
    /// fixes is that this law did not hold for every successfully admitted string. This is the
    /// executable specification of the fix, not a regression example.
    #[test]
    fn admission_establishes_the_append_identity_law() {
        for (text, capacity) in [
            (b"plain".as_slice(), Capacity::of_code_points(5)),
            ("café".as_bytes(), Capacity::of_code_points(4)),
            ("e\u{301}".as_bytes(), Capacity::of_code_points(1)),
            ("\u{344}".as_bytes(), Capacity::of_code_points(2)),
        ] {
            let admitted_text = admitted(text, capacity).expect("within capacity");
            let admitted_text = admitted_text.as_ref();
            assert_eq!(
                append(held(""), held(admitted_text), capacity).as_deref(),
                Some(admitted_text),
                "append(\"\", {admitted_text:?})"
            );
            assert_eq!(
                append(held(admitted_text), held(""), capacity).as_deref(),
                Some(admitted_text),
                "append({admitted_text:?}, \"\")"
            );
        }
    }

    /// A text a string holds is in NFC, which a debug build holds it to.
    #[test]
    #[should_panic(expected = "a string holds text in NFC")]
    fn a_text_not_in_nfc_is_not_held() {
        Text::held("e\u{301}");
    }
}
