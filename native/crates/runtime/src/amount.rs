//! What a `Decimal` is, and what each operation on one answers (spec §primitives,
//! §stdlib-decimal).
//!
//! A `Decimal` is an integer and a scale: the amount is the integer over ten to the scale, and the
//! scale is kept as it was written, so `1.0` and `1.00` are one amount and two values. Equality and
//! order read the amount; the text a value is written as reads the scale too. Every operation here
//! answers what the language states for it, which is what the JVM's `DecimalMath` answers, and says
//! where it answers nothing so that its caller can end the run for the reason its contract names.
//!
//! The integer is worked on with `num_bigint`, and nowhere outside this file. What that crate is
//! asked for is integer arithmetic and nothing else: which scale a result has, how it is rounded,
//! what a value is written as and where an operation refuses are all written here. So the crate
//! could be swapped for another without a single answer moving, and nothing that reads a `Decimal`
//! — the arena, generated code, a host — ever sees one of its types.
//!
//! No operation here builds a power of ten from a scale it was handed. A scale is a 32-bit number,
//! and `10^2147483647` is a number no memory holds, so an operation whose answer is small but whose
//! operands differ wildly in scale — rounding `1E-2000000000` to a whole number — decides that
//! answer from how many digits there are, and one whose answer is itself that wide refuses before
//! building it ([`WIDEST`]).

use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};
use souther_text::DecimalText;
use std::cmp::Ordering;

/// How many bits the integer of a `Decimal` holds at most.
///
/// The language states the range of a scale and not of the integer, and a representation has to
/// stop somewhere: this is where the JVM's `BigInteger` stops, so that a result one carrier holds is
/// one the other holds too. A result past it has no place, and the operation that would have built
/// it refuses (spec §an-operation-refuses-only-what-its-own-answer-has-no-place-for).
const WIDEST: u64 = i32::MAX as u64;

/// How many digits the text an amount is written as at a boundary may spell an exponent out into
/// (spec §primitives): `1E+1000` is written with its thousand zeros and `1E+1001` as it stands.
const SPELT_OUT: i64 = 1000;

/// A `Decimal`: a sign, the digits as a whole number, and a scale.
///
/// Nought has no sign, so there is one way to hold each value: `-0.0` is read as `0.0`, as the JVM
/// reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Amount {
    negative: bool,
    magnitude: BigUint,
    scale: i32,
}

/// A rounding mode, as `RoundingMode` declares its cases (spec §stdlib-decimal). Each says which of
/// the two whole numbers of the scale's grid either side of a value it answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Rounding {
    HalfUp,
    HalfEven,
    HalfDown,
    Up,
    Down,
    Ceiling,
    Floor,
}

/// What rounding dropped, measured against half of the unit it rounded to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dropped {
    Nothing,
    BelowHalf,
    Half,
    AboveHalf,
}

impl Rounding {
    /// Whether a magnitude rounded towards nought is taken one further from nought instead.
    fn away(self, negative: bool, odd: bool, dropped: Dropped) -> bool {
        if dropped == Dropped::Nothing {
            return false;
        }
        match self {
            Rounding::Up => true,
            Rounding::Down => false,
            Rounding::Ceiling => !negative,
            Rounding::Floor => negative,
            Rounding::HalfUp => dropped >= Dropped::Half,
            Rounding::HalfDown => dropped == Dropped::AboveHalf,
            Rounding::HalfEven => {
                dropped == Dropped::AboveHalf || (dropped == Dropped::Half && odd)
            }
        }
    }
}

impl PartialOrd for Dropped {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dropped {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

/// `log2(10)`, for how many bits a power of ten is wide.
const LOG2_10: f64 = std::f64::consts::LOG2_10;

/// Ten to each power a `u128` holds, from nought to 38: what most amounts are measured and brought
/// to one scale against, read here rather than worked out by `pow` each time.
const TENS: [u128; 39] = {
    let mut tens = [1u128; 39];
    let mut at = 1;
    while at < tens.len() {
        tens[at] = tens[at - 1] * 10;
        at += 1;
    }
    tens
};

/// Ten to `n`, for an `n` the caller has already held to a width a value may have.
fn ten_to(n: u64) -> BigUint {
    if let Some(small) = TENS.get(n as usize) {
        return BigUint::from(*small);
    }
    let n = u32::try_from(n).expect("a power of ten built here is one a value may be as wide as");
    BigUint::from(10u8).pow(n)
}

/// The magnitude, where it is no wider than a `Decimal` holds.
fn held(magnitude: BigUint) -> Option<BigUint> {
    (magnitude.bits() <= WIDEST).then_some(magnitude)
}

/// The magnitude times ten to `by`, where that is no wider than a `Decimal` holds. Refused before
/// it is built where it could not be: the product is at least as wide as its factors' widths less
/// one, and a power of ten is `by · log2 10` bits wide.
fn scaled_up(magnitude: &BigUint, by: u64) -> Option<BigUint> {
    if magnitude.is_zero() || by == 0 {
        return Some(magnitude.clone());
    }
    let narrowest = (magnitude.bits() - 1) as f64 + (by as f64) * LOG2_10 - 1.0;
    if narrowest > WIDEST as f64 {
        return None;
    }
    held(magnitude * ten_to(by))
}

/// How many decimal digits the magnitude has; one for nought, as the JVM counts it.
///
/// Read off how many bits it has, which gives it to within one, and settled against a power of ten
/// no wider than the magnitude itself.
fn precision(magnitude: &BigUint) -> u64 {
    if let Some(small) = magnitude.to_u128() {
        // How many of the powers are no greater than it, which is its digits; nought has one.
        return TENS.partition_point(|&ten| ten <= small).max(1) as u64;
    }
    let mut digits = ((magnitude.bits() - 1) as f64 * std::f64::consts::LOG10_2) as u64 + 1;
    while digits > 1 && *magnitude < ten_to(digits - 1) {
        digits -= 1;
    }
    while *magnitude >= ten_to(digits) {
        digits += 1;
    }
    digits
}

/// What a remainder is, against half the divisor it was left by.
fn dropped(remainder: &BigUint, divisor: &BigUint) -> Dropped {
    if remainder.is_zero() {
        return Dropped::Nothing;
    }
    match (remainder << 1u8).cmp(divisor) {
        Ordering::Less => Dropped::BelowHalf,
        Ordering::Equal => Dropped::Half,
        Ordering::Greater => Dropped::AboveHalf,
    }
}

/// The quotient, rounded by `mode` from what the division dropped.
fn rounded(quotient: BigUint, negative: bool, dropped: Dropped, mode: Rounding) -> BigUint {
    let odd = quotient.is_odd();
    if mode.away(negative, odd, dropped) {
        quotient + 1u8
    } else {
        quotient
    }
}

/// The digits the magnitude is written in, in decimal.
fn digits(magnitude: &BigUint) -> String {
    magnitude.to_str_radix(10)
}

impl Amount {
    fn new(negative: bool, magnitude: BigUint, scale: i32) -> Amount {
        Amount {
            negative: negative && !magnitude.is_zero(),
            magnitude,
            scale,
        }
    }

    /// The value these parts are: a sign, the magnitude as little-endian bytes, and the scale.
    pub(crate) fn of_parts(negative: bool, magnitude: &[u8], scale: i32) -> Amount {
        Amount::new(negative, BigUint::from_bytes_le(magnitude), scale)
    }

    /// The parts [`Amount::of_parts`] takes: the magnitude as little-endian bytes, with no zero
    /// byte at the top and none at all for nought.
    pub(crate) fn parts(&self) -> (bool, Vec<u8>, i32) {
        let bytes = if self.magnitude.is_zero() {
            Vec::new()
        } else {
            self.magnitude.to_bytes_le()
        };
        (self.negative, bytes, self.scale)
    }

    /// `Decimal.fromInt`: the same number, at scale nought. Every `Int` is one exactly.
    pub(crate) fn of_int(value: i64) -> Amount {
        Amount::new(value < 0, BigUint::from(value.unsigned_abs()), 0)
    }

    /// The whole number these ASCII digits write in decimal, at `scale`.
    fn of_digits(negative: bool, digits: &[u8], scale: i32) -> Amount {
        let magnitude = if digits.is_empty() {
            BigUint::zero()
        } else {
            BigUint::parse_bytes(digits, 10).expect("the digits read are ASCII digits")
        };
        Amount::new(negative, magnitude, scale)
    }

    /// The value decimal text writes, at the scale its fractional digits give it
    /// (`String.toDecimal`).
    ///
    /// Always one: a scale is as many digits as the text has after its point, and a string is
    /// shorter than the largest scale.
    pub(crate) fn of_decimal_text(text: DecimalText) -> Amount {
        let scale = i32::try_from(text.fraction.len())
            .expect("a string holds fewer digits than the largest scale");
        let mut digits = Vec::with_capacity(text.whole.len() + text.fraction.len());
        digits.extend_from_slice(text.whole);
        digits.extend_from_slice(text.fraction);
        Amount::of_digits(text.negative, &digits, scale)
    }

    /// The whole number integer text writes, at `scale`: what a host hands over as a value's
    /// integer and its scale. Nothing where the text writes no integer.
    pub(crate) fn of_integer_text(text: DecimalText, scale: i32) -> Option<Amount> {
        text.fraction
            .is_empty()
            .then(|| Amount::of_digits(text.negative, text.whole, scale))
    }

    /// The value a JSON number writes, at the scale its spelling gives it: as many places as its
    /// fraction has, less its exponent, which is how the JVM reads one (`new BigDecimal(text)`).
    /// Nothing where that scale is not one a `Decimal` has.
    ///
    /// `written` is a number as RFC 8259 writes one, which the document's reader has held it to.
    pub(crate) fn of_json_number(written: &[u8]) -> Option<Amount> {
        let (negative, unsigned) = match written.split_first() {
            Some((b'-', rest)) => (true, rest),
            _ => (false, written),
        };
        let (mantissa, exponent) = match unsigned.iter().position(|&it| it == b'e' || it == b'E') {
            Some(at) => (&unsigned[..at], Some(&unsigned[at + 1..])),
            None => (unsigned, None),
        };
        let (whole, fraction) = match mantissa.iter().position(|&it| it == b'.') {
            Some(at) => (&mantissa[..at], &mantissa[at + 1..]),
            None => (mantissa, &[][..]),
        };
        let exponent = match exponent {
            None => 0,
            Some(spelt) => {
                let (below, digits) = match spelt.split_first() {
                    Some((b'-', rest)) => (true, rest),
                    Some((b'+', rest)) => (false, rest),
                    _ => (false, spelt),
                };
                // An exponent wider than sixty-four bits gives a scale no `Decimal` has, and
                // one within them is settled by the scale below.
                let mut value: i64 = 0;
                for &digit in digits {
                    value = value
                        .checked_mul(10)?
                        .checked_add(i64::from(digit - b'0'))?;
                }
                if below { -value } else { value }
            }
        };
        let fraction_digits = i64::try_from(fraction.len()).ok()?;
        let scale = i32::try_from(fraction_digits.checked_sub(exponent)?).ok()?;
        let mut digits = Vec::with_capacity(whole.len() + fraction.len());
        digits.extend_from_slice(whole);
        digits.extend_from_slice(fraction);
        Some(Amount::of_digits(negative, &digits, scale))
    }

    /// The scale, as the value carries it.
    pub(crate) fn scale(&self) -> i32 {
        self.scale
    }

    /// The integer, in decimal: a `-` where it is below nought, and no leading zero.
    pub(crate) fn unscaled_text(&self) -> String {
        let digits = digits(&self.magnitude);
        if self.negative {
            format!("-{digits}")
        } else {
            digits
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.magnitude.is_zero()
    }

    /// The same amount on the other side of nought, at the same scale. Total.
    pub(crate) fn negated(&self) -> Amount {
        Amount::new(!self.negative, self.magnitude.clone(), self.scale)
    }

    /// Two values by amount, whatever their scales (`Decimal.compare`, `==` and `<`). Total.
    ///
    /// By sign first. Two values at one scale are then their magnitudes compared, and two whose
    /// magnitudes a `u128` holds are brought to one scale in one: those are nearly every pair a
    /// sort or a comparison of amounts is handed, and neither needs a digit counted. Past them, by
    /// where each value's leading digit stands; only two values whose leading digits stand at one
    /// place are brought to one scale, and then the one with the larger scale is no wider than the
    /// other already is.
    pub(crate) fn compare(&self, other: &Amount) -> Ordering {
        let sign = |it: &Amount| match (it.is_zero(), it.negative) {
            (true, _) => 0,
            (false, true) => -1,
            (false, false) => 1,
        };
        let by_sign = sign(self).cmp(&sign(other));
        if by_sign != Ordering::Equal || self.is_zero() {
            return by_sign;
        }
        let by_magnitude = if self.scale == other.scale {
            self.magnitude.cmp(&other.magnitude)
        } else if let (Some(mine), Some(theirs)) =
            (self.magnitude.to_u128(), other.magnitude.to_u128())
        {
            // The one at the smaller scale raised to the other's. Neither is nought here, so one
            // raised past what a `u128` holds is the greater.
            let apart = i64::from(self.scale) - i64::from(other.scale);
            let raised = |magnitude: u128, by: i64| {
                TENS.get(by as usize)
                    .and_then(|ten| magnitude.checked_mul(*ten))
            };
            if apart < 0 {
                raised(mine, -apart).map_or(Ordering::Greater, |it| it.cmp(&theirs))
            } else {
                raised(theirs, apart).map_or(Ordering::Less, |it| mine.cmp(&it))
            }
        } else {
            self.compare_wide(other)
        };
        if self.negative {
            by_magnitude.reverse()
        } else {
            by_magnitude
        }
    }

    /// The magnitudes of two values of one sign compared by amount, where either is wider than a
    /// `u128`: by where each value's leading digit stands, and brought to one scale only where
    /// those stand at one place.
    fn compare_wide(&self, other: &Amount) -> Ordering {
        let leading = |it: &Amount| precision(&it.magnitude) as i64 - i64::from(it.scale);
        match leading(self).cmp(&leading(other)) {
            Ordering::Equal => {
                let apart = i64::from(self.scale) - i64::from(other.scale);
                let raised = |magnitude: &BigUint, by: i64| magnitude * ten_to(by as u64);
                match apart.cmp(&0) {
                    Ordering::Less => raised(&self.magnitude, -apart).cmp(&other.magnitude),
                    Ordering::Equal => self.magnitude.cmp(&other.magnitude),
                    Ordering::Greater => self.magnitude.cmp(&raised(&other.magnitude, apart)),
                }
            }
            unequal => unequal,
        }
    }

    /// `+` and `Decimal.add`: at the larger of the two scales. Nothing where the sum is wider than
    /// a `Decimal` holds.
    pub(crate) fn add(&self, other: &Amount) -> Option<Amount> {
        let scale = self.scale.max(other.scale);
        let raised = |it: &Amount| {
            scaled_up(
                &it.magnitude,
                (i64::from(scale) - i64::from(it.scale)) as u64,
            )
        };
        let (mine, theirs) = (raised(self)?, raised(other)?);
        let (negative, magnitude) = if self.negative == other.negative {
            (self.negative, mine + theirs)
        } else if mine >= theirs {
            (self.negative, mine - theirs)
        } else {
            (other.negative, theirs - mine)
        };
        Some(Amount::new(negative, held(magnitude)?, scale))
    }

    /// `-` and `Decimal.subtract`, as [`Amount::add`] of the negation.
    pub(crate) fn subtract(&self, other: &Amount) -> Option<Amount> {
        self.add(&other.negated())
    }

    /// `*` and `Decimal.multiply`: at the sum of the two scales. Nothing where that sum is not a
    /// scale, or the product is wider than a `Decimal` holds.
    pub(crate) fn multiply(&self, other: &Amount) -> Option<Amount> {
        let scale = i32::try_from(i64::from(self.scale) + i64::from(other.scale)).ok()?;
        if self.magnitude.bits() + other.magnitude.bits() > WIDEST + 1 {
            return None;
        }
        let magnitude = held(&self.magnitude * &other.magnitude)?;
        Some(Amount::new(
            self.negative != other.negative,
            magnitude,
            scale,
        ))
    }

    /// The value at `scale`, rounded by `mode` where places are dropped (`Decimal.round`, and
    /// `Decimal.toInt` at scale nought). Nothing where the scale is not one a `Decimal` has, or the
    /// value at it is wider than a `Decimal` holds.
    pub(crate) fn round(&self, scale: i64, mode: Rounding) -> Option<Amount> {
        let scale = i32::try_from(scale).ok()?;
        if scale >= self.scale {
            let by = (i64::from(scale) - i64::from(self.scale)) as u64;
            return Some(Amount::new(
                self.negative,
                scaled_up(&self.magnitude, by)?,
                scale,
            ));
        }
        let dropping = (i64::from(self.scale) - i64::from(scale)) as u64;
        let (quotient, dropped) = self.dropping(dropping);
        let magnitude = held(rounded(quotient, self.negative, dropped, mode))?;
        Some(Amount::new(self.negative, magnitude, scale))
    }

    /// The magnitude with its last `places` digits dropped, and what was dropped.
    ///
    /// A magnitude with fewer digits than `places` less one is below a tenth of the unit it is
    /// rounded to, so the answer is nought and what was dropped is below half, without the power
    /// of ten the division would have wanted.
    fn dropping(&self, places: u64) -> (BigUint, Dropped) {
        if self.magnitude.is_zero() {
            return (BigUint::zero(), Dropped::Nothing);
        }
        if places > precision(&self.magnitude) {
            return (BigUint::zero(), Dropped::BelowHalf);
        }
        let unit = ten_to(places);
        let (quotient, remainder) = self.magnitude.div_rem(&unit);
        let dropped = dropped(&remainder, &unit);
        (quotient, dropped)
    }

    /// `Decimal.toInt`: the whole number the value rounds to by `mode`. Nothing where that is not
    /// an `Int`.
    pub(crate) fn to_int(&self, mode: Rounding) -> Option<i64> {
        if self.magnitude.is_zero() {
            return Some(0);
        }
        let whole = if self.scale <= 0 {
            // A whole number already, and one with more than nineteen digits is past every `Int`.
            let by = -i64::from(self.scale) as u64;
            if precision(&self.magnitude) + by > 19 {
                return None;
            }
            &self.magnitude * ten_to(by)
        } else {
            let (quotient, dropped) = self.dropping(self.scale as u64);
            rounded(quotient, self.negative, dropped, mode)
        };
        let magnitude = whole.to_u64()?;
        if self.negative {
            0i64.checked_sub_unsigned(magnitude)
        } else {
            i64::try_from(magnitude).ok()
        }
    }

    /// `Decimal.divide`: the quotient at `scale`, rounded by `mode`. Nothing where the scale is
    /// not one a `Decimal` has, or the quotient at it is wider than a `Decimal` holds.
    ///
    /// # Panics
    ///
    /// Where the divisor is nought, which is answered as a case before any division is asked for
    /// (spec §a-division-that-does-not-run-needs-no-scale).
    pub(crate) fn divide(&self, divisor: &Amount, scale: i64, mode: Rounding) -> Option<Amount> {
        assert!(
            !divisor.is_zero(),
            "a zero divisor is answered as a case and never divided by"
        );
        let scale = i32::try_from(scale).ok()?;
        let negative = self.negative != divisor.negative;
        if self.is_zero() {
            return Some(Amount::new(false, BigUint::zero(), scale));
        }
        // The quotient at `scale` is `self · 10^raise / divisor`, the power of ten on whichever
        // side keeps it whole.
        let raise = i64::from(scale) - i64::from(self.scale) + i64::from(divisor.scale);
        let (quotient, dropped) = if raise >= 0 {
            // As wide as the quotient and the divisor together, and refused where the quotient
            // alone would be wider than a `Decimal` holds.
            let narrowest = (self.magnitude.bits() - 1) as f64 + raise as f64 * LOG2_10
                - divisor.magnitude.bits() as f64
                - 1.0;
            if narrowest > WIDEST as f64 {
                return None;
            }
            let dividend = &self.magnitude * ten_to(raise as u64);
            let (quotient, remainder) = dividend.div_rem(&divisor.magnitude);
            (quotient, dropped(&remainder, &divisor.magnitude))
        } else {
            let lowered = (-raise) as u64;
            // Below a tenth where the divisor, raised, has two more digits than the dividend.
            let apart = precision(&self.magnitude) as i64 - precision(&divisor.magnitude) as i64;
            if lowered as i64 >= apart + 2 {
                (BigUint::zero(), Dropped::BelowHalf)
            } else {
                let by = &divisor.magnitude * ten_to(lowered);
                let (quotient, remainder) = self.magnitude.div_rem(&by);
                (quotient, dropped(&remainder, &by))
            }
        };
        let magnitude = held(rounded(quotient, negative, dropped, mode))?;
        Some(Amount::new(negative, magnitude, scale))
    }

    /// How many characters [`Amount::plain_text`] is, worked out without writing it: a sign, the
    /// digits, and either the whole zeros a negative scale stands for or a point and the leading
    /// fractional zeros a scale above the digits asks for. Nought is `0` at every scale up to
    /// nought.
    fn plain_length(&self) -> i64 {
        let sign = i64::from(self.negative);
        let precision = precision(&self.magnitude) as i64;
        let scale = i64::from(self.scale);
        if scale <= 0 {
            if self.is_zero() {
                1
            } else {
                sign + precision - scale
            }
        } else if precision > scale {
            sign + precision + 1
        } else {
            sign + 2 + scale
        }
    }

    /// `String.fromDecimal`: the value in plain notation, never with an exponent, at the scale it
    /// carries (spec §stdlib-string). Nothing where that text is longer than a string holds, which
    /// is decided before any of it is written: the text of a value near either end of the scale
    /// range is a couple of billion characters.
    pub(crate) fn plain_text(&self) -> Option<String> {
        if self.plain_length() > souther_text::MOST {
            return None;
        }
        let digits = digits(&self.magnitude);
        let sign = if self.negative { "-" } else { "" };
        let scale = i64::from(self.scale);
        Some(if scale <= 0 {
            if self.is_zero() {
                "0".to_string()
            } else {
                format!("{sign}{digits}{}", "0".repeat((-scale) as usize))
            }
        } else if digits.len() as i64 > scale {
            let point = digits.len() - scale as usize;
            format!("{sign}{}.{}", &digits[..point], &digits[point..])
        } else {
            let zeros = "0".repeat(scale as usize - digits.len());
            format!("{sign}0.{zeros}{digits}")
        })
    }

    /// The amount with as many of its trailing zeros dropped as the scale lets it drop: one form
    /// for every value the language calls equal (spec §primitives), as the JVM's `leastDigits`
    /// answers it. The scale stops at its smallest, and fixing the scale fixes the digits.
    fn least_digits(&self) -> Amount {
        if self.is_zero() {
            return Amount::new(false, BigUint::zero(), 0);
        }
        let room = (i64::from(self.scale) - i64::from(i32::MIN)) as u64;
        let mut magnitude = self.magnitude.clone();
        let mut dropped = 0u64;
        // Nineteen at a time while they are there, then one at a time.
        let chunk = BigUint::from(10_000_000_000_000_000_000u64);
        while dropped + 19 <= room {
            let (quotient, remainder) = magnitude.div_rem(&chunk);
            if !remainder.is_zero() {
                break;
            }
            magnitude = quotient;
            dropped += 19;
        }
        let ten = BigUint::from(10u8);
        while dropped < room {
            let (quotient, remainder) = magnitude.div_rem(&ten);
            if !remainder.is_zero() {
                break;
            }
            magnitude = quotient;
            dropped += 1;
        }
        let scale = (i64::from(self.scale) - dropped as i64) as i32;
        Amount::new(self.negative, magnitude, scale)
    }

    /// The text the value is written as at a boundary: its amount, and not the scale it was read
    /// or worked out at (spec §primitives, the JVM's `Representations.canonicalNumber`).
    ///
    /// The fewest digits the amount is written with, spelt out into its whole zeros where that is
    /// at most [`SPELT_OUT`] digits and left with its exponent where it is more; and then written as
    /// a JSON number the way the JVM's `BigDecimal.toString` writes one, which is how the JVM's
    /// boundary writes it.
    pub(crate) fn external_text(&self) -> String {
        let least = self.least_digits();
        let spelt = if least.scale < 0
            && precision(&least.magnitude) as i64 - i64::from(least.scale) <= SPELT_OUT
        {
            let by = (-i64::from(least.scale)) as u64;
            Amount::new(least.negative, &least.magnitude * ten_to(by), 0)
        } else {
            least
        };
        spelt.scientific_text()
    }

    /// The value as the JVM's `BigDecimal.toString` writes it: plain where the scale is not below
    /// nought and the leading digit stands no more than six places after the point, and otherwise
    /// one digit, the rest after a point, and the exponent.
    fn scientific_text(&self) -> String {
        let digits = digits(&self.magnitude);
        let sign = if self.negative { "-" } else { "" };
        if self.scale == 0 {
            return format!("{sign}{digits}");
        }
        let scale = i64::from(self.scale);
        let adjusted = -scale + (digits.len() as i64 - 1);
        if scale >= 0 && adjusted >= -6 {
            let point = digits.len() as i64 - scale;
            return if point > 0 {
                let point = point as usize;
                format!("{sign}{}.{}", &digits[..point], &digits[point..])
            } else {
                format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
            };
        }
        let (first, rest) = digits.split_at(1);
        let mut written = format!("{sign}{first}");
        if !rest.is_empty() {
            written.push('.');
            written.push_str(rest);
        }
        if adjusted != 0 {
            written.push('E');
            if adjusted > 0 {
                written.push('+');
            }
            written.push_str(&adjusted.to_string());
        }
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use souther_text::{Text, decimal_text};

    /// A value as the JVM's `new BigDecimal(text)` reads decimal text or a JSON number.
    fn d(text: &str) -> Amount {
        Amount::of_json_number(text.as_bytes()).expect("a value the tests write")
    }

    fn shown(amount: &Amount) -> (String, i32) {
        (amount.unscaled_text(), amount.scale())
    }

    const EVERY: [Rounding; 7] = [
        Rounding::Up,
        Rounding::Down,
        Rounding::Ceiling,
        Rounding::Floor,
        Rounding::HalfUp,
        Rounding::HalfDown,
        Rounding::HalfEven,
    ];

    /// A value's integer and scale are what its spelling gives it, `-0` is nought, and an
    /// exponent moves the scale.
    #[test]
    fn a_json_number_is_read_at_the_scale_its_spelling_gives_it() {
        for (text, unscaled, scale) in [
            ("0", "0", 0),
            ("-0", "0", 0),
            ("-0.00", "0", 2),
            ("1.50", "150", 2),
            ("-12.345", "-12345", 3),
            ("1e2", "1", -2),
            ("1.5E+3", "15", -2),
            ("25e-3", "25", 3),
            (
                "123456789012345678901234567890",
                "123456789012345678901234567890",
                0,
            ),
        ] {
            assert_eq!(shown(&d(text)), (unscaled.to_string(), scale), "{text}");
        }
        assert!(Amount::of_json_number(b"1e2147483648").is_some());
        assert!(Amount::of_json_number(b"1e2147483649").is_none());
        assert!(Amount::of_json_number(b"1e-2147483647").is_some());
        assert!(Amount::of_json_number(b"1e-2147483648").is_none());
        assert!(Amount::of_json_number(b"1e99999999999999999999").is_none());
    }

    /// Scale is carried and not compared: `1.0` and `1.00` are one amount.
    #[test]
    fn values_are_compared_by_amount_whatever_their_scales() {
        for (one, other, order) in [
            ("1.0", "1.00", Ordering::Equal),
            ("0", "0.000", Ordering::Equal),
            ("-0.0", "0", Ordering::Equal),
            ("1", "1.01", Ordering::Less),
            ("-1", "-1.01", Ordering::Greater),
            ("-1", "1", Ordering::Less),
            ("1e2", "99.99", Ordering::Greater),
            ("1e-2000000000", "0", Ordering::Greater),
            ("1e2000000000", "1e1999999999", Ordering::Greater),
            ("12.30", "1.23e1", Ordering::Equal),
            ("1.50", "1.49", Ordering::Greater),
            (
                "1",
                "1.00000000000000000000000000000000000000",
                Ordering::Equal,
            ),
            (
                "1",
                "1.000000000000000000000000000000000000001",
                Ordering::Less,
            ),
            (
                "2",
                "1.999999999999999999999999999999999999999",
                Ordering::Greater,
            ),
            (
                "340282366920938463463374607431768211455",
                "3.4e38",
                Ordering::Greater,
            ),
            (
                "340282366920938463463374607431768211456",
                "340282366920938463463374607431768211455.9",
                Ordering::Greater,
            ),
            (
                "-7e40",
                "-70000000000000000000000000000000000000000.0",
                Ordering::Equal,
            ),
        ] {
            assert_eq!(d(one).compare(&d(other)), order, "{one} {other}");
            assert_eq!(d(other).compare(&d(one)), order.reverse(), "{other} {one}");
        }
    }

    /// A sum and a difference are at the larger scale, a product at the sum of the two; negation
    /// keeps the scale.
    #[test]
    fn arithmetic_answers_the_scale_the_language_states() {
        let sum = d("1.5").add(&d("2.25")).unwrap();
        assert_eq!(shown(&sum), ("375".to_string(), 2));
        let difference = d("1").subtract(&d("0.001")).unwrap();
        assert_eq!(shown(&difference), ("999".to_string(), 3));
        let product = d("0.10").multiply(&d("100.0")).unwrap();
        assert_eq!(shown(&product), ("10000".to_string(), 3));
        assert_eq!(shown(&d("1.50").negated()), ("-150".to_string(), 2));
        assert_eq!(shown(&d("0.00").negated()), ("0".to_string(), 2));
        let across = d("-5").add(&d("3.5")).unwrap();
        assert_eq!(shown(&across), ("-15".to_string(), 1));
        let nothing = d("2.5").subtract(&d("2.50")).unwrap();
        assert_eq!(shown(&nothing), ("0".to_string(), 2));
    }

    /// A product whose scale is past the range aborts, and so does a sum that would have to be
    /// spelt out past what a `Decimal` holds; nought raised to any scale is still nought.
    #[test]
    fn an_answer_with_no_place_is_nothing() {
        assert!(d("1e-2147483647").multiply(&d("1e-1")).is_none());
        assert!(d("1e2147483647").multiply(&d("1e2")).is_none());
        let floor = d("1e2147483647").multiply(&d("1e1")).unwrap();
        assert_eq!(floor.scale(), i32::MIN);
        assert!(d("0e-2147483647").multiply(&d("1e-1")).is_none());
        assert!(d("1e-2147483647").add(&d("1e2147483647")).is_none());
        let nought = d("0e2147483647").add(&d("1e-2147483647")).unwrap();
        assert_eq!(shown(&nought), ("1".to_string(), 2147483647));
    }

    /// Every mode against the rows `java.math.RoundingMode`'s own documentation tabulates.
    #[test]
    fn every_mode_rounds_as_the_jvm_tabulates() {
        use Rounding::*;
        let table: [(&str, [i64; 7]); 10] = [
            ("5.5", [6, 5, 6, 5, 6, 5, 6]),
            ("2.5", [3, 2, 3, 2, 3, 2, 2]),
            ("1.6", [2, 1, 2, 1, 2, 2, 2]),
            ("1.1", [2, 1, 2, 1, 1, 1, 1]),
            ("1.0", [1, 1, 1, 1, 1, 1, 1]),
            ("-1.0", [-1, -1, -1, -1, -1, -1, -1]),
            ("-1.1", [-2, -1, -1, -2, -1, -1, -1]),
            ("-1.6", [-2, -1, -1, -2, -2, -2, -2]),
            ("-2.5", [-3, -2, -2, -3, -3, -2, -2]),
            ("-5.5", [-6, -5, -5, -6, -6, -5, -6]),
        ];
        for (value, answers) in table {
            for (mode, answer) in [Up, Down, Ceiling, Floor, HalfUp, HalfDown, HalfEven]
                .into_iter()
                .zip(answers)
            {
                assert_eq!(d(value).to_int(mode), Some(answer), "{value} {mode:?}");
                let at_nought = d(value).round(0, mode).unwrap();
                assert_eq!(
                    shown(&at_nought),
                    (answer.to_string(), 0),
                    "{value} {mode:?}"
                );
            }
        }
    }

    /// A value far below the unit it is rounded to is decided by its sign and the mode, without the
    /// power of ten its scale names.
    #[test]
    fn a_value_far_below_the_unit_is_rounded_without_building_its_scale() {
        for mode in EVERY {
            let away = matches!(mode, Rounding::Up | Rounding::Ceiling);
            assert_eq!(d("1e-2000000000").to_int(mode), Some(i64::from(away)));
            let below = matches!(mode, Rounding::Up | Rounding::Floor);
            assert_eq!(d("-1e-2000000000").to_int(mode), Some(-i64::from(below)));
            let rounded = d("7e-2147483647").round(-2147483648, mode).unwrap();
            assert_eq!(rounded.scale(), i32::MIN);
        }
    }

    #[test]
    fn a_whole_number_past_an_int_is_nothing() {
        assert_eq!(
            d("9223372036854775807").to_int(Rounding::Down),
            Some(i64::MAX)
        );
        assert_eq!(
            d("-9223372036854775808").to_int(Rounding::Down),
            Some(i64::MIN)
        );
        assert_eq!(
            d("9223372036854775807.5").to_int(Rounding::Down),
            Some(i64::MAX)
        );
        assert_eq!(d("9223372036854775807.5").to_int(Rounding::Up), None);
        assert_eq!(d("-9223372036854775808.5").to_int(Rounding::Up), None);
        assert_eq!(d("1e19").to_int(Rounding::Down), None);
        assert_eq!(
            d("9e18").to_int(Rounding::Down),
            Some(9_000_000_000_000_000_000)
        );
        assert_eq!(d("1e2000000000").to_int(Rounding::Down), None);
    }

    #[test]
    fn a_scale_outside_the_range_is_nothing() {
        assert!(d("1").round(2147483648, Rounding::Up).is_none());
        assert!(d("1").round(-2147483649, Rounding::Up).is_none());
        assert!(d("1").divide(&d("3"), 2147483648, Rounding::Up).is_none());
        assert!(d("1").round(2147483647, Rounding::Up).is_none());
        let far = d("0").round(2147483647, Rounding::Up).unwrap();
        assert_eq!(far.scale(), i32::MAX);
    }

    /// `Decimal.divide` at the rows `CompileDecimalMathTest` and the specification hold the JVM
    /// to.
    #[test]
    fn a_quotient_is_rounded_at_the_scale_asked() {
        use Rounding::*;
        for (dividend, divisor, scale, mode, unscaled, answered_scale) in [
            ("10", "3", 2, HalfUp, "333", 2),
            ("2", "3", 2, HalfUp, "67", 2),
            ("2", "3", 2, Down, "66", 2),
            ("-2", "3", 2, Floor, "-67", 2),
            ("1", "8", 2, HalfEven, "12", 2),
            ("3", "8", 2, HalfEven, "38", 2),
            ("1", "8", 2, HalfDown, "12", 2),
            ("1", "8", 2, HalfUp, "13", 2),
            ("100", "0.5", 0, HalfUp, "200", 0),
            ("1.00", "4", 1, HalfUp, "3", 1),
            ("12345", "1", -2, HalfUp, "123", -2),
            ("0", "7", 3, HalfUp, "0", 3),
            ("1", "1e10", 2, Up, "1", 2),
            ("1", "1e10", 2, Down, "0", 2),
            ("-1", "1e10", 2, Up, "-1", 2),
            ("1e-5", "1e-2147483647", -2147483642, Down, "1", -2147483642),
        ] {
            let answered = d(dividend)
                .divide(&d(divisor), scale, mode)
                .unwrap_or_else(|| panic!("{dividend} / {divisor}"));
            assert_eq!(
                shown(&answered),
                (unscaled.to_string(), answered_scale),
                "{dividend} / {divisor} at {scale} {mode:?}"
            );
        }
        assert!(d("1").divide(&d("1e-2147483647"), 0, Up).is_none());
    }

    /// Plain notation keeps the scale: trailing zeros after the point, and the whole zeros a
    /// negative scale stands for, never an exponent.
    #[test]
    fn plain_text_is_the_value_at_its_scale() {
        for (value, text) in [
            ("1000.00", "1000.00"),
            ("0.001", "0.001"),
            ("-0.5", "-0.5"),
            ("12e2", "1200"),
            ("0e3", "0"),
            ("0.000", "0.000"),
            ("1e-7", "0.0000001"),
            ("-123", "-123"),
        ] {
            assert_eq!(d(value).plain_text().as_deref(), Some(text), "{value}");
        }
        assert_eq!(d("1e-2147483647").plain_text(), None);
        assert_eq!(d("1e2147483647").plain_text(), None);
        let longest = Amount::of_parts(false, &[1], (souther_text::MOST - 2) as i32);
        assert_eq!(longest.plain_length(), souther_text::MOST);
        let past = Amount::of_parts(false, &[1], (souther_text::MOST - 1) as i32);
        assert_eq!(past.plain_length(), souther_text::MOST + 1);
        assert_eq!(past.plain_text(), None);
    }

    /// Two values of one amount are written one way at a boundary, an exponent is spelt out into a
    /// thousand digits and no more, and the JVM's `toString` decides where a point goes.
    #[test]
    fn a_value_is_written_at_a_boundary_as_its_amount() {
        for (value, written) in [
            ("1.50", "1.5"),
            ("1.5", "1.5"),
            ("100.00", "100"),
            ("1e2", "100"),
            ("0.000", "0"),
            ("-0.0", "0"),
            ("-2.500", "-2.5"),
            ("0.0000001", "1E-7"),
            ("0.000001", "0.000001"),
            ("1.2300e-10", "1.23E-10"),
            ("1e999", &format!("1{}", "0".repeat(999))),
            ("1e1000", "1E+1000"),
            ("10e998", &format!("1{}", "0".repeat(999))),
            ("1e1000000", "1E+1000000"),
            ("123456789e992", "1.23456789E+1000"),
            ("12345678e992", &format!("12345678{}", "0".repeat(992))),
        ] {
            assert_eq!(d(value).external_text(), written, "{value}");
        }
        // The scale stops at its smallest, and the digits it could not drop stay.
        let floor = Amount::of_parts(false, &[10], i32::MIN);
        assert_eq!(floor.least_digits(), floor);
        let above = Amount::of_parts(false, &[100], i32::MIN + 1);
        assert_eq!(above.least_digits(), floor);
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
            let read = Amount::of_decimal_text(decimal_text(Text::held(text)).unwrap());
            assert_eq!(shown(&read), (unscaled.to_string(), scale), "{text}");
        }
    }

    #[test]
    fn parts_are_read_back_as_the_value_they_were_taken_from() {
        for value in [
            "0",
            "-0.00",
            "1",
            "-1",
            "255",
            "256",
            "-123456789012345678901.5",
        ] {
            let (negative, magnitude, scale) = d(value).parts();
            assert_eq!(
                Amount::of_parts(negative, &magnitude, scale),
                d(value),
                "{value}"
            );
            assert_ne!(magnitude.last(), Some(&0), "{value}");
        }
        assert_eq!(shown(&Amount::of_int(i64::MIN)), (i64::MIN.to_string(), 0));
    }

    #[test]
    fn precision_counts_decimal_digits() {
        for (value, digits) in [
            (BigUint::zero(), 1),
            (BigUint::from(9u8), 1),
            (BigUint::from(10u8), 2),
            (BigUint::from(99u8), 2),
            (BigUint::from(u64::MAX), 20),
            (BigUint::from(u128::MAX), 39),
            (ten_to(38), 39),
            (ten_to(38) - 1u8, 38),
            (ten_to(39), 40),
            (ten_to(100), 101),
            (ten_to(100) - 1u8, 100),
        ] {
            assert_eq!(precision(&value), digits, "{value}");
        }
    }
}
