//! A `Decimal` read out of text this runtime reads.
//!
//! What a `Decimal` is and what each operation on one answers is [`Amount`]'s, in `souther_exact`,
//! which the WebAssembly runtime answers from too. What is here is how text a string holds comes to
//! digits and a scale, which is this runtime's: it reads a string through `souther_text`.

pub(crate) use souther_exact::Amount;
use souther_text::DecimalText;

/// The value decimal text writes, at the scale its fractional digits give it
/// (`String.toDecimal`).
///
/// Always one: a scale is as many digits as the text has after its point, and a string is shorter
/// than the largest scale.
pub(crate) fn of_decimal_text(text: DecimalText) -> Amount {
    let scale = i32::try_from(text.fraction.len())
        .expect("a string holds fewer digits than the largest scale");
    let mut digits = Vec::with_capacity(text.whole.len() + text.fraction.len());
    digits.extend_from_slice(text.whole);
    digits.extend_from_slice(text.fraction);
    Amount::of_digits(text.negative, &digits, scale)
}

/// The whole number integer text writes, at `scale`: what a host hands over as a value's integer
/// and its scale. Nothing where the text writes no integer.
pub(crate) fn of_integer_text(text: DecimalText, scale: i32) -> Option<Amount> {
    text.fraction
        .is_empty()
        .then(|| Amount::of_digits(text.negative, text.whole, scale))
}

/// `String.fromDecimal`: the value in plain notation at the scale it carries, where that text is
/// no longer than a string holds.
pub(crate) fn plain_text(amount: &Amount) -> Option<String> {
    amount.plain_text(crate::STRING_HOLDS.code_points())
}

#[cfg(test)]
mod tests {
    use super::*;
    use souther_text::{Text, decimal_text};

    fn shown(amount: &Amount) -> (String, i32) {
        (amount.unscaled_text(), amount.scale())
    }

    #[test]
    fn decimal_text_is_read_at_the_scale_of_its_fraction() {
        for (text, unscaled, scale) in [
            ("1", "1", 0),
            ("1.0", "10", 1),
            ("001.50", "150", 2),
            ("-0.00", "0", 2),
            ("+7.25", "725", 2),
        ] {
            let read = of_decimal_text(decimal_text(Text::held(text)).unwrap());
            assert_eq!(shown(&read), (unscaled.to_string(), scale), "{text}");
        }
    }

    /// Text one character longer than a string holds is not written. Where the edge stands is
    /// `souther_exact`'s to hold; what is held here is that the bound it is handed is this
    /// runtime's string's.
    #[test]
    fn plain_text_stops_where_a_string_does() {
        let holds = crate::STRING_HOLDS.code_points();
        let past = Amount::of_parts(false, &[1], (holds - 1) as i32);
        assert_eq!(plain_text(&past), None);
        let short = Amount::of_parts(false, &[1], 3);
        assert_eq!(plain_text(&short).as_deref(), Some("0.001"));
    }
}
