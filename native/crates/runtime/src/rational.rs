//! A `Rational` as generated code holds one: an address, and behind it a layout only this file
//! reads.
//!
//! A `Rational` has no external form, so no host is ever handed one and none reads behind it: it
//! is made and read by generated code and by this runtime, between which it is an address for the
//! reason a `Decimal` is one. What a value is and what each operation answers is [`Ratio`]'s, and
//! what is here is where one is kept and how an operation that answers nothing says so, as the
//! `Decimal`'s do, by answering whether it wrote its value (`kernels`), so that which reason the
//! run ends for is read off the call and not decided here.
//!
//! The value is `numerator × 2^twos × 5^fives / denominator`, in the one form that has each value
//! once: the numerator and the denominator are coprime and neither is a multiple of two or of
//! five, the denominator is above nought, and nought is `0 / 1` with no exponents. A `Decimal`
//! enters by its scale being negated into the two exponents and nothing being built from it, so a
//! decimal at a scale of a billion is as small here as it is there, and equal values have equal
//! parts.
//!
//! An operation answers nothing where its answer has no place: an exponent past sixty-four bits, a
//! numerator or a denominator wider than a `Decimal`'s integer may be ([`WIDEST`]), or a
//! comparison whose working width is past that.
//!
//! The layout, in the arena and aligned to a slot as everything there is: the power of two in the
//! first slot, the power of five in the second, the sign times how many bytes the numerator is in
//! the third, how many bytes the denominator is in the fourth, and the numerator's bytes and then
//! the denominator's after them, each little end first and with no zero byte at the top.

use crate::amount::{Amount, Dropped, Rounding, WIDEST, dropped, held, rounded};
use crate::decimal::{Decimal, amount, decimal_of, rounding};
use crate::kernels::answered;
use crate::magnitude::Magnitude;
use crate::{Comparison, Count, Value, souther_alloc};
use souther_native_abi::SLOT;
use std::cmp::Ordering;

/// A `Rational`, as the functions here take and answer one: an address, a type of its own for the
/// reason [`crate::Text`] is.
#[repr(C)]
pub struct Rational {
    _opaque: [u8; 0],
}

/// Where the power of two is.
const TWOS: usize = 0;
/// Where the power of five is.
const FIVES: usize = SLOT as usize;
/// Where the sign times the count of the numerator's bytes is.
const SIGNED_NUMERATOR: usize = 2 * SLOT as usize;
/// Where the count of the denominator's bytes is.
const DENOMINATOR_LENGTH: usize = 3 * SLOT as usize;
/// Where the numerator's bytes start.
const BYTES: usize = 4 * SLOT as usize;

/// `log2(5)` to sixteen places, below and above it, for how many bits a power of five is wide
/// without a floating-point number: an exponent is sixty-four bits, and a `f64` is not exact past
/// fifty-three.
const LOG2_5_BELOW: i128 = 23_219_280_948_873_623;
const LOG2_5_ABOVE: i128 = 23_219_280_948_873_624;
const PLACES: i128 = 10_000_000_000_000_000;

/// An exact rational, in the form that has each value once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ratio {
    negative: bool,
    numerator: Magnitude,
    denominator: Magnitude,
    twos: i64,
    fives: i64,
}

fn one() -> Magnitude {
    Magnitude::Small(1)
}

/// `left · right`, where that is no wider than a `Decimal`'s integer may be. Refused before it is
/// built where it could not be: the product is at least as wide as its factors' widths less one.
fn product(left: &Magnitude, right: &Magnitude) -> Option<Magnitude> {
    if left.is_zero() || right.is_zero() {
        return Some(Magnitude::ZERO);
    }
    if left.bits() + right.bits() - 1 > WIDEST {
        return None;
    }
    held(left.mul(right))
}

/// `left / right`, for a `right` that divides it.
fn over(left: &Magnitude, right: &Magnitude) -> Magnitude {
    left.div_rem(right).0
}

/// `whole × 2^twos × 5^fives`, both exponents being at least nought, where that is no wider than a
/// `Decimal`'s integer may be. Asked before it is built, and by the count a factor of five is two
/// bits under, so that nothing a `Decimal` would hold is refused here.
fn written(whole: &Magnitude, twos: i128, fives: i128) -> Option<Magnitude> {
    assert!(
        twos >= 0 && fives >= 0,
        "only a power above nought is written out"
    );
    if whole.is_zero() {
        return Some(Magnitude::ZERO);
    }
    if whole.bits() as i128 + twos + 2 * fives > i128::from(WIDEST) {
        return None;
    }
    Some(whole.times_two_to(twos as u64).times_five_to(fives as u64))
}

/// A sign and a magnitude that are the sum of two of them.
fn signed_sum(left: (bool, &Magnitude), right: (bool, &Magnitude)) -> (bool, Magnitude) {
    if left.0 == right.0 {
        return (left.0, left.1.add(right.1));
    }
    match left.1.cmp(right.1) {
        Ordering::Equal => (false, Magnitude::ZERO),
        Ordering::Greater => (left.0, left.1.sub(right.1)),
        Ordering::Less => (right.0, right.1.sub(left.1)),
    }
}

/// Between which two powers of two `numerator / denominator × 2^twos × 5^fives` stands: its base-2
/// logarithm is above the first and below the second, whatever the exponents are.
fn log_bounds(
    numerator: &Magnitude,
    denominator: &Magnitude,
    twos: i128,
    fives: i128,
) -> (i128, i128) {
    let across = numerator.bits() as i128 - denominator.bits() as i128 + twos;
    let (low, high) = if fives >= 0 {
        (
            fives * LOG2_5_BELOW / PLACES,
            fives * LOG2_5_ABOVE / PLACES + 1,
        )
    } else {
        (
            fives * LOG2_5_ABOVE / PLACES - 1,
            fives * LOG2_5_BELOW / PLACES,
        )
    };
    (across - 1 + low, across + 1 + high)
}

impl Ratio {
    const ZERO: Ratio = Ratio {
        negative: false,
        numerator: Magnitude::ZERO,
        denominator: Magnitude::Small(1),
        twos: 0,
        fives: 0,
    };

    /// The one form of `numerator × 2^twos × 5^fives / denominator`, which is not over nought.
    /// Nothing where an exponent it comes to is past sixty-four bits, or a part is wider than a
    /// `Decimal`'s integer may be.
    fn canonical(
        negative: bool,
        numerator: Magnitude,
        denominator: Magnitude,
        mut twos: i128,
        mut fives: i128,
    ) -> Option<Ratio> {
        assert!(!denominator.is_zero(), "a rational is not over nought");
        if numerator.is_zero() {
            return Some(Ratio::ZERO);
        }
        let common = numerator.gcd(&denominator);
        let (mut numerator, mut denominator) = if common == one() {
            (numerator, denominator)
        } else {
            (over(&numerator, &common), over(&denominator, &common))
        };
        // The two are coprime by now, so a factor of two or of five is on one of them alone.
        let by = numerator.twos();
        if by > 0 {
            numerator = numerator.shifted_down(by);
            twos += i128::from(by);
        }
        let by = denominator.twos();
        if by > 0 {
            denominator = denominator.shifted_down(by);
            twos -= i128::from(by);
        }
        let five = Magnitude::Small(5);
        loop {
            let (quotient, remainder) = numerator.div_rem(&five);
            if !remainder.is_zero() {
                break;
            }
            numerator = quotient;
            fives += 1;
        }
        loop {
            let (quotient, remainder) = denominator.div_rem(&five);
            if !remainder.is_zero() {
                break;
            }
            denominator = quotient;
            fives -= 1;
        }
        Some(Ratio {
            negative,
            numerator: held(numerator)?,
            denominator: held(denominator)?,
            twos: i64::try_from(twos).ok()?,
            fives: i64::try_from(fives).ok()?,
        })
    }

    /// `Rational.fromInt`.
    pub(crate) fn of_int(value: i64) -> Ratio {
        Ratio::canonical(
            value < 0,
            Magnitude::Small(u128::from(value.unsigned_abs())),
            one(),
            0,
            0,
        )
        .expect("every Int has a rational")
    }

    /// `Rational.fromDecimal`: the scale is negated into both exponents and nothing is built from
    /// it.
    pub(crate) fn of_decimal(value: &Amount) -> Ratio {
        let (negative, magnitude, scale) = value.split();
        let exponent = -i128::from(scale);
        Ratio::canonical(negative, magnitude.clone(), one(), exponent, exponent)
            .expect("every Decimal has a rational")
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.numerator.is_zero()
    }

    fn signum(&self) -> i8 {
        if self.is_zero() {
            0
        } else if self.negative {
            -1
        } else {
            1
        }
    }

    /// Whether this is a whole number.
    pub(crate) fn is_whole(&self) -> bool {
        self.denominator == one() && self.twos >= 0 && self.fives >= 0
    }

    /// Whether this has a finite decimal spelling: a denominator with no factor left that ten is
    /// not made of.
    pub(crate) fn has_finite_decimal(&self) -> bool {
        self.denominator == one()
    }

    pub(crate) fn negated(&self) -> Ratio {
        Ratio {
            negative: !self.negative && !self.is_zero(),
            ..self.clone()
        }
    }

    pub(crate) fn multiply(&self, other: &Ratio) -> Option<Ratio> {
        if self.is_zero() || other.is_zero() {
            return Some(Ratio::ZERO);
        }
        let across_one = self.numerator.gcd(&other.denominator);
        let across_two = other.numerator.gcd(&self.denominator);
        Ratio::canonical(
            self.negative != other.negative,
            product(
                &over(&self.numerator, &across_one),
                &over(&other.numerator, &across_two),
            )?,
            product(
                &over(&self.denominator, &across_two),
                &over(&other.denominator, &across_one),
            )?,
            i128::from(self.twos) + i128::from(other.twos),
            i128::from(self.fives) + i128::from(other.fives),
        )
    }

    /// `/`, over a divisor that is not nought.
    pub(crate) fn divide(&self, divisor: &Ratio) -> Option<Ratio> {
        assert!(
            !divisor.is_zero(),
            "a zero divisor is answered as an abort and never divided by"
        );
        if self.is_zero() {
            return Some(Ratio::ZERO);
        }
        let across_one = self.numerator.gcd(&divisor.numerator);
        let across_two = self.denominator.gcd(&divisor.denominator);
        Ratio::canonical(
            self.negative != divisor.negative,
            product(
                &over(&self.numerator, &across_one),
                &over(&divisor.denominator, &across_two),
            )?,
            product(
                &over(&self.denominator, &across_two),
                &over(&divisor.numerator, &across_one),
            )?,
            i128::from(self.twos) - i128::from(divisor.twos),
            i128::from(self.fives) - i128::from(divisor.fives),
        )
    }

    pub(crate) fn add(&self, other: &Ratio) -> Option<Ratio> {
        if self.is_zero() {
            return Some(other.clone());
        }
        if other.is_zero() {
            return Some(self.clone());
        }
        // The lesser of each pair of exponents is common to both terms and stays an exponent. What
        // is left is the distance between them, and that is built: the exact sum is a number with
        // that many digits in it.
        let twos = self.twos.min(other.twos);
        let fives = self.fives.min(other.fives);
        let here = written(
            &self.numerator,
            i128::from(self.twos) - i128::from(twos),
            i128::from(self.fives) - i128::from(fives),
        )?;
        let there = written(
            &other.numerator,
            i128::from(other.twos) - i128::from(twos),
            i128::from(other.fives) - i128::from(fives),
        )?;
        let shared = self.denominator.gcd(&other.denominator);
        let over_this = over(&self.denominator, &shared);
        let here = product(&here, &over(&other.denominator, &shared))?;
        let there = product(&there, &over_this)?;
        let (negative, sum) = signed_sum((self.negative, &here), (other.negative, &there));
        Ratio::canonical(
            negative,
            held(sum)?,
            product(&over_this, &other.denominator)?,
            i128::from(twos),
            i128::from(fives),
        )
    }

    pub(crate) fn subtract(&self, other: &Ratio) -> Option<Ratio> {
        self.add(&other.negated())
    }

    /// Where the magnitude stands against `other`'s. Nothing where the pair needs a working width
    /// past what a `Decimal`'s integer may be.
    ///
    /// Two magnitudes whose logarithms are apart are ordered by them and nothing is built, which is
    /// nearly every pair. The rest are cross-multiplied, with the powers each side has in excess
    /// of the other's on the side that has them.
    fn compare_magnitudes(&self, other: &Ratio) -> Option<Ordering> {
        let (low, high) = log_bounds(
            &self.numerator,
            &self.denominator,
            i128::from(self.twos),
            i128::from(self.fives),
        );
        let (other_low, other_high) = log_bounds(
            &other.numerator,
            &other.denominator,
            i128::from(other.twos),
            i128::from(other.fives),
        );
        if high < other_low {
            return Some(Ordering::Less);
        }
        if low > other_high {
            return Some(Ordering::Greater);
        }
        let twos = i128::from(self.twos) - i128::from(other.twos);
        let fives = i128::from(self.fives) - i128::from(other.fives);
        let left = written(
            &product(&self.numerator, &other.denominator)?,
            twos.max(0),
            fives.max(0),
        )?;
        let right = written(
            &product(&other.numerator, &self.denominator)?,
            (-twos).max(0),
            (-fives).max(0),
        )?;
        Some(left.cmp(&right))
    }

    /// Where this stands against `other` by exact value. Nothing where the pair needs a working
    /// width past what a `Decimal`'s integer may be.
    pub(crate) fn compare(&self, other: &Ratio) -> Option<Ordering> {
        if self == other {
            return Some(Ordering::Equal);
        }
        let by_sign = self.signum().cmp(&other.signum());
        if by_sign != Ordering::Equal {
            return Some(by_sign);
        }
        let by_magnitude = self.compare_magnitudes(other)?;
        Some(if self.negative {
            by_magnitude.reverse()
        } else {
            by_magnitude
        })
    }

    /// The whole number the magnitude of the value at `scale` rounds to by `mode`, where that is
    /// no wider than `widest` bits. The scale goes to the exponents and nothing is built from it,
    /// so a value far below the unit rounds to nought without the power that would have said so.
    fn rounded_at(&self, scale: i32, mode: Rounding, widest: u64) -> Option<Magnitude> {
        if self.is_zero() {
            return Some(Magnitude::ZERO);
        }
        let twos = i128::from(self.twos) + i128::from(scale);
        let fives = i128::from(self.fives) + i128::from(scale);
        let (low, high) = log_bounds(&self.numerator, &self.denominator, twos, fives);
        // Below an eighth of the unit, which is below half of it.
        if high < -3 {
            return held(rounded(
                Magnitude::ZERO,
                self.negative,
                Dropped::BelowHalf,
                mode,
            ));
        }
        if low > i128::from(widest) + 1 {
            return None;
        }
        let up = written(&self.numerator, twos.max(0), fives.max(0))?;
        let down = written(&self.denominator, (-twos).max(0), (-fives).max(0))?;
        let (quotient, remainder) = up.div_rem(&down);
        held(rounded(
            quotient,
            self.negative,
            dropped(&remainder, &down),
            mode,
        ))
    }

    /// An `Int`, where a magnitude and a sign name one.
    fn as_int(&self, magnitude: &Magnitude) -> Option<i64> {
        let Magnitude::Small(small) = magnitude else {
            return None;
        };
        let magnitude = u64::try_from(*small).ok()?;
        if self.negative {
            0i64.checked_sub_unsigned(magnitude)
        } else {
            i64::try_from(magnitude).ok()
        }
    }

    /// `Rational.toWholeNumber` of a whole number: nothing where it is past every `Int`.
    pub(crate) fn to_whole(&self) -> Option<i64> {
        assert!(
            self.is_whole(),
            "a fraction is answered as a case and never read as whole"
        );
        let (low, _) = log_bounds(
            &self.numerator,
            &self.denominator,
            i128::from(self.twos),
            i128::from(self.fives),
        );
        if low > 65 {
            return None;
        }
        let whole = written(
            &self.numerator,
            i128::from(self.twos),
            i128::from(self.fives),
        )?;
        self.as_int(&whole)
    }

    /// `Rational.toFiniteDecimal` of a value that has one: at the least scale a `Decimal` holds
    /// that writes it, with what is left of the powers among its digits. Nothing where no scale
    /// does.
    pub(crate) fn to_finite_decimal(&self) -> Option<Amount> {
        assert!(
            self.has_finite_decimal(),
            "a repeating fraction is answered as a case and never read as a decimal"
        );
        let tens = i128::from(self.twos.min(self.fives));
        if tens < -i128::from(i32::MAX) {
            return None;
        }
        let scale = if tens > i128::from(i32::MAX) + 1 {
            i32::MIN
        } else {
            (-tens) as i32
        };
        let digits = written(
            &self.numerator,
            i128::from(self.twos) + i128::from(scale),
            i128::from(self.fives) + i128::from(scale),
        )?;
        Amount::of_magnitude(self.negative, digits, scale)
    }

    /// `Rational.toInt`: the whole number the value rounds to by `mode`. Nothing where that is not
    /// an `Int`.
    pub(crate) fn to_int(&self, mode: Rounding) -> Option<i64> {
        let whole = self.rounded_at(0, mode, 64)?;
        self.as_int(&whole)
    }

    /// `Rational.toDecimal`: the value at `scale` places, rounded by `mode`. Nothing where the
    /// scale is not one a `Decimal` has or the value at it is wider than a `Decimal` holds.
    pub(crate) fn to_decimal(&self, scale: i64, mode: Rounding) -> Option<Amount> {
        let scale = i32::try_from(scale).ok()?;
        let magnitude = self.rounded_at(scale, mode, WIDEST)?;
        Amount::of_magnitude(self.negative && !magnitude.is_zero(), magnitude, scale)
    }
}

/// A `Rational` holding this value, in room the arena answered: the one way one is written.
fn rational_of(ratio: &Ratio) -> *mut Rational {
    ratio.numerator.with_le_bytes(|numerator| {
        ratio.denominator.with_le_bytes(|denominator| {
            let numerator_length =
                i64::try_from(numerator.len()).expect("a magnitude is shorter than an Int");
            let denominator_length =
                i64::try_from(denominator.len()).expect("a magnitude is shorter than an Int");
            let at = souther_alloc(Count(BYTES as i64 + numerator_length + denominator_length));
            unsafe {
                at.add(TWOS).cast::<i64>().write(ratio.twos);
                at.add(FIVES).cast::<i64>().write(ratio.fives);
                at.add(SIGNED_NUMERATOR)
                    .cast::<i64>()
                    .write(if ratio.negative {
                        -numerator_length
                    } else {
                        numerator_length
                    });
                at.add(DENOMINATOR_LENGTH)
                    .cast::<i64>()
                    .write(denominator_length);
                at.add(BYTES)
                    .copy_from_nonoverlapping(numerator.as_ptr(), numerator.len());
                at.add(BYTES + numerator.len())
                    .copy_from_nonoverlapping(denominator.as_ptr(), denominator.len());
            }
            at.cast()
        })
    })
}

/// The value a `Rational` holds.
///
/// # Safety
///
/// `at` is one [`rational_of`] answered, and the mark below it still stands.
unsafe fn ratio(at: *const Rational) -> Ratio {
    let at = at.cast::<u8>();
    unsafe {
        let signed = at.add(SIGNED_NUMERATOR).cast::<i64>().read();
        let numerator_length = signed.unsigned_abs() as usize;
        let denominator_length = at.add(DENOMINATOR_LENGTH).cast::<i64>().read() as usize;
        let numerator = std::slice::from_raw_parts(at.add(BYTES), numerator_length);
        let denominator =
            std::slice::from_raw_parts(at.add(BYTES + numerator_length), denominator_length);
        Ratio {
            negative: signed < 0,
            numerator: Magnitude::of_le_bytes(numerator),
            denominator: Magnitude::of_le_bytes(denominator),
            twos: at.add(TWOS).cast::<i64>().read(),
            fives: at.add(FIVES).cast::<i64>().read(),
        }
    }
}

/// `Rational.fromInt`.
#[unsafe(no_mangle)]
pub extern "C" fn souther_rational_from_int(value: i64) -> *mut Rational {
    rational_of(&Ratio::of_int(value))
}

/// `Rational.fromDecimal`, and the reading of a `Decimal` an operator does as an exact number.
///
/// # Safety
///
/// `at` is a `Decimal` the runtime answered, and the mark below it still stands. So for every
/// function here that reads a `Rational`, of one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_from_decimal(at: *const Decimal) -> *mut Rational {
    rational_of(&Ratio::of_decimal(&unsafe { amount(at) }))
}

/// The unary `-`.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_negate(at: *const Rational) -> *mut Rational {
    rational_of(&unsafe { ratio(at) }.negated())
}

/// Whether a `Rational` is nought: what a division asks of its divisor before it divides.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_is_zero(at: *const Rational) -> i8 {
    i8::from(unsafe { ratio(at) }.is_zero())
}

/// Whether a `Rational` is a whole number.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_is_whole(at: *const Rational) -> i8 {
    i8::from(unsafe { ratio(at) }.is_whole())
}

/// Whether a `Rational` has a finite decimal spelling.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_has_finite_decimal(at: *const Rational) -> i8 {
    i8::from(unsafe { ratio(at) }.has_finite_decimal())
}

/// Two `Rational`s by exact value, whatever their exponents: `==`, `<` and `Rational.compare`.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
///
/// # Panics
///
/// Where the pair needs a working width past what a `Decimal`'s integer may be to be ordered: two
/// values that close, at exponents that far from one another. That is a run with no room for what
/// the answer wanted and not a value with no place, so it ends as an arena that has run out does
/// and not as an abort of the program: which of the two it is decides which of the two it ends as.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_compare(
    left: *const Rational,
    right: *const Rational,
) -> Comparison {
    let ordering = unsafe { ratio(left).compare(&ratio(right)) }
        .expect("no room for the working width this pair of rationals is ordered at");
    Comparison(ordering as i64)
}

/// `+` and `Rational.add`, written through `out` where the sum is one a `Rational` holds.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`], and `out` is room for the address of a `Rational`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_add(
    left: *const Rational,
    right: *const Rational,
    out: *mut *mut Rational,
) -> i8 {
    let sum = unsafe { ratio(left).add(&ratio(right)) };
    unsafe { answered(sum.as_ref().map(rational_of), out) }
}

/// `-` and `Rational.subtract`, as [`souther_rational_add`].
///
/// # Safety
///
/// As [`souther_rational_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_subtract(
    left: *const Rational,
    right: *const Rational,
    out: *mut *mut Rational,
) -> i8 {
    let difference = unsafe { ratio(left).subtract(&ratio(right)) };
    unsafe { answered(difference.as_ref().map(rational_of), out) }
}

/// `*` and `Rational.multiply`, as [`souther_rational_add`].
///
/// # Safety
///
/// As [`souther_rational_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_multiply(
    left: *const Rational,
    right: *const Rational,
    out: *mut *mut Rational,
) -> i8 {
    let product = unsafe { ratio(left).multiply(&ratio(right)) };
    unsafe { answered(product.as_ref().map(rational_of), out) }
}

/// `/` and `Rational.divide`, as [`souther_rational_add`], over a divisor
/// [`souther_rational_is_zero`] has said is not nought.
///
/// # Safety
///
/// As [`souther_rational_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_divide(
    dividend: *const Rational,
    divisor: *const Rational,
    out: *mut *mut Rational,
) -> i8 {
    let quotient = unsafe { ratio(dividend).divide(&ratio(divisor)) };
    unsafe { answered(quotient.as_ref().map(rational_of), out) }
}

/// `Rational.toWholeNumber` of a value [`souther_rational_is_whole`] has said is whole, written
/// through `out` where it is an `Int`.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`], and `out` is room for an `Int`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_to_whole(at: *const Rational, out: *mut i64) -> i8 {
    unsafe { answered(ratio(at).to_whole(), out) }
}

/// `Rational.toFiniteDecimal` of a value [`souther_rational_has_finite_decimal`] has said has one,
/// written through `out` where a `Decimal` holds it.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`], and `out` is room for the address of a `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_to_finite_decimal(
    at: *const Rational,
    out: *mut *mut Decimal,
) -> i8 {
    let written = unsafe { ratio(at) }.to_finite_decimal();
    unsafe { answered(written.as_ref().map(decimal_of), out) }
}

/// `Rational.toInt`, written through `out` where the whole number the value rounds to is an `Int`.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`], `mode` is a value of `RoundingMode`, and `out` is room
/// for an `Int`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_to_int(
    mode: *const Value,
    at: *const Rational,
    out: *mut i64,
) -> i8 {
    let whole = unsafe { ratio(at).to_int(rounding(mode)) };
    unsafe { answered(whole, out) }
}

/// `Rational.toDecimal`, written through `out` where the scale is one a `Decimal` has and the value
/// at it is one a `Decimal` holds.
///
/// # Safety
///
/// As [`souther_rational_to_int`], and `out` is room for the address of a `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_to_decimal(
    scale: i64,
    mode: *const Value,
    at: *const Rational,
    out: *mut *mut Decimal,
) -> i8 {
    let rounded = unsafe { ratio(at).to_decimal(scale, rounding(mode)) };
    unsafe { answered(rounded.as_ref().map(decimal_of), out) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{souther_mark, souther_reset};

    fn whole(n: i64) -> Ratio {
        Ratio::of_int(n)
    }

    fn ratio_of(n: i64, d: i64) -> Ratio {
        whole(n)
            .divide(&whole(d))
            .expect("a small quotient has a place")
    }

    fn decimal(negative: bool, digits: u8, scale: i32) -> Amount {
        Amount::of_parts(negative, &[digits], scale)
    }

    #[test]
    fn a_value_has_one_form_whatever_it_was_written_as() {
        assert_eq!(ratio_of(6, 4), ratio_of(3, 2));
        assert_eq!(ratio_of(-6, 4), ratio_of(3, -2));
        assert_eq!(ratio_of(0, 7), Ratio::ZERO);
        assert_eq!(ratio_of(4, 2), whole(2));
        // A half is a one and a power of two, with no denominator left.
        let half = ratio_of(1, 2);
        assert_eq!((half.twos, half.fives), (-1, 0));
        assert_eq!(half.denominator, one());
        // A decimal and the ratio of the same value are one value: 0.5 is 5 at scale 1.
        assert_eq!(Ratio::of_decimal(&decimal(false, 5, 1)), half);
        assert_eq!(Ratio::of_decimal(&decimal(false, 50, 2)), half);
        assert_eq!(Ratio::of_decimal(&decimal(false, 0, 9)), Ratio::ZERO);
    }

    #[test]
    fn the_four_operations_are_exact() {
        let half = ratio_of(1, 2);
        let third = ratio_of(1, 3);
        assert_eq!(half.add(&third), Some(ratio_of(5, 6)));
        assert_eq!(half.subtract(&third), Some(ratio_of(1, 6)));
        assert_eq!(third.subtract(&half), Some(ratio_of(-1, 6)));
        assert_eq!(third.multiply(&whole(3)), Some(whole(1)));
        assert_eq!(third.divide(&third), Some(whole(1)));
        assert_eq!(half.subtract(&half), Some(Ratio::ZERO));
        assert_eq!(ratio_of(-1, 2).add(&half), Some(Ratio::ZERO));
        assert_eq!(
            ratio_of(-2, 3).multiply(&ratio_of(-3, 4)),
            Some(half.clone())
        );
        assert_eq!(half.negated().negated(), half);
        assert_eq!(Ratio::ZERO.negated(), Ratio::ZERO);
        // A tenth and a fifth: the sum has a five in it that comes off.
        assert_eq!(
            Ratio::of_decimal(&decimal(false, 1, 1)).add(&Ratio::of_decimal(&decimal(false, 2, 1))),
            Some(Ratio::of_decimal(&decimal(false, 3, 1)))
        );
    }

    #[test]
    fn an_order_is_by_exact_value() {
        assert_eq!(
            ratio_of(1, 3).compare(&ratio_of(1, 2)),
            Some(Ordering::Less)
        );
        assert_eq!(
            ratio_of(-1, 2).compare(&ratio_of(-1, 3)),
            Some(Ordering::Less)
        );
        assert_eq!(
            ratio_of(-1, 2).compare(&ratio_of(1, 3)),
            Some(Ordering::Less)
        );
        assert_eq!(
            ratio_of(2, 4).compare(&ratio_of(1, 2)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Ratio::ZERO.compare(&ratio_of(-1, 9)),
            Some(Ordering::Greater)
        );
        // Close enough that no logarithm settles it.
        let near = whole(i64::MAX).divide(&whole(i64::MAX - 1)).unwrap();
        assert_eq!(near.compare(&whole(1)), Some(Ordering::Greater));
        assert_eq!(whole(1).compare(&near), Some(Ordering::Less));
    }

    #[test]
    fn a_scale_at_the_end_of_its_range_is_an_exponent_and_nothing_is_built() {
        let least = Ratio::of_decimal(&decimal(false, 1, i32::MAX));
        assert_eq!(
            (least.twos, least.fives),
            (-i64::from(i32::MAX), -i64::from(i32::MAX))
        );
        let most = Ratio::of_decimal(&decimal(false, 1, i32::MIN));
        assert_eq!((most.twos, most.fives), (1 << 31, 1 << 31));
        assert_eq!(least.compare(&whole(1)), Some(Ordering::Less));
        assert_eq!(most.compare(&whole(1)), Some(Ordering::Greater));
        assert_eq!(most.negated().compare(&least), Some(Ordering::Less));
        // A scale's negation reaches one further than its own end, so this is ten and not one.
        assert_eq!(least.multiply(&most), Some(whole(10)));
        assert_eq!(least.divide(&least), Some(whole(1)));
        // The sum is a number with two billion digits in it, which no `Decimal` holds.
        assert_eq!(whole(1).add(&least), None);
    }

    #[test]
    fn an_exponent_past_sixty_four_bits_has_no_place() {
        let big = Ratio::of_decimal(&decimal(false, 1, i32::MIN));
        let mut power = big;
        // Squaring doubles the exponent: 2^31, 2^32, ... 2^63 is past a signed exponent.
        let mut refused = false;
        for _ in 0..40 {
            match power.multiply(&power) {
                Some(next) => power = next,
                None => {
                    refused = true;
                    break;
                }
            }
        }
        assert!(refused);
    }

    #[test]
    fn what_is_whole_and_what_has_a_finite_decimal() {
        assert!(ratio_of(4, 2).is_whole());
        assert!(!ratio_of(3, 2).is_whole());
        assert!(!ratio_of(1, 3).is_whole());
        assert!(ratio_of(1, 2).has_finite_decimal());
        assert!(ratio_of(1, 5).has_finite_decimal());
        assert!(!ratio_of(1, 3).has_finite_decimal());
        assert!(!ratio_of(1, 6).has_finite_decimal());
        assert!(Ratio::of_decimal(&decimal(false, 1, 5)).has_finite_decimal());
        assert!(!Ratio::of_decimal(&decimal(false, 1, 5)).is_whole());
        assert!(Ratio::of_decimal(&decimal(false, 1, -5)).is_whole());
    }

    #[test]
    fn a_whole_number_is_an_int_where_an_int_holds_it() {
        assert_eq!(whole(0).to_whole(), Some(0));
        assert_eq!(whole(i64::MIN).to_whole(), Some(i64::MIN));
        assert_eq!(whole(i64::MAX).to_whole(), Some(i64::MAX));
        assert_eq!(whole(i64::MAX).add(&whole(1)).unwrap().to_whole(), None);
        assert_eq!(
            whole(i64::MIN).subtract(&whole(1)).unwrap().to_whole(),
            None
        );
        assert_eq!(
            Ratio::of_decimal(&decimal(true, 7, -3)).to_whole(),
            Some(-7000)
        );
        let huge = Ratio::of_decimal(&decimal(false, 1, i32::MIN));
        assert_eq!(huge.to_whole(), None);
    }

    #[test]
    fn a_finite_decimal_is_written_at_the_least_scale_that_writes_it() {
        assert_eq!(
            ratio_of(1, 2).to_finite_decimal(),
            Some(decimal(false, 5, 1))
        );
        assert_eq!(
            ratio_of(-1, 4).to_finite_decimal(),
            Some(Amount::of_parts(true, &[25], 2))
        );
        assert_eq!(whole(3).to_finite_decimal(), Some(decimal(false, 3, 0)));
        assert_eq!(Ratio::ZERO.to_finite_decimal(), Some(decimal(false, 0, 0)));
        // 3000 is 3 at scale -3: the least scale that writes it.
        assert_eq!(whole(3000).to_finite_decimal(), Some(decimal(false, 3, -3)));
        assert_eq!(
            Ratio::of_decimal(&decimal(false, 12, 40)).to_finite_decimal(),
            Some(decimal(false, 12, 40))
        );
        // A scale at the end of the range is a scale a `Decimal` has.
        let least = decimal(false, 1, i32::MAX);
        assert_eq!(Ratio::of_decimal(&least).to_finite_decimal(), Some(least));
    }

    #[test]
    fn a_value_with_a_fraction_is_rounded_by_the_mode_it_is_told() {
        let five_halves = ratio_of(5, 2);
        assert_eq!(five_halves.to_int(Rounding::HalfEven), Some(2));
        assert_eq!(five_halves.to_int(Rounding::HalfUp), Some(3));
        assert_eq!(five_halves.to_int(Rounding::HalfDown), Some(2));
        assert_eq!(five_halves.to_int(Rounding::Down), Some(2));
        assert_eq!(five_halves.to_int(Rounding::Up), Some(3));
        assert_eq!(ratio_of(-5, 2).to_int(Rounding::HalfUp), Some(-3));
        assert_eq!(ratio_of(-5, 2).to_int(Rounding::Ceiling), Some(-2));
        assert_eq!(ratio_of(-5, 2).to_int(Rounding::Floor), Some(-3));
        assert_eq!(ratio_of(-1, 3).to_int(Rounding::Ceiling), Some(0));
        assert_eq!(whole(7).to_int(Rounding::Up), Some(7));
        assert_eq!(
            whole(i64::MAX).add(&whole(1)).unwrap().to_int(Rounding::Up),
            None
        );

        assert_eq!(
            ratio_of(1, 3).to_decimal(2, Rounding::HalfUp),
            Some(decimal(false, 33, 2))
        );
        assert_eq!(
            ratio_of(2, 3).to_decimal(2, Rounding::HalfUp),
            Some(decimal(false, 67, 2))
        );
        assert_eq!(
            ratio_of(2, 3).to_decimal(0, Rounding::Down),
            Some(decimal(false, 0, 0))
        );
        assert_eq!(
            ratio_of(-1, 3).to_decimal(2, Rounding::Down),
            Some(decimal(true, 33, 2))
        );
        assert_eq!(
            ratio_of(1, 8).to_decimal(2, Rounding::HalfEven),
            Some(decimal(false, 12, 2))
        );
        assert_eq!(
            ratio_of(1, 8).to_decimal(2, Rounding::HalfUp),
            Some(decimal(false, 13, 2))
        );
        // A negative scale is a grid of tens.
        assert_eq!(
            whole(1234).to_decimal(-2, Rounding::HalfUp),
            Some(decimal(false, 12, -2))
        );
        assert_eq!(
            ratio_of(1, 3).to_decimal(i64::from(i32::MAX) + 1, Rounding::Down),
            None
        );
    }

    #[test]
    fn a_value_far_below_the_unit_rounds_without_the_power_that_says_so() {
        let tiny = Ratio::of_decimal(&decimal(false, 1, i32::MAX));
        assert_eq!(tiny.to_int(Rounding::Down), Some(0));
        assert_eq!(tiny.to_int(Rounding::HalfUp), Some(0));
        assert_eq!(tiny.to_int(Rounding::Up), Some(1));
        assert_eq!(tiny.negated().to_int(Rounding::Floor), Some(-1));
        assert_eq!(tiny.negated().to_int(Rounding::Down), Some(0));
        assert_eq!(tiny.to_decimal(0, Rounding::Up), Some(decimal(false, 1, 0)));
        // At its own scale it is what it was.
        assert_eq!(
            tiny.to_decimal(i64::from(i32::MAX), Rounding::Down),
            Some(decimal(false, 1, i32::MAX))
        );
    }

    #[test]
    fn a_value_past_what_a_decimal_holds_has_no_place_at_a_scale() {
        let huge = Ratio::of_decimal(&decimal(false, 1, i32::MIN));
        assert_eq!(huge.to_decimal(0, Rounding::Down), None);
        assert_eq!(huge.to_int(Rounding::Down), None);
    }

    #[test]
    fn what_the_arena_holds_reads_back_as_what_went_in() {
        let mark = souther_mark();
        for value in [
            Ratio::ZERO,
            ratio_of(1, 3),
            ratio_of(-7, 12),
            whole(i64::MIN),
            Ratio::of_decimal(&decimal(true, 9, i32::MIN)),
            Ratio::of_decimal(&decimal(false, 1, i32::MAX)),
        ] {
            let at = rational_of(&value);
            assert_eq!(unsafe { ratio(at) }, value);
        }
        let third = souther_rational_from_int(3);
        let one_third = {
            let mut out = std::ptr::null_mut();
            let one = souther_rational_from_int(1);
            assert_eq!(unsafe { souther_rational_divide(one, third, &mut out) }, 1);
            out
        };
        assert_eq!(
            unsafe { souther_rational_compare(one_third, third) },
            Comparison(-1)
        );
        assert_eq!(unsafe { souther_rational_is_whole(one_third) }, 0);
        assert_eq!(unsafe { souther_rational_is_whole(third) }, 1);
        let mut whole = 0;
        assert_eq!(unsafe { souther_rational_to_whole(third, &mut whole) }, 1);
        assert_eq!(whole, 3);
        souther_reset(mark);
    }
}
