//! A `Rational` as generated code holds one: an address, and behind it a layout only this file
//! reads.
//!
//! A `Rational` has no external form, so no host is ever handed one and none reads behind it: it
//! is made and read by generated code and by this runtime, between which it is an address for the
//! reason a `Decimal` is one. What a value is and what each operation answers is
//! `souther_exact`'s [`Ratio`], which the WebAssembly runtime answers with too, and what is here is
//! where one is kept and how an operation that answers nothing says so, as the `Decimal`'s do, by
//! answering whether it wrote its value (`kernels`), so that which reason the run ends for is read
//! off the call and not decided here.
//!
//! A [`Failure`] is one of two, and they end a run two ways. A value with no place is an answer
//! the language has an abort for, so it is answered as nothing written. The run having no room for
//! what the answer needed is not a value at all, so it ends the run as an arena that has run out
//! does ([`settled`]).
//!
//! The layout, in the arena and aligned to a slot as everything there is: the power of two in the
//! first slot, the power of five in the second, the sign times how many bytes the numerator is in
//! the third, how many bytes the denominator is in the fourth, and the numerator's bytes and then
//! the denominator's after them, each little end first and with no zero byte at the top.

use crate::collection::{Hash, hash_of_parts};
use crate::decimal::{Decimal, amount, decimal_of, rounding};
use crate::kernels::answered;
use crate::{Bool, Comparison, Count, Value, souther_alloc};
use souther_exact::{Exact, Failure, Magnitude, Ratio};
use souther_native_abi::SLOT;

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

/// The answer, where there is one, for the caller that ends the run for a value with no place: and
/// the end of the run, where there was no room, for the one that has nothing to say for a shortage
/// but that it happened.
///
/// # Panics
///
/// Where the run had no room, as an arena that has run out does.
fn settled<T>(result: Exact<T>) -> Option<T> {
    match result {
        Ok(answer) => Some(answer),
        Err(Failure::NoPlace) => None,
        Err(Failure::NoRoom) => {
            panic!("no room for the working width this answer is worked out at")
        }
    }
}

/// A `Rational` holding this value, in room the arena answered: the one way one is written.
fn rational_of(ratio: &Ratio) -> *mut Rational {
    let (negative, numerator, denominator, twos, fives) = ratio.parts();
    numerator.with_le_bytes(|numerator| {
        denominator.with_le_bytes(|denominator| {
            let numerator_length =
                i64::try_from(numerator.len()).expect("a magnitude is shorter than an Int");
            let denominator_length =
                i64::try_from(denominator.len()).expect("a magnitude is shorter than an Int");
            let at = souther_alloc(Count(BYTES as i64 + numerator_length + denominator_length));
            unsafe {
                at.add(TWOS).cast::<i64>().write(twos);
                at.add(FIVES).cast::<i64>().write(fives);
                at.add(SIGNED_NUMERATOR).cast::<i64>().write(if negative {
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
/// `at` is one [`rational_of`] answered, and the scope it was made in is still open.
unsafe fn ratio(at: *const Rational) -> Ratio {
    let at = at.cast::<u8>();
    unsafe {
        let signed = at.add(SIGNED_NUMERATOR).cast::<i64>().read();
        let numerator_length = signed.unsigned_abs() as usize;
        let denominator_length = at.add(DENOMINATOR_LENGTH).cast::<i64>().read() as usize;
        let numerator = std::slice::from_raw_parts(at.add(BYTES), numerator_length);
        let denominator =
            std::slice::from_raw_parts(at.add(BYTES + numerator_length), denominator_length);
        // Trusted because every cell of one was written from the parts of a value.
        Ratio::from_trusted_parts(
            signed < 0,
            Magnitude::of_le_bytes(numerator),
            Magnitude::of_le_bytes(denominator),
            at.add(TWOS).cast::<i64>().read(),
            at.add(FIVES).cast::<i64>().read(),
        )
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
/// `at` is a `Decimal` the runtime answered, and the scope it was made in is still open. So for every
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
pub unsafe extern "C" fn souther_rational_is_zero(at: *const Rational) -> Bool {
    Bool::from(unsafe { ratio(at) }.is_zero())
}

/// Whether a `Rational` is a whole number.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_is_whole(at: *const Rational) -> Bool {
    Bool::from(unsafe { ratio(at) }.is_whole())
}

/// Whether a `Rational` has a finite decimal spelling.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_has_finite_decimal(at: *const Rational) -> Bool {
    Bool::from(unsafe { ratio(at) }.has_finite_decimal())
}

/// Two `Rational`s by exact value, whatever their exponents: `==`, `<` and `Rational.compare`.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
///
/// # Panics
///
/// Where the pair needs more room to be ordered than a run has: two values that close, at
/// exponents that far from one another. That is a run with no room for what
/// the answer wanted and not a value with no place, so it ends as an arena that has run out does
/// and not as an abort of the program: which of the two it is decides which of the two it ends as.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_compare(
    left: *const Rational,
    right: *const Rational,
) -> Comparison {
    let ordering = settled(unsafe { ratio(left).compare(&ratio(right)) })
        .expect("an order has an answer for every pair");
    Comparison(ordering as i64)
}

/// A `Rational`'s hash, from the one form each value is kept in, so two that are equal hash alike:
/// its sign, the bytes of its numerator and its denominator, and its powers of two and five, and
/// never written out as text first.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_hash(at: *const Rational) -> Hash {
    let value = unsafe { ratio(at) };
    let (negative, numerator, denominator, twos, fives) = value.parts();
    numerator.with_le_bytes(|numerator| {
        denominator.with_le_bytes(|denominator| {
            hash_of_parts(&[
                &[u8::from(negative)],
                numerator,
                denominator,
                &twos.to_le_bytes(),
                &fives.to_le_bytes(),
            ])
        })
    })
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
) -> Bool {
    let sum = settled(unsafe { ratio(left).add(&ratio(right)) });
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
) -> Bool {
    let difference = settled(unsafe { ratio(left).subtract(&ratio(right)) });
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
) -> Bool {
    let product = settled(unsafe { ratio(left).multiply(&ratio(right)) });
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
) -> Bool {
    let quotient = settled(unsafe { ratio(dividend).divide(&ratio(divisor)) });
    unsafe { answered(quotient.as_ref().map(rational_of), out) }
}

/// `Rational.toWholeNumber` of a value [`souther_rational_is_whole`] has said is whole, written
/// through `out` where it is an `Int`.
///
/// # Safety
///
/// As [`souther_rational_from_decimal`], and `out` is room for an `Int`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_rational_to_whole(at: *const Rational, out: *mut i64) -> Bool {
    unsafe { answered(settled(ratio(at).to_whole()), out) }
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
) -> Bool {
    let written = settled(unsafe { ratio(at) }.to_finite_decimal());
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
) -> Bool {
    let whole = settled(unsafe { ratio(at).to_int(rounding(mode)) });
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
) -> Bool {
    let rounded = settled(unsafe { ratio(at).to_decimal(scale, rounding(mode)) });
    unsafe { answered(rounded.as_ref().map(decimal_of), out) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amount::Amount;
    use crate::{souther_scope_close, souther_scope_open};

    fn ratio_of(n: i64, d: i64) -> Ratio {
        Ratio::of_int(n)
            .divide(&Ratio::of_int(d))
            .expect("a small quotient has a place")
    }

    #[test]
    fn what_the_arena_holds_reads_back_as_what_went_in() {
        let scope = souther_scope_open();
        for value in [
            Ratio::ZERO,
            ratio_of(1, 3),
            ratio_of(-7, 12),
            Ratio::of_int(i64::MIN),
            Ratio::of_decimal(&Amount::from_trusted_parts(true, &[9], i32::MIN)),
            Ratio::of_decimal(&Amount::from_trusted_parts(false, &[1], i32::MAX)),
        ] {
            let at = rational_of(&value);
            assert_eq!(unsafe { ratio(at) }, value);
        }
        let third = souther_rational_from_int(3);
        let one_third = {
            let mut out = std::ptr::null_mut();
            let one = souther_rational_from_int(1);
            assert_eq!(
                unsafe { souther_rational_divide(one, third, &mut out) },
                Bool::TRUE
            );
            out
        };
        assert_eq!(
            unsafe { souther_rational_compare(one_third, third) },
            Comparison(-1)
        );
        assert_eq!(unsafe { souther_rational_is_whole(one_third) }, Bool::FALSE);
        assert_eq!(unsafe { souther_rational_is_whole(third) }, Bool::TRUE);
        let mut whole = 0;
        assert_eq!(
            unsafe { souther_rational_to_whole(third, &mut whole) },
            Bool::TRUE
        );
        assert_eq!(whole, 3);
        souther_scope_close(scope);
    }

    /// A `Decimal` that becomes a `Rational` comes back as that `Decimal`, through the parts the two
    /// are read and written as.
    #[test]
    fn a_decimal_comes_back_as_itself() {
        for (negative, digits, scale) in [
            (false, 5u8, 1),
            (true, 25, 2),
            (false, 3, -3),
            (false, 1, i32::MAX),
        ] {
            let written = Amount::from_trusted_parts(negative, &[digits], scale);
            let back = Ratio::of_decimal(&written).to_finite_decimal().ok();
            assert_eq!(back, Some(written));
        }
    }

    /// The two limits are two: a value with no place says so, and the run being out of room is not
    /// that.
    #[test]
    fn what_has_no_place_is_not_what_there_was_no_room_for() {
        assert_eq!(settled::<u8>(Err(Failure::NoPlace)), None);
        assert!(std::panic::catch_unwind(|| settled::<u8>(Err(Failure::NoRoom))).is_err());
    }
}
