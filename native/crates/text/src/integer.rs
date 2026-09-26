//! Integer text, which `String.toInt` reads and `String.fromInt` writes (spec
//! §string-integer-text).
//!
//! What is read is Souther's grammar and not a host parser's: an optional ASCII sign and one or
//! more ASCII digits, and nothing else. A decimal digit from outside ASCII is not a digit here, so
//! no Unicode version decides what is a number.

use crate::Text;
use alloc::string::{String, ToString};

/// The integer the text writes in decimal, where it is integer text and the integer is an `Int`.
///
/// Leading zeros are read, before or after a sign, and `-0` is nought. A number outside the range
/// is not integer text of an `Int`, which is an answer and not a reason to end the run.
pub fn integer(text: Text) -> Option<i64> {
    let text = text.as_bytes();
    let (negative, digits) = match text.split_first()? {
        (b'-', rest) => (true, rest),
        (b'+', rest) => (false, rest),
        _ => (false, text),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    for digit in digits {
        if !digit.is_ascii_digit() {
            return None;
        }
        // Counted toward the sign, so that the smallest `Int`, one further from nought than the
        // largest, is reached without passing through a number no `Int` holds.
        let step = i64::from(digit - b'0');
        value = value.checked_mul(10)?;
        value = if negative {
            value.checked_sub(step)?
        } else {
            value.checked_add(step)?
        };
    }
    Some(value)
}

/// The integer written in decimal (`String.fromInt`): a `-` where it is below nought, and no
/// leading zero.
pub fn written(value: i64) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_text_is_an_ascii_sign_and_ascii_digits() {
        for (text, read) in [
            ("7", Some(7)),
            ("007", Some(7)),
            ("+007", Some(7)),
            ("-007", Some(-7)),
            ("-0", Some(0)),
            ("+0", Some(0)),
            ("9223372036854775807", Some(i64::MAX)),
            ("-9223372036854775808", Some(i64::MIN)),
            ("9223372036854775808", None),
            ("-9223372036854775809", None),
            ("", None),
            ("-", None),
            ("+", None),
            (" 5", None),
            ("5 ", None),
            ("12x", None),
            ("１２３", None),
            ("٣", None),
            ("--5", None),
            ("+-5", None),
        ] {
            assert_eq!(integer(Text::held(text)), read, "{text:?}");
        }
    }

    #[test]
    fn an_integer_is_written_in_decimal() {
        assert_eq!(written(0), "0");
        assert_eq!(written(-42), "-42");
        assert_eq!(written(i64::MIN), "-9223372036854775808");
    }
}
