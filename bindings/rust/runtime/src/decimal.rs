//! A Souther `Decimal` as Rust holds one.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A Souther `Decimal`: its integer and its scale, the amount being the integer over ten to the
/// scale.
///
/// The two numbers the language says a `Decimal` is, and nothing else, as the PHP runtime holds
/// one. What a host does with them — a decimal crate, a money type, text — is the host's. The scale
/// is kept as it was written: `1.50` is `150` at scale 2 and `1.5` is `15` at scale 1, and
/// [`Decimal::scale`] says which.
///
/// Equal, ordered and hashed by amount, as Souther's `==`, `<` and `Decimal.compare` compare two
/// whatever their scales: `1.50` and `1.5` are equal, and hash alike. So a `Decimal` is one key of
/// a map or a set however it was written, as it is to the model.
#[derive(Debug, Clone)]
pub struct Decimal {
    unscaled: String,
    scale: i32,
}

impl Decimal {
    /// The `Decimal` whose integer is `unscaled`, written as an optional `-` and ASCII digits, at
    /// `scale`.
    ///
    /// # Errors
    ///
    /// [`NotADecimal`] where `unscaled` is written otherwise. The library ends the process on
    /// such an integer, so it is refused here first.
    pub fn new(unscaled: &str, scale: i32) -> Result<Self, NotADecimal> {
        let (negative, digits) = match unscaled.strip_prefix('-') {
            Some(digits) => (true, digits),
            None => (false, unscaled),
        };
        if digits.is_empty() || !digits.bytes().all(|it| it.is_ascii_digit()) {
            return Err(NotADecimal(unscaled.to_owned()));
        }
        let digits = digits.trim_start_matches('0');
        let unscaled = match (digits.is_empty(), negative) {
            (true, _) => "0".to_owned(),
            (false, true) => format!("-{digits}"),
            (false, false) => digits.to_owned(),
        };
        Ok(Decimal { unscaled, scale })
    }

    /// The `Decimal` of `integer` at `scale`.
    pub fn of(integer: i128, scale: i32) -> Self {
        Decimal {
            unscaled: integer.to_string(),
            scale,
        }
    }

    /// The integer, as an optional `-` and its digits, with no leading zero.
    pub fn unscaled(&self) -> &str {
        &self.unscaled
    }

    /// The scale.
    pub fn scale(&self) -> i32 {
        self.scale
    }
}

impl Decimal {
    /// Whether it is below nought, and its amount as the digits of its integer with no trailing
    /// zero and where the first of them stands: the one way of writing each amount, so two of one
    /// amount are alike here whatever their scales. Nought is `(false, 0, "")`.
    fn amount(&self) -> (bool, i64, &str) {
        let negative = self.unscaled.starts_with('-');
        let digits = self.unscaled.trim_start_matches('-').trim_end_matches('0');
        if digits.is_empty() {
            return (false, 0, "");
        }
        let written = self.unscaled.trim_start_matches('-').len();
        let length = i64::try_from(written).expect("an integer's digits are counted in 64 bits");
        (negative, length - i64::from(self.scale), digits)
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.amount() == other.amount()
    }
}

impl Eq for Decimal {}

impl Hash for Decimal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.amount().hash(state);
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Decimal {
    /// By amount: the sign, then where the first digit stands, then the digits, of which a
    /// shorter one that the longer begins with is the smaller amount since the longer ends in no
    /// zero.
    fn cmp(&self, other: &Self) -> Ordering {
        let (negative, magnitude, digits) = self.amount();
        let (their_negative, their_magnitude, their_digits) = other.amount();
        let signum = |negative: bool, digits: &str| match (negative, digits.is_empty()) {
            (_, true) => 0,
            (true, false) => -1,
            (false, false) => 1,
        };
        let (mine, theirs) = (
            signum(negative, digits),
            signum(their_negative, their_digits),
        );
        if mine != theirs || mine == 0 {
            return mine.cmp(&theirs);
        }
        let by_size = magnitude
            .cmp(&their_magnitude)
            .then_with(|| digits.cmp(their_digits));
        if negative { by_size.reverse() } else { by_size }
    }
}

/// An integer a `Decimal` cannot have, as it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotADecimal(pub String);

impl fmt::Display for NotADecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a Decimal's integer is an optional '-' and ASCII digits, and not '{}'",
            self.0
        )
    }
}

impl std::error::Error for NotADecimal {}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(unscaled: &str, scale: i32) -> Decimal {
        Decimal::new(unscaled, scale).unwrap()
    }

    #[test]
    fn two_of_one_amount_are_equal_whatever_their_scales() {
        assert_eq!(of("15", 1), of("150", 2));
        assert_eq!(of("0", 5), of("-0", -3));
        assert_eq!(of("1", 2), of("10", 3));
        assert_eq!(of("12", -1), of("120", 0));
        assert_ne!(of("15", 1), of("15", 2));
        assert_eq!(of("150", 2).scale(), 2);
        let mut set = std::collections::HashSet::new();
        set.insert(of("15", 1));
        assert!(set.contains(&of("1500", 3)));
    }

    #[test]
    fn decimals_are_ordered_by_amount() {
        let ordered = [
            of("-2", 0),
            of("-15", 1),
            of("-100", 2),
            of("0", 7),
            of("1", 3),
            of("99", 3),
            of("1", 1),
            of("15", 1),
            of("16", 1),
            of("2", -1),
        ];
        for pair in ordered.windows(2) {
            assert!(pair[0] < pair[1], "{:?} < {:?}", pair[0], pair[1]);
        }
        assert_eq!(of("15", 1).cmp(&of("150", 2)), Ordering::Equal);
    }
}
