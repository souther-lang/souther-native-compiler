//! The magnitude of a `Decimal`: the whole number its digits write, without a sign.
//!
//! Held in a `u128` wherever it fits, and as a `BigUint` only past that, one form for each value:
//! a `Wide` never holds what a `u128` holds. Nearly every amount a model computes with — money,
//! a rate, a quantity — has fewer than 39 digits, and on those every operation here is machine
//! arithmetic with no allocation; `num_bigint` is asked only where a value, or a result on the way
//! to one, leaves the `u128`. What an operation answers does not depend on which form did the
//! work, and a test holds every operation's `u128` path to the `BigUint` answer for the same
//! operands, across the edge where one form gives way to the other.
//!
//! Nothing here knows a scale, a sign or a rounding mode. It is the integer arithmetic `amount`
//! works a `Decimal`'s operations out with, and nowhere else reads it.

use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};
use std::cmp::Ordering;

/// Ten to each power a `u128` holds, from nought to 38.
pub(crate) const TENS: [u128; 39] = {
    let mut tens = [1u128; 39];
    let mut at = 1;
    while at < tens.len() {
        tens[at] = tens[at - 1] * 10;
        at += 1;
    }
    tens
};

/// A magnitude, in the narrower of the two forms that holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Magnitude {
    Small(u128),
    Wide(BigUint),
}

use Magnitude::{Small, Wide};

/// Ten to `n` as a `BigUint`, for an `n` the caller has already held to a width a value may have.
fn big_ten_to(n: u64) -> BigUint {
    if let Some(small) = TENS.get(n as usize) {
        return BigUint::from(*small);
    }
    let n = u32::try_from(n).expect("a power of ten built here is one a value may be as wide as");
    BigUint::from(10u8).pow(n)
}

impl Magnitude {
    pub(crate) const ZERO: Magnitude = Small(0);

    /// The magnitude a `BigUint` is, in the narrower form.
    pub(crate) fn of_big(big: BigUint) -> Magnitude {
        match big.to_u128() {
            Some(small) => Small(small),
            None => Wide(big),
        }
    }

    /// The same number as a `BigUint`, for the operations only that form answers.
    pub(crate) fn big(&self) -> BigUint {
        match self {
            Small(small) => BigUint::from(*small),
            Wide(big) => big.clone(),
        }
    }

    /// The magnitude these bytes write, little end first.
    pub(crate) fn of_le_bytes(bytes: &[u8]) -> Magnitude {
        if bytes.len() <= 16 {
            let mut word = [0u8; 16];
            word[..bytes.len()].copy_from_slice(bytes);
            return Small(u128::from_le_bytes(word));
        }
        Magnitude::of_big(BigUint::from_bytes_le(bytes))
    }

    /// The magnitude as bytes, little end first and with no zero byte at the top, none at all for
    /// nought, handed to `with` for as long as it runs: a `u128`'s are on the stack.
    pub(crate) fn with_le_bytes<T>(&self, with: impl FnOnce(&[u8]) -> T) -> T {
        match self {
            Small(small) => {
                let bytes = small.to_le_bytes();
                let used = 16 - (small.leading_zeros() / 8) as usize;
                with(&bytes[..used])
            }
            Wide(big) => with(&big.to_bytes_le()),
        }
    }

    /// The whole number these ASCII digits write in decimal, leading zeros and all.
    pub(crate) fn of_digits(digits: &[u8]) -> Magnitude {
        // Thirty-eight digits are below ten to the 38th, which a `u128` holds.
        if digits.len() <= 38 {
            return Small(digits.iter().fold(0u128, |so_far, digit| {
                so_far * 10 + u128::from(digit - b'0')
            }));
        }
        Magnitude::of_big(
            BigUint::parse_bytes(digits, 10).expect("the digits read are ASCII digits"),
        )
    }

    /// The digits it is written in, in decimal, with no leading zero.
    pub(crate) fn digits(&self) -> String {
        match self {
            Small(small) => small.to_string(),
            Wide(big) => big.to_str_radix(10),
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        matches!(self, Small(0))
    }

    pub(crate) fn is_odd(&self) -> bool {
        match self {
            Small(small) => small & 1 == 1,
            Wide(big) => big.is_odd(),
        }
    }

    /// How many bits it is wide.
    pub(crate) fn bits(&self) -> u64 {
        match self {
            Small(small) => u64::from(128 - small.leading_zeros()),
            Wide(big) => big.bits(),
        }
    }

    /// How many decimal digits it has; one for nought, as the JVM counts it.
    ///
    /// A `u128`'s by the powers of ten below it. A wider one's read off how many bits it has,
    /// which gives it to within one, and settled against a power of ten no wider than itself.
    pub(crate) fn precision(&self) -> u64 {
        match self {
            Small(small) => TENS.partition_point(|&ten| ten <= *small).max(1) as u64,
            Wide(big) => {
                let mut digits = ((big.bits() - 1) as f64 * std::f64::consts::LOG10_2) as u64 + 1;
                while digits > 1 && *big < big_ten_to(digits - 1) {
                    digits -= 1;
                }
                while *big >= big_ten_to(digits) {
                    digits += 1;
                }
                digits
            }
        }
    }

    pub(crate) fn add(&self, other: &Magnitude) -> Magnitude {
        if let (Small(one), Small(two)) = (self, other)
            && let Some(sum) = one.checked_add(*two)
        {
            return Small(sum);
        }
        Magnitude::of_big(self.big() + other.big())
    }

    /// `self` less `other`, which is no greater than it.
    pub(crate) fn sub(&self, other: &Magnitude) -> Magnitude {
        match (self, other) {
            (Small(one), Small(two)) => Small(one - two),
            _ => Magnitude::of_big(self.big() - other.big()),
        }
    }

    pub(crate) fn mul(&self, other: &Magnitude) -> Magnitude {
        if let (Small(one), Small(two)) = (self, other)
            && let Some(product) = one.checked_mul(*two)
        {
            return Small(product);
        }
        Magnitude::of_big(self.big() * other.big())
    }

    /// One more.
    pub(crate) fn increment(&self) -> Magnitude {
        self.add(&Small(1))
    }

    /// Times ten to `by`, for a `by` the caller has already held to a width a value may have.
    pub(crate) fn times_ten_to(&self, by: u64) -> Magnitude {
        if self.is_zero() || by == 0 {
            return self.clone();
        }
        if let Small(small) = self
            && let Some(ten) = TENS.get(by as usize)
            && let Some(raised) = small.checked_mul(*ten)
        {
            return Small(raised);
        }
        Magnitude::of_big(self.big() * big_ten_to(by))
    }

    /// The quotient and the remainder of `self` over `divisor`, which is not nought.
    pub(crate) fn div_rem(&self, divisor: &Magnitude) -> (Magnitude, Magnitude) {
        match (self, divisor) {
            (Small(one), Small(two)) => (Small(one / two), Small(one % two)),
            // A dividend a `u128` holds over one it does not is nought, all of it left over.
            (Small(_), Wide(_)) => (Magnitude::ZERO, self.clone()),
            _ => {
                let (quotient, remainder) = self.big().div_rem(&divisor.big());
                (Magnitude::of_big(quotient), Magnitude::of_big(remainder))
            }
        }
    }

    /// Ten to `n`, for an `n` the caller has already held to a width a value may have.
    pub(crate) fn ten_to(n: u64) -> Magnitude {
        match TENS.get(n as usize) {
            Some(small) => Small(*small),
            None => Magnitude::of_big(big_ten_to(n)),
        }
    }

    /// The magnitude with as many of its trailing zero digits dropped as there are and `most`
    /// allows, and how many were.
    pub(crate) fn without_trailing_zeros(&self, most: u64) -> (Magnitude, u64) {
        if self.is_zero() {
            return (self.clone(), 0);
        }
        let mut magnitude = self.clone();
        let mut dropped = 0u64;
        // Nineteen at a time while the value is wide and they are there, then one at a time,
        // which a value a `u128` holds does as machine division.
        let chunk = BigUint::from(10_000_000_000_000_000_000u64);
        while let Wide(big) = &magnitude
            && dropped + 19 <= most
        {
            let (quotient, remainder) = big.div_rem(&chunk);
            if !remainder.is_zero() {
                break;
            }
            magnitude = Magnitude::of_big(quotient);
            dropped += 19;
        }
        let ten = Small(10);
        while dropped < most {
            let (quotient, remainder) = magnitude.div_rem(&ten);
            if !remainder.is_zero() {
                break;
            }
            magnitude = quotient;
            dropped += 1;
        }
        (magnitude, dropped)
    }
}

impl PartialOrd for Magnitude {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Magnitude {
    /// A `Wide` is past every `u128`, since it is only ever what a `u128` does not hold.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Small(one), Small(two)) => one.cmp(two),
            (Small(_), Wide(_)) => Ordering::Less,
            (Wide(_), Small(_)) => Ordering::Greater,
            (Wide(one), Wide(two)) => one.cmp(two),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Operands either side of every edge a `u128` path could get wrong: nought, the ends of a
    /// word and of a `u128`, powers of ten at and past the table, and values well past it.
    fn operands() -> Vec<BigUint> {
        let two = BigUint::from(2u8);
        let mut every = vec![
            BigUint::zero(),
            BigUint::from(1u8),
            BigUint::from(9u8),
            BigUint::from(10u8),
            BigUint::from(12_345u32),
            BigUint::from(u64::MAX),
            BigUint::from(u64::MAX) + 1u8,
            BigUint::from(u128::MAX / 10),
            BigUint::from(u128::MAX) - 1u8,
            BigUint::from(u128::MAX),
            BigUint::from(u128::MAX) + 1u8,
            BigUint::from(u128::MAX) * 3u8,
            two.pow(200),
            two.pow(200) - 1u8,
        ];
        for power in [18, 19, 37, 38, 39, 40, 77] {
            let ten = big_ten_to(power);
            every.push(ten.clone() - 1u8);
            every.push(ten.clone());
            every.push(ten + 7u8);
        }
        every
    }

    /// One form for each value: a `u128`'s is `Small`, and only a wider one is `Wide`.
    fn normal(magnitude: &Magnitude) -> bool {
        match magnitude {
            Small(_) => true,
            Wide(big) => big.to_u128().is_none(),
        }
    }

    fn of(big: &BigUint) -> Magnitude {
        Magnitude::of_big(big.clone())
    }

    /// Every operation answers what the same operation over `BigUint`s answers, whichever form
    /// its operands and its answer are in, and answers it in the narrower form.
    #[test]
    fn every_operation_answers_what_the_big_integer_answers() {
        let every = operands();
        for one in &every {
            let it = of(one);
            assert!(normal(&it), "{one}");
            assert_eq!(it.big(), *one);
            assert_eq!(it.is_zero(), one.is_zero(), "{one}");
            assert_eq!(it.is_odd(), one.is_odd(), "{one}");
            assert_eq!(it.bits(), one.bits(), "{one}");
            assert_eq!(it.digits(), one.to_str_radix(10), "{one}");
            let digits = if one.is_zero() {
                1
            } else {
                one.to_str_radix(10).len() as u64
            };
            assert_eq!(it.precision(), digits, "{one}");
            assert_eq!(
                Magnitude::of_digits(one.to_str_radix(10).as_bytes()),
                it,
                "{one}"
            );
            let bytes = if one.is_zero() {
                Vec::new()
            } else {
                one.to_bytes_le()
            };
            assert_eq!(it.with_le_bytes(<[u8]>::to_vec), bytes, "{one}");
            assert_eq!(Magnitude::of_le_bytes(&bytes), it, "{one}");
            let increment = it.increment();
            assert!(normal(&increment));
            assert_eq!(increment.big(), one + 1u8, "{one}");
            for by in [0, 1, 2, 19, 38, 39, 60] {
                let raised = it.times_ten_to(by);
                assert!(normal(&raised));
                assert_eq!(raised.big(), one * big_ten_to(by), "{one} · 10^{by}");
            }
            let (stripped, dropped) = it.without_trailing_zeros(u64::MAX);
            assert!(normal(&stripped));
            let mut expected = (one.clone(), 0u64);
            while !expected.0.is_zero() && (&expected.0 % 10u8).is_zero() {
                expected = (&expected.0 / 10u8, expected.1 + 1);
            }
            assert_eq!((stripped.big(), dropped), expected, "{one}");
            let (limited, dropped) = it.without_trailing_zeros(1);
            assert!(dropped <= 1);
            assert_eq!(limited.big() * big_ten_to(dropped), *one, "{one}");
            for two in &every {
                let other = of(two);
                assert_eq!(it.cmp(&other), one.cmp(two), "{one} {two}");
                let sum = it.add(&other);
                assert!(normal(&sum));
                assert_eq!(sum.big(), one + two, "{one} + {two}");
                let product = it.mul(&other);
                assert!(normal(&product));
                assert_eq!(product.big(), one * two, "{one} · {two}");
                if one >= two {
                    let difference = it.sub(&other);
                    assert!(normal(&difference));
                    assert_eq!(difference.big(), one - two, "{one} - {two}");
                }
                if !two.is_zero() {
                    let (quotient, remainder) = it.div_rem(&other);
                    assert!(normal(&quotient) && normal(&remainder));
                    assert_eq!(
                        (quotient.big(), remainder.big()),
                        one.div_rem(two),
                        "{one} / {two}"
                    );
                }
            }
        }
        for power in 0..80 {
            assert_eq!(Magnitude::ten_to(power).big(), big_ten_to(power), "{power}");
            assert!(normal(&Magnitude::ten_to(power)));
        }
    }
}
