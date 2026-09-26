//! What the `String` module's operations answer, over the text alone (spec §stdlib-string).
//!
//! An operation that takes a string apart answers pieces of the text it was handed, borrowed from
//! it: a piece of NFC text is NFC, so nothing is built. One that builds a string out of others
//! answers the text it built, put in NFC again, since NFC is not closed under joining. Every
//! length, index and count is in code points.
//!
//! A search for one run of text inside another is a search over bytes. UTF-8 never writes the
//! bytes of one code point in the middle of another's, so where the bytes of a well-formed run
//! are found in well-formed text they stand at code points and nowhere else.

use crate::canonical::nfc;
use crate::{code_points, decoded, scalar_values};
use alloc::vec::Vec;

/// The most copies [`repeat`] makes, and the widest [`pad_left`] and [`pad_right`] widen to.
///
/// The language says a count no string could hold aborts, and not which count that is
/// (souther-lang/souther#1986). The JVM carrier answers with the largest `int`, and this is that
/// number, so that the two abort for the same counts.
pub const MOST: i64 = i32::MAX as i64;

/// Whether a code point is String whitespace (spec §string-whitespace): the 25 code points of
/// Unicode 18.0's `White_Space`, written out rather than read off a table, as the specification
/// writes them.
pub fn is_whitespace(point: u32) -> bool {
    matches!(
        point,
        0x09..=0x0d
            | 0x20
            | 0x85
            | 0xa0
            | 0x1680
            | 0x2000..=0x200a
            | 0x2028
            | 0x2029
            | 0x202f
            | 0x205f
            | 0x3000
    )
}

/// Where each code point starts, and where the text ends: one more than there are code points.
fn boundaries(text: &[u8]) -> Vec<usize> {
    let mut at = 0;
    let mut starts = Vec::new();
    while let Some((_, width)) = decoded(text, at) {
        starts.push(at);
        at += width;
    }
    starts.push(text.len());
    starts
}

/// Where the first run of `needle` starts in `text` at or after `from`.
fn found(text: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from);
    }
    text.get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| from + at)
}

/// Whether `needle` is in the text (`String.contains`). The empty text is in every text.
pub fn contains(needle: &[u8], text: &[u8]) -> bool {
    found(text, needle, 0).is_some()
}

/// Whether the text begins with `prefix` (`String.startsWith`).
pub fn starts_with(prefix: &[u8], text: &[u8]) -> bool {
    text.starts_with(prefix)
}

/// Whether the text ends with `suffix` (`String.endsWith`).
pub fn ends_with(suffix: &[u8], text: &[u8]) -> bool {
    text.ends_with(suffix)
}

/// The code points from `from` up to but not including `to` (`String.slice`), or nothing where the
/// string has no such index or `to` is before `from`.
pub fn slice(from: i64, to: i64, text: &[u8]) -> Option<&[u8]> {
    let starts = boundaries(text);
    let at = |index: i64| starts.get(usize::try_from(index).ok()?).copied();
    let (begin, end) = (at(from)?, at(to)?);
    text.get(begin..end)
}

/// The text with the String whitespace at either end taken off (`String.trim`).
pub fn trim(text: &[u8]) -> &[u8] {
    let starts = boundaries(text);
    let white = |at: usize| decoded(text, starts[at]).is_some_and(|(it, _)| is_whitespace(it));
    let points = starts.len() - 1;
    let mut begin = 0;
    while begin < points && white(begin) {
        begin += 1;
    }
    let mut end = points;
    while end > begin && white(end - 1) {
        end -= 1;
    }
    &text[starts[begin]..starts[end]]
}

/// The runs of the text between runs of String whitespace, none of them empty (`String.words`).
pub fn words(text: &[u8]) -> Vec<&[u8]> {
    let mut pieces = Vec::new();
    let mut begun: Option<usize> = None;
    let mut at = 0;
    while let Some((point, width)) = decoded(text, at) {
        match (is_whitespace(point), begun) {
            (true, Some(from)) => {
                pieces.push(&text[from..at]);
                begun = None;
            }
            (false, None) => begun = Some(at),
            _ => {}
        }
        at += width;
    }
    if let Some(from) = begun {
        pieces.push(&text[from..]);
    }
    pieces
}

/// The pieces between each run of `separator`, empty ones kept (`String.split`). An empty
/// separator splits nothing off: the text is the one piece.
pub fn split<'a>(separator: &[u8], text: &'a [u8]) -> Vec<&'a [u8]> {
    if separator.is_empty() {
        return alloc::vec![text];
    }
    let mut pieces = Vec::new();
    let mut from = 0;
    while let Some(at) = found(text, separator, from) {
        pieces.push(&text[from..at]);
        from = at + separator.len();
    }
    pieces.push(&text[from..]);
    pieces
}

/// The lines of the text (`String.lines`): broken at each `\n`, a `\r` just before it going with
/// it, and empty ones kept, so a newline at the end leaves an empty last line. A `\r` alone breaks
/// nothing.
pub fn lines(text: &[u8]) -> Vec<&[u8]> {
    let mut pieces = split(b"\n", text);
    let last = pieces.len() - 1;
    for piece in &mut pieces[..last] {
        if let Some(kept) = piece.strip_suffix(b"\r") {
            *piece = kept;
        }
    }
    pieces
}

/// Each code point of the text, as the piece of it that writes it (`String.characters`).
pub fn characters(text: &[u8]) -> Vec<&[u8]> {
    let starts = boundaries(text);
    starts
        .windows(2)
        .map(|pair| &text[pair[0]..pair[1]])
        .collect()
}

/// The two joined, in NFC (`String.append`, and `++` over two strings).
pub fn append(left: &[u8], right: &[u8]) -> Vec<u8> {
    let mut joined = Vec::with_capacity(left.len() + right.len());
    joined.extend_from_slice(left);
    joined.extend_from_slice(right);
    nfc(&joined)
}

/// The pieces joined with `separator` between each two, in NFC (`String.join`; `String.concat`
/// is this with no separator).
pub fn join<'a>(separator: &[u8], pieces: impl IntoIterator<Item = &'a [u8]>) -> Vec<u8> {
    let mut joined = Vec::new();
    for (at, piece) in pieces.into_iter().enumerate() {
        if at > 0 {
            joined.extend_from_slice(separator);
        }
        joined.extend_from_slice(piece);
    }
    nfc(&joined)
}

/// Every run of `target` replaced by `replacement`, left to right and none overlapping, in NFC
/// (`String.replace`). An empty target replaces nothing, rather than putting the replacement
/// between every two code points.
pub fn replace(target: &[u8], replacement: &[u8], text: &[u8]) -> Vec<u8> {
    if target.is_empty() {
        return text.to_vec();
    }
    join(replacement, split(target, text))
}

/// The code points in the opposite order, in NFC (`String.reverse`). A mark reversed to stand
/// after a letter it composes with composes, so what comes back need not be as long.
pub fn reverse(text: &[u8]) -> Vec<u8> {
    let mut pieces = characters(text);
    pieces.reverse();
    join(b"", pieces)
}

/// `copies` copies of the text joined, in NFC (`String.repeat`): nothing for a count of nought or
/// fewer, or of the empty text, and nothing at all past [`MOST`], where the run is to end instead.
pub fn repeat(copies: i64, text: &[u8]) -> Option<Vec<u8>> {
    if copies <= 0 || text.is_empty() {
        return Some(Vec::new());
    }
    if copies > MOST {
        return None;
    }
    let copies = usize::try_from(copies).ok()?;
    let mut joined = Vec::with_capacity(text.len().checked_mul(copies)?);
    for _ in 0..copies {
        joined.extend_from_slice(text);
    }
    Some(nfc(&joined))
}

/// The text widened on the left to `width` code points with copies of `pad`
/// (`String.padLeft`), or nothing past [`MOST`].
pub fn pad_left(width: i64, pad: &[u8], text: &[u8]) -> Option<Vec<u8>> {
    widened(width, pad, text, true)
}

/// The text widened on the right, as [`pad_left`] widens it on the left (`String.padRight`).
pub fn pad_right(width: i64, pad: &[u8], text: &[u8]) -> Option<Vec<u8>> {
    widened(width, pad, text, false)
}

/// The text widened to exactly `width` code points, and left alone where it is that wide already
/// or `pad` is empty.
///
/// The fill is `pad` repeated as often as covers what is needed, put in NFC and cut to what is
/// needed from its front, and then joined to the text and put in NFC again. Composing where two
/// copies of `pad` meet, or where the fill meets the text, can take a code point away, so where
/// the join comes up short the fill is made again one code point longer.
fn widened(width: i64, pad: &[u8], text: &[u8], before: bool) -> Option<Vec<u8>> {
    let long = code_points(text) as i64;
    if pad.is_empty() || long >= width {
        return Some(text.to_vec());
    }
    if width > MOST {
        return None;
    }
    let pad_long = code_points(pad) as i64;
    let mut needed = width - long;
    loop {
        let copies = (needed + pad_long - 1) / pad_long;
        let fill = repeat(copies, pad)?;
        let fill = if code_points(&fill) as i64 > needed {
            slice(0, needed, &fill)?.to_vec()
        } else {
            fill
        };
        let joined = if before {
            append(&fill, text)
        } else {
            append(text, &fill)
        };
        if code_points(&joined) as i64 >= width {
            return Some(joined);
        }
        needed += 1;
    }
}

/// The code points of the text, as numbers (`String.codePoints`).
pub fn code_points_of(text: &[u8]) -> Vec<i64> {
    scalar_values(text).map(i64::from).collect()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::string::String;
    use std::vec::Vec;

    fn texts(pieces: Vec<&[u8]>) -> Vec<&str> {
        pieces
            .into_iter()
            .map(|it| core::str::from_utf8(it).unwrap())
            .collect()
    }

    fn text(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn a_slice_counts_code_points_and_refuses_what_the_string_has_not_got() {
        let s = "a𠮷b日".as_bytes();
        assert_eq!(slice(1, 3, s), Some("𠮷b".as_bytes()));
        assert_eq!(slice(0, 4, s), Some(s));
        assert_eq!(slice(4, 4, s), Some(&b""[..]));
        assert_eq!(slice(0, 5, s), None);
        assert_eq!(slice(-1, 2, s), None);
        assert_eq!(slice(3, 2, s), None);
    }

    #[test]
    fn whitespace_is_the_languages_and_not_asciis() {
        assert_eq!(trim("\u{3000} a b\u{a0}\n".as_bytes()), b"a b");
        assert_eq!(trim(b"   "), b"");
        assert_eq!(trim("\u{1c}a".as_bytes()), "\u{1c}a".as_bytes());
        assert_eq!(
            texts(words("  a\u{3000}b\u{2003}c ".as_bytes())),
            ["a", "b", "c"]
        );
        assert!(words(b" \t ").is_empty());
    }

    #[test]
    fn a_split_keeps_empty_pieces_and_an_empty_separator_splits_nothing() {
        assert_eq!(texts(split(b",", b"a,,b")), ["a", "", "b"]);
        assert_eq!(texts(split(b",", b"")), [""]);
        assert_eq!(texts(split(b"", b"abc")), ["abc"]);
        assert_eq!(
            texts(split("日".as_bytes(), "a日b日".as_bytes())),
            ["a", "b", ""]
        );
    }

    #[test]
    fn a_line_ends_at_a_newline_and_a_return_just_before_it() {
        assert_eq!(texts(lines(b"a\nb\r\nc")), ["a", "b", "c"]);
        assert_eq!(texts(lines(b"a\n")), ["a", ""]);
        assert_eq!(texts(lines(b"a\r\r\nb")), ["a\r", "b"]);
        assert_eq!(texts(lines(b"a\rb\r")), ["a\rb\r"]);
        assert_eq!(texts(lines(b"")), [""]);
    }

    #[test]
    fn what_is_built_is_put_in_nfc() {
        assert_eq!(text(append(b"e", "\u{301}".as_bytes())), "\u{e9}");
        assert_eq!(
            text(join(b"", ["e".as_bytes(), "\u{301}".as_bytes()])),
            "\u{e9}"
        );
        assert_eq!(text(join(b"-", ["a".as_bytes(), b"b", b"c"])), "a-b-c");
        assert_eq!(text(replace(b"x", "\u{301}".as_bytes(), b"ex")), "\u{e9}");
        assert_eq!(text(reverse("\u{301}e".as_bytes())), "\u{e9}");
        assert_eq!(text(reverse("a𠮷b".as_bytes())), "b𠮷a");
    }

    #[test]
    fn an_empty_target_replaces_nothing() {
        assert_eq!(text(replace(b"", b"-", b"abc")), "abc");
        assert_eq!(text(replace(b"aa", b"b", b"aaa")), "ba");
    }

    #[test]
    fn a_repeat_past_the_most_copies_is_none() {
        assert_eq!(repeat(3, b"ab").map(text).as_deref(), Some("ababab"));
        assert_eq!(repeat(0, b"ab").map(text).as_deref(), Some(""));
        assert_eq!(repeat(-4, b"ab").map(text).as_deref(), Some(""));
        assert_eq!(repeat(MOST + 1, b"").map(text).as_deref(), Some(""));
        assert_eq!(repeat(MOST + 1, b"ab"), None);
    }

    #[test]
    fn a_pad_widens_to_the_width_exactly() {
        assert_eq!(pad_left(5, b"0", b"42").map(text).as_deref(), Some("00042"));
        assert_eq!(
            pad_right(5, b"xy", b"a").map(text).as_deref(),
            Some("axyxy")
        );
        assert_eq!(pad_left(4, b"xy", b"a").map(text).as_deref(), Some("xyxa"));
        assert_eq!(pad_left(2, b"0", b"123").map(text).as_deref(), Some("123"));
        assert_eq!(pad_left(9, b"", b"a").map(text).as_deref(), Some("a"));
        assert_eq!(pad_left(MOST + 1, b"0", b"a"), None);
        // A pad that composes into what it meets is asked for once more.
        assert_eq!(
            pad_right(2, "\u{301}".as_bytes(), b"e")
                .map(text)
                .as_deref(),
            Some("\u{e9}\u{301}")
        );
    }

    #[test]
    fn a_search_is_over_the_text_and_every_text_holds_the_empty_one() {
        assert!(contains(b"", b"abc"));
        assert!(contains("本".as_bytes(), "日本語".as_bytes()));
        assert!(!contains(b"d", b"abc"));
        assert!(starts_with(b"ab", b"abc"));
        assert!(ends_with(b"bc", b"abc"));
        assert!(!ends_with(b"abcd", b"abc"));
    }

    #[test]
    fn characters_are_one_code_point_each() {
        assert_eq!(texts(characters("a𠮷🇯🇵".as_bytes())), ["a", "𠮷", "🇯", "🇵"]);
        assert!(characters(b"").is_empty());
        assert_eq!(code_points_of("a𠮷".as_bytes()), [0x61, 0x20bb7]);
    }
}
