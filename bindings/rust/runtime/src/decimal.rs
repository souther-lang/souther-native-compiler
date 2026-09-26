//! A Souther `Decimal` as Rust holds one.

use std::fmt;

/// A Souther `Decimal`: its integer and its scale, the amount being the integer over ten to the
/// scale.
///
/// The two numbers the language says a `Decimal` is, and nothing else, as the PHP runtime holds
/// one. What a host does with them — a decimal crate, a money type, text — is the host's. The scale
/// is kept as it was: `1.50` is `150` at scale 2 and `1.5` is `15` at scale 1, two values of one
/// amount, as they are in Souther, and not equal here either.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
