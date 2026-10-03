//! What the `String` module's operations answer, over the text alone (spec §stdlib-string).
//!
//! An operation that takes a string apart answers pieces of the text it was handed, borrowed from
//! it: a piece of NFC text is NFC, so nothing is built. One that builds a string out of others
//! answers the text it built, joined through [`Joined`], which puts what it joined in NFC again. Every length, index and count is in code points.
//!
//! A search for one text inside another is `str`'s, which does not slow down on text that nearly
//! matches over and over. UTF-8 never writes the bytes of one code point in the middle of
//! another's, so where one text is found in another it stands at code points and nowhere else.

use crate::canonical::Joined;
use crate::capacity::{Capacity, code_points as capacity_code_points};
use crate::{Text, code_points};
use alloc::string::String;
use alloc::vec::Vec;

/// Whether a code point is String whitespace (spec §string-whitespace): Unicode 18.0's
/// `White_Space`, as 199x-notation holds it.
pub fn is_whitespace(character: char) -> bool {
    notation199x::is_white_space(character)
}

/// Whether `needle` is in the text (`String.contains`). The empty text is in every text.
pub fn contains(needle: Text, text: Text) -> bool {
    text.as_str().contains(needle.as_str())
}

/// Whether the text begins with `prefix` (`String.startsWith`).
pub fn starts_with(prefix: Text, text: Text) -> bool {
    text.as_str().starts_with(prefix.as_str())
}

/// Whether the text ends with `suffix` (`String.endsWith`).
pub fn ends_with(suffix: Text, text: Text) -> bool {
    text.as_str().ends_with(suffix.as_str())
}

/// The code points from `from` up to but not including `to` (`String.slice`), or nothing where the
/// string has no such index or `to` is before `from`.
///
/// Read as far as `to` and no further.
pub fn slice(from: i64, to: i64, text: Text<'_>) -> Option<Text<'_>> {
    let (from, to) = (usize::try_from(from).ok()?, usize::try_from(to).ok()?);
    if to < from {
        return None;
    }
    let text = text.as_str();
    let mut starts = text.char_indices().map(|(at, _)| at).chain([text.len()]);
    let begin = starts.nth(from)?;
    let end = if to == from {
        begin
    } else {
        starts.nth(to - from - 1)?
    };
    Some(Text(&text[begin..end]))
}

/// The text with the String whitespace at either end taken off (`String.trim`).
pub fn trim(text: Text<'_>) -> Text<'_> {
    Text(text.as_str().trim_matches(is_whitespace))
}

/// The runs of the text between runs of String whitespace, none of them empty (`String.words`).
pub fn words(text: Text<'_>) -> Vec<Text<'_>> {
    text.as_str()
        .split(is_whitespace)
        .filter(|piece| !piece.is_empty())
        .map(Text)
        .collect()
}

/// The pieces between each run of `separator`, empty ones kept (`String.split`). An empty
/// separator splits nothing off: the text is the one piece.
pub fn split<'a>(separator: Text, text: Text<'a>) -> Vec<Text<'a>> {
    if separator.as_str().is_empty() {
        return alloc::vec![text];
    }
    text.as_str().split(separator.as_str()).map(Text).collect()
}

/// The lines of the text (`String.lines`): broken at each `\n`, a `\r` just before it going with
/// it, and empty ones kept, so a newline at the end leaves an empty last line. A `\r` alone breaks
/// nothing.
pub fn lines(text: Text<'_>) -> Vec<Text<'_>> {
    let mut pieces: Vec<&str> = text.as_str().split('\n').collect();
    let last = pieces.len() - 1;
    for piece in &mut pieces[..last] {
        if let Some(kept) = piece.strip_suffix('\r') {
            *piece = kept;
        }
    }
    pieces.into_iter().map(Text).collect()
}

/// Each code point of the text, as the piece of it that writes it (`String.characters`).
pub fn characters(text: Text<'_>) -> Vec<Text<'_>> {
    let text = text.as_str();
    text.char_indices()
        .map(|(at, character)| Text(&text[at..at + character.len_utf8()]))
        .collect()
}

/// The code points of the text, as numbers (`String.codePoints`).
pub fn code_points_of(text: Text) -> Vec<i64> {
    text.as_str()
        .chars()
        .map(|it| i64::from(u32::from(it)))
        .collect()
}

/// The two joined (`String.append`, and `++` over two strings), or nothing where they are more
/// text than the carrier holds.
///
/// Every operation below that builds text answers nothing where what it builds is more than
/// `capacity`, and says so before the text is made: it is spent as the text is built (`Joined`),
/// and none is built past it.
pub fn append(left: Text, right: Text, capacity: Capacity) -> Option<String> {
    join(Text(""), [left, right], capacity)
}

/// The pieces joined with `separator` between each two (`String.join`; `String.concat` is this
/// with no separator).
pub fn join<'a>(
    separator: Text,
    pieces: impl IntoIterator<Item = Text<'a>>,
    capacity: Capacity,
) -> Option<String> {
    let mut joined = Joined::new(capacity);
    for (at, piece) in pieces.into_iter().enumerate() {
        if at > 0 {
            joined.push(separator)?;
        }
        joined.push(piece)?;
    }
    Some(joined.finished())
}

/// Every run of `target` replaced by `replacement`, left to right and none overlapping
/// (`String.replace`). An empty target replaces nothing, rather than putting the replacement
/// between every two code points.
pub fn replace(target: Text, replacement: Text, text: Text, capacity: Capacity) -> Option<String> {
    if target.as_str().is_empty() {
        return Some(String::from(text.as_str()));
    }
    // The pieces are walked and not collected: what is held of them is no more than the text is.
    join(
        replacement,
        text.as_str().split(target.as_str()).map(Text),
        capacity,
    )
}

/// The code points in the opposite order (`String.reverse`). A mark reversed to stand after a
/// letter it composes with composes, so what comes back need not be as long.
pub fn reverse(text: Text, capacity: Capacity) -> Option<String> {
    let text = text.as_str();
    let backwards = text
        .char_indices()
        .rev()
        .map(|(at, character)| Text(&text[at..at + character.len_utf8()]));
    join(Text(""), backwards, capacity)
}

/// `copies` copies of the text joined (`String.repeat`): nothing for a count of nought or fewer,
/// or of the empty text, and no text at all where the copies are more than the carrier holds,
/// where the run is to end instead. Measured before the first is written.
pub fn repeat(copies: i64, text: Text, capacity: Capacity) -> Option<String> {
    if copies <= 0 || text.as_str().is_empty() {
        return Some(String::new());
    }
    if !capacity.holds(capacity_code_points(text.as_str()).saturating_mul(copies)) {
        return None;
    }
    let mut joined = Joined::new(capacity);
    for _ in 0..copies {
        joined.push(text)?;
    }
    Some(joined.finished())
}

/// The text widened on the left to `width` code points with copies of `pad`
/// (`String.padLeft`), or nothing where that is more text than the carrier holds.
pub fn pad_left(width: i64, pad: Text, text: Text, capacity: Capacity) -> Option<String> {
    widened(width, pad, text, true, capacity)
}

/// The text widened on the right, as [`pad_left`] widens it on the left (`String.padRight`).
pub fn pad_right(width: i64, pad: Text, text: Text, capacity: Capacity) -> Option<String> {
    widened(width, pad, text, false, capacity)
}

/// The text widened to exactly `width` code points, and left alone where it is that wide already
/// or `pad` is empty.
///
/// The fill is `pad` repeated as often as covers what is needed, cut to what is needed from its
/// front, and joined to the text. Composing where two copies of `pad` meet, or where the fill
/// meets the text, can take a code point away, so where the join comes up short the fill is made
/// again one code point longer. Each of them is built within what the carrier holds: the copies of
/// `pad`, and the fill and the text together.
fn widened(width: i64, pad: Text, text: Text, before: bool, capacity: Capacity) -> Option<String> {
    let long = code_points(text) as i64;
    if pad.as_str().is_empty() || long >= width {
        return Some(String::from(text.as_str()));
    }
    // Each code point is a unit at least, so a width past what is held has no text that is held.
    if !capacity.holds(width) {
        return None;
    }
    let pad_long = code_points(pad) as i64;
    let mut needed = width - long;
    loop {
        let copies = (needed + pad_long - 1) / pad_long;
        let whole = repeat(copies, pad, capacity)?;
        let fill = slice(0, needed, Text(&whole)).unwrap_or(Text(&whole));
        let joined = if before {
            append(fill, text, capacity)?
        } else {
            append(text, fill, capacity)?
        };
        if code_points(Text(&joined)) as i64 >= width {
            return Some(joined);
        }
        needed += 1;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    fn held(text: &str) -> Text<'_> {
        Text::held(text)
    }

    fn texts<'a>(pieces: Vec<Text<'a>>) -> Vec<&'a str> {
        pieces.into_iter().map(Text::as_str).collect()
    }

    #[test]
    fn a_slice_counts_code_points_and_refuses_what_the_string_has_not_got() {
        let s = held("a𠮷b日");
        assert_eq!(slice(1, 3, s).map(Text::as_str), Some("𠮷b"));
        assert_eq!(slice(0, 4, s).map(Text::as_str), Some("a𠮷b日"));
        assert_eq!(slice(4, 4, s).map(Text::as_str), Some(""));
        assert_eq!(slice(0, 5, s), None);
        assert_eq!(slice(5, 5, s), None);
        assert_eq!(slice(-1, 2, s), None);
        assert_eq!(slice(3, 2, s), None);
    }

    #[test]
    fn whitespace_is_the_languages_and_not_asciis() {
        assert_eq!(trim(held("\u{3000} a b\u{a0}\n")).as_str(), "a b");
        assert_eq!(trim(held("   ")).as_str(), "");
        assert_eq!(trim(held("\u{1c}a")).as_str(), "\u{1c}a");
        assert_eq!(
            texts(words(held("  a\u{3000}b\u{2003}c "))),
            ["a", "b", "c"]
        );
        assert!(words(held(" \t ")).is_empty());
    }

    #[test]
    fn a_split_keeps_empty_pieces_and_an_empty_separator_splits_nothing() {
        assert_eq!(texts(split(held(","), held("a,,b"))), ["a", "", "b"]);
        assert_eq!(texts(split(held(","), held(""))), [""]);
        assert_eq!(texts(split(held(""), held("abc"))), ["abc"]);
        assert_eq!(texts(split(held("日"), held("a日b日"))), ["a", "b", ""]);
    }

    #[test]
    fn a_line_ends_at_a_newline_and_a_return_just_before_it() {
        assert_eq!(texts(lines(held("a\nb\r\nc"))), ["a", "b", "c"]);
        assert_eq!(texts(lines(held("a\n"))), ["a", ""]);
        assert_eq!(texts(lines(held("a\r\r\nb"))), ["a\r", "b"]);
        assert_eq!(texts(lines(held("a\rb\r"))), ["a\rb\r"]);
        assert_eq!(texts(lines(held(""))), [""]);
    }

    /// More than any text here is long.
    const ROOMY: Capacity = Capacity::of_code_points(1 << 20);

    #[test]
    fn what_is_built_is_put_in_nfc() {
        let built = |it: Option<String>| it.expect("within what is held");
        assert_eq!(built(append(held("e"), held("\u{301}"), ROOMY)), "\u{e9}");
        assert_eq!(
            built(join(held(""), [held("e"), held("\u{301}")], ROOMY)),
            "\u{e9}"
        );
        assert_eq!(
            built(join(held("-"), [held("a"), held("b"), held("c")], ROOMY)),
            "a-b-c"
        );
        assert_eq!(
            built(replace(held("x"), held("\u{301}"), held("ex"), ROOMY)),
            "\u{e9}"
        );
        assert_eq!(built(reverse(held("\u{301}e"), ROOMY)), "\u{e9}");
        assert_eq!(built(reverse(held("a𠮷b"), ROOMY)), "b𠮷a");
    }

    #[test]
    fn an_empty_target_replaces_nothing() {
        let replaced = |target, replacement, text| {
            replace(held(target), held(replacement), held(text), ROOMY).unwrap()
        };
        assert_eq!(replaced("", "-", "abc"), "abc");
        assert_eq!(replaced("aa", "b", "aaa"), "ba");
    }

    #[test]
    fn a_repeat_past_what_is_held_is_none() {
        let held_ = Capacity::of_code_points(10);
        assert_eq!(repeat(3, held("ab"), held_).as_deref(), Some("ababab"));
        assert_eq!(repeat(5, held("ab"), held_).as_deref(), Some("ababababab"));
        assert_eq!(repeat(6, held("ab"), held_), None);
        assert_eq!(repeat(0, held("ab"), held_).as_deref(), Some(""));
        assert_eq!(repeat(-4, held("ab"), held_).as_deref(), Some(""));
        assert_eq!(repeat(i64::MAX, held(""), held_).as_deref(), Some(""));
        assert_eq!(repeat(i64::MAX, held("ab"), held_), None);
        // What is held is counted in code points: a code point outside the basic plane is still
        // one, though it is two UTF-16 units and four bytes.
        assert_eq!(
            repeat(5, held("\u{10000}"), held_).map(|it| it.chars().count()),
            Some(5)
        );
        assert_eq!(
            repeat(10, held("\u{10000}"), held_).map(|it| it.chars().count()),
            Some(10)
        );
        assert_eq!(repeat(11, held("\u{10000}"), held_), None);
    }

    #[test]
    fn a_pad_widens_to_the_width_exactly() {
        let pad_left = |width, pad, text| pad_left(width, held(pad), held(text), ROOMY);
        let pad_right = |width, pad, text| pad_right(width, held(pad), held(text), ROOMY);
        assert_eq!(pad_left(5, "0", "42").as_deref(), Some("00042"));
        assert_eq!(pad_right(5, "xy", "a").as_deref(), Some("axyxy"));
        assert_eq!(pad_left(4, "xy", "a").as_deref(), Some("xyxa"));
        assert_eq!(pad_left(2, "0", "123").as_deref(), Some("123"));
        assert_eq!(pad_left(9, "", "a").as_deref(), Some("a"));
        // A pad that composes into what it meets is asked for once more.
        assert_eq!(
            pad_right(2, "\u{301}", "e").as_deref(),
            Some("\u{e9}\u{301}")
        );
    }

    /// The fill and the text together are held to what the carrier holds, in units and not in the
    /// code points the width counts: four code points outside the basic plane are eight units, and
    /// the two units of fill that widen them to six code points come to ten.
    #[test]
    fn a_pad_is_measured_with_the_text_it_widens() {
        let astral = held("\u{10000}\u{10000}\u{10000}\u{10000}");
        for (capacity, holds) in [(6, true), (5, false), (4, false)] {
            let capacity = Capacity::of_code_points(capacity);
            assert_eq!(
                pad_right(6, held("a"), astral, capacity).is_some(),
                holds,
                "{capacity:?}"
            );
            assert_eq!(
                pad_left(6, held("a"), astral, capacity).is_some(),
                holds,
                "{capacity:?}"
            );
        }
        // A width no capacity holds is refused before anything is worked out from it.
        assert_eq!(pad_left(i64::MAX, held("a"), held("b"), ROOMY), None);
    }

    #[test]
    fn a_search_is_over_the_text_and_every_text_holds_the_empty_one() {
        assert!(contains(held(""), held("abc")));
        assert!(contains(held("本"), held("日本語")));
        assert!(!contains(held("d"), held("abc")));
        assert!(starts_with(held("ab"), held("abc")));
        assert!(ends_with(held("bc"), held("abc")));
        assert!(!ends_with(held("abcd"), held("abc")));
    }

    #[test]
    fn characters_are_one_code_point_each() {
        assert_eq!(texts(characters(held("a𠮷🇯🇵"))), ["a", "𠮷", "🇯", "🇵"]);
        assert!(characters(held("")).is_empty());
        assert_eq!(code_points_of(held("a𠮷")), [0x61, 0x20bb7]);
    }
}
