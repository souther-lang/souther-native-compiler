//! Decimal text, which `String.toDecimal` reads (spec §string-decimal-text).
//!
//! Only which text is decimal text, and which of its digits stand where. What amount the digits
//! are is not text and is not answered here: it is the runtime's, which keeps a `Decimal` and does
//! its arithmetic. So the reading stops at the digits, and a number of any length is read without
//! anything here holding it as a number.
//!
//! The grammar is Souther's and not a host parser's, as integer text's is: an optional ASCII sign,
//! one or more ASCII digits, and optionally a point followed by one or more ASCII digits. No
//! exponent, no point with nothing on one side of it, and no digit from outside ASCII.

use crate::Text;

/// Decimal text taken apart: the sign, and the digits either side of the point.
///
/// The digits are the text's own, leading and trailing zeros included, since those are what the
/// scale is read off: `1.50` has two fractional digits and is read at scale 2.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DecimalText<'a> {
    /// Whether a `-` was written. `-0.0` writes one and is nought all the same, which is the
    /// amount's to say and not the text's.
    pub negative: bool,
    /// The digits before the point: one or more.
    pub whole: &'a [u8],
    /// The digits after the point, none where no point was written.
    pub fraction: &'a [u8],
}

/// The text taken apart, where it is decimal text.
pub fn decimal_text(text: Text<'_>) -> Option<DecimalText<'_>> {
    let text = text.as_bytes();
    let (negative, unsigned) = match text.split_first()? {
        (b'-', rest) => (true, rest),
        (b'+', rest) => (false, rest),
        _ => (false, text),
    };
    let (whole, fraction) = match unsigned.iter().position(|&it| it == b'.') {
        Some(point) => {
            let fraction = &unsigned[point + 1..];
            // A point says digits follow it, so one with none after it is not decimal text.
            if fraction.is_empty() {
                return None;
            }
            (&unsigned[..point], fraction)
        }
        None => (unsigned, &[][..]),
    };
    let digits = |run: &[u8]| run.iter().all(u8::is_ascii_digit);
    (!whole.is_empty() && digits(whole) && digits(fraction)).then_some(DecimalText {
        negative,
        whole,
        fraction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Option<(bool, &str, &str)> {
        decimal_text(Text::held(text)).map(|it| {
            (
                it.negative,
                core::str::from_utf8(it.whole).unwrap(),
                core::str::from_utf8(it.fraction).unwrap(),
            )
        })
    }

    /// The rows `Strings.isDecimalText` answers on the JVM, and the ones either side of each rule.
    #[test]
    fn decimal_text_is_a_sign_digits_and_digits_after_a_point() {
        for (text, taken) in [
            ("1", Some((false, "1", ""))),
            ("1.0", Some((false, "1", "0"))),
            ("001.50", Some((false, "001", "50"))),
            ("+1.5", Some((false, "1", "5"))),
            ("-0.00", Some((true, "0", "00"))),
            ("-12", Some((true, "12", ""))),
            ("", None),
            ("-", None),
            ("+", None),
            (".5", None),
            ("5.", None),
            ("-.5", None),
            ("1..5", None),
            ("1.5.0", None),
            ("1e5", None),
            ("1E+5", None),
            (" 1", None),
            ("1 ", None),
            ("１２３.４５", None),
            ("٣.٣", None),
            ("--1", None),
            ("+-1", None),
        ] {
            assert_eq!(read(text), taken, "{text:?}");
        }
    }
}
