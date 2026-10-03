//! A `Decimal` as generated code and a host hold one: an address, and behind it a layout only this
//! file reads.
//!
//! Nothing outside the runtime reads behind the address. Generated code hands it to the functions
//! here and is handed one back, a value of a union carries it in a slot as it carries a string's,
//! and a host asks for its integer and its scale through [`souther_decimal_unscaled`] and
//! [`souther_decimal_scale`]. So how a `Decimal` is kept is not part of any contract: the layout
//! below can change without an object or a binding being built again, and `souther_native_abi`
//! says nothing about it.
//!
//! What each operation answers is [`Amount`]'s. What is here is where a value is kept, which mode a
//! `RoundingMode` value is, and how an operation that answers nothing says so: as a kernel over
//! strings does, by answering whether it wrote its value (`kernels`), so that which reason the run
//! ends for is read off the call and not decided here.
//!
//! The layout, in the arena and aligned to a slot as everything there is: the scale in the first
//! slot, the sign times how many bytes the magnitude is in the second, and the magnitude's bytes
//! after them, little end first and with no zero byte at the top. Nought is no bytes at all, at
//! whatever scale it was written at.

use crate::amount::{Amount, of_decimal_text, of_integer_text, plain_text};
use crate::collection::{Hash, hash_of_parts};
use crate::external::Form;
use crate::kernels::answered;
use crate::{Bool, Comparison, Count, Text, Value, souther_alloc, string_of, text};
use souther_exact::Rounding;
use souther_native_abi::{SLOT, WHICH};
use std::ptr;

/// A `Decimal`, as the functions here take and answer one: an address, a type of its own for the
/// reason [`Text`] is.
#[repr(C)]
pub struct Decimal {
    _opaque: [u8; 0],
}

/// Where the scale is.
const SCALE: usize = 0;
/// Where the sign times the count of the magnitude's bytes is.
const SIGNED_LENGTH: usize = SLOT as usize;
/// Where the magnitude's bytes start.
const MAGNITUDE: usize = 2 * SLOT as usize;

/// A `Decimal` holding this value, in room the arena answered: the one way one is written.
pub(crate) fn decimal_of(amount: &Amount) -> *mut Decimal {
    amount.with_parts(|negative, magnitude, scale| {
        let length = i64::try_from(magnitude.len()).expect("a magnitude is shorter than an Int");
        let at = souther_alloc(Count(MAGNITUDE as i64 + length));
        unsafe {
            at.add(SCALE).cast::<i64>().write(i64::from(scale));
            at.add(SIGNED_LENGTH)
                .cast::<i64>()
                .write(if negative { -length } else { length });
            at.add(MAGNITUDE)
                .copy_from_nonoverlapping(magnitude.as_ptr(), magnitude.len());
        }
        at.cast()
    })
}

/// The value a `Decimal` holds.
///
/// # Safety
///
/// `at` is one [`decimal_of`] answered, and the scope it was made in is still open.
pub(crate) unsafe fn amount(at: *const Decimal) -> Amount {
    let at = at.cast::<u8>();
    unsafe {
        let scale = at.add(SCALE).cast::<i64>().read();
        let signed = at.add(SIGNED_LENGTH).cast::<i64>().read();
        let magnitude =
            std::slice::from_raw_parts(at.add(MAGNITUDE), signed.unsigned_abs() as usize);
        let scale = i32::try_from(scale).expect("a Decimal carries a scale a Decimal has");
        Amount::of_parts(signed < 0, magnitude, scale)
    }
}

/// The token of every unit the language itself declares, defined under the symbol
/// `souther_native_abi::type_symbol` spells for it, as the object of a module's build defines one
/// for each declaration it is the home of. The runtime is the home of these: it is the one thing
/// every object in a library shares, so a value of one built in one object is the value another
/// reads, and no object defines one of its own.
///
/// The symbol and the entry in [`LANGUAGE_UNIT_TOKENS`] are made from one module and one name, so
/// the table says what is defined and a test holds it to the one `souther_native_abi` states.
macro_rules! language_units {
    ($($module:literal . $name:literal => $item:ident),* $(,)?) => {
        $(
            #[doc = concat!("The token a value of `", $module, ".", $name, "` carries.")]
            #[unsafe(export_name = concat!("souther$type$", $module, "$", $name))]
            pub static $item: [u8; 1] = [0];
        )*

        /// Every token defined here, by the module and the name of its declaration.
        #[cfg(test)]
        pub(crate) const LANGUAGE_UNIT_TOKENS: &[(&str, &str, &[u8; 1])] =
            &[$(($module, $name, &$item)),*];
    };
}

language_units! {
    "souther.decimal"."HALF_UP" => HALF_UP,
    "souther.decimal"."HALF_EVEN" => HALF_EVEN,
    "souther.decimal"."HALF_DOWN" => HALF_DOWN,
    "souther.decimal"."UP" => UP,
    "souther.decimal"."DOWN" => DOWN,
    "souther.decimal"."CEILING" => CEILING,
    "souther.decimal"."FLOOR" => FLOOR,
}

/// Which mode a value of `RoundingMode` is, read off the token at its front.
///
/// # Safety
///
/// `mode` is a value of `RoundingMode`: one generated code laid out with one of the tokens above,
/// which is the only way one is made.
pub(crate) unsafe fn rounding(mode: *const Value) -> Rounding {
    let token = unsafe {
        mode.cast::<u8>()
            .add(WHICH as usize)
            .cast::<*const u8>()
            .read()
    };
    [
        (&HALF_UP, Rounding::HalfUp),
        (&HALF_EVEN, Rounding::HalfEven),
        (&HALF_DOWN, Rounding::HalfDown),
        (&UP, Rounding::Up),
        (&DOWN, Rounding::Down),
        (&CEILING, Rounding::Ceiling),
        (&FLOOR, Rounding::Floor),
    ]
    .into_iter()
    .find(|(it, _)| ptr::eq(it.as_ptr(), token))
    .map(|(_, mode)| mode)
    .expect("a RoundingMode is tagged by one of the tokens the runtime defines for its cases")
}

/// The value integer text and a scale write, as [`souther_decimal_literal`] reads them: a `String`
/// of the runtime's layout, which a literal the compiler wrote always is.
///
/// # Safety
///
/// As [`crate::souther_string_compare`].
unsafe fn of_parts(unscaled: *const Text, scale: i64) -> *mut Decimal {
    let written = unsafe { text(&unscaled) };
    let scale = i32::try_from(scale).expect("a Decimal is handed over at a scale a Decimal has");
    let amount = souther_text::decimal_text(written)
        .and_then(|it| of_integer_text(it, scale))
        .expect("a Decimal's integer is handed over as integer text");
    decimal_of(&amount)
}

/// A `Decimal` of this integer and scale, for a caller outside a Souther program, written through
/// `out` where they name one, and answering whether they did: the integer as integer text (an
/// optional sign and ASCII digits, `length` bytes at `unscaled`) and the scale as a number a scale
/// may be. Bytes that are not integer text, a count below nought and a scale past the 32-bit range
/// name none, and are answered as that rather than ending the process: the runtime is what decides
/// what a `Decimal` is, and a binding that checks first does so only to say it in its own words.
///
/// Bytes and a count, and not a `String` of the runtime's layout: the unscaled digits are the
/// integer's text and not the value's written form (spec §what-a-string-holds says what a
/// `String` is measured against, and an integer's digits are never that — they are always ASCII,
/// so they always fit, and routing them through String admission would make a `Decimal` a caller
/// otherwise has fallible on a capacity that has nothing to do with it).
///
/// # Safety
///
/// `unscaled` points at `length` bytes that may be read, where `length` is above nought, and `out`
/// is room for the address of a `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_of_parts(
    unscaled: *const u8,
    length: Count,
    scale: i64,
    out: *mut *mut Decimal,
) -> Bool {
    let named = usize::try_from(length.0).ok().and_then(|held| {
        let bytes = if held == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(unscaled, held) }
        };
        let written = std::str::from_utf8(bytes).ok()?;
        let scale = i32::try_from(scale).ok()?;
        souther_text::decimal_text(souther_text::Text::held(written))
            .and_then(|it| of_integer_text(it, scale))
            .map(|amount| decimal_of(&amount))
    });
    unsafe { answered(named, out) }
}

/// A `Decimal` literal: the integer the checker read it as, which the object carries as a string,
/// and its scale.
///
/// # Safety
///
/// `unscaled` is a string of the runtime's layout, as a literal the compiler wrote always is, and
/// `scale` is one a `Decimal` has.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_literal(
    unscaled: *const Text,
    scale: i64,
) -> *mut Decimal {
    unsafe { of_parts(unscaled, scale) }
}

/// The integer a `Decimal` is, as integer text, for the same caller.
///
/// # Safety
///
/// `at` is a `Decimal` the runtime answered, and the scope it was made in is still open. So for every
/// function here that reads one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_unscaled(at: *const Decimal) -> *mut Text {
    string_of(&unsafe { amount(at) }.unscaled_text())
}

/// The scale a `Decimal` carries, for the same caller.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_scale(at: *const Decimal) -> i64 {
    i64::from(unsafe { amount(at) }.scale())
}

/// Two `Decimal`s by amount, whatever their scales: `==`, `<` and `Decimal.compare`.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_compare(
    left: *const Decimal,
    right: *const Decimal,
) -> Comparison {
    let ordering = unsafe { amount(left).compare(&amount(right)) };
    Comparison(ordering as i64)
}

/// A `Decimal`'s hash, from its amount with as many trailing zeros dropped as its scale lets it
/// drop, which is one form for one amount whatever the scale: `1.0` and `1.00`, which are equal,
/// hash alike. Hashed as its sign, its magnitude's bytes and its scale, and never written out as
/// text first.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_hash(at: *const Decimal) -> Hash {
    unsafe { amount(at) }
        .least_digits()
        .with_parts(|negative, magnitude, scale| {
            hash_of_parts(&[&[u8::from(negative)], magnitude, &scale.to_le_bytes()])
        })
}

/// Whether a `Decimal` is nought, at whatever scale: what a division asks of its divisor before it
/// divides.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_is_zero(at: *const Decimal) -> Bool {
    Bool::from(unsafe { amount(at) }.is_zero())
}

/// The unary `-`.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_negate(at: *const Decimal) -> *mut Decimal {
    decimal_of(&unsafe { amount(at) }.negated())
}

/// `+` and `Decimal.add`, written through `out` where the sum is one a `Decimal` holds.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`], and `out` is room for the address of a `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_add(
    left: *const Decimal,
    right: *const Decimal,
    out: *mut *mut Decimal,
) -> Bool {
    let sum = unsafe { amount(left).add(&amount(right)) };
    unsafe { answered(sum.as_ref().map(decimal_of), out) }
}

/// `-` and `Decimal.subtract`, as [`souther_decimal_add`].
///
/// # Safety
///
/// As [`souther_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_subtract(
    left: *const Decimal,
    right: *const Decimal,
    out: *mut *mut Decimal,
) -> Bool {
    let difference = unsafe { amount(left).subtract(&amount(right)) };
    unsafe { answered(difference.as_ref().map(decimal_of), out) }
}

/// `*` and `Decimal.multiply`, as [`souther_decimal_add`].
///
/// # Safety
///
/// As [`souther_decimal_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_multiply(
    left: *const Decimal,
    right: *const Decimal,
    out: *mut *mut Decimal,
) -> Bool {
    let product = unsafe { amount(left).multiply(&amount(right)) };
    unsafe { answered(product.as_ref().map(decimal_of), out) }
}

/// `Decimal.fromInt`.
#[unsafe(no_mangle)]
pub extern "C" fn souther_decimal_from_int(value: i64) -> *mut Decimal {
    decimal_of(&Amount::of_int(value))
}

/// `Decimal.toInt`, written through `out` where the whole number the value rounds to is an `Int`.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`], `mode` is a value of `RoundingMode`, and `out` is room for an
/// `Int`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_to_int(
    mode: *const Value,
    at: *const Decimal,
    out: *mut i64,
) -> Bool {
    let whole = unsafe { amount(at).to_int(rounding(mode)) };
    unsafe { answered(whole, out) }
}

/// `Decimal.round`, written through `out` where the scale is one a `Decimal` has and the value at
/// it is one a `Decimal` holds.
///
/// # Safety
///
/// As [`souther_decimal_to_int`], and `out` is room for the address of a `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_round(
    scale: i64,
    mode: *const Value,
    at: *const Decimal,
    out: *mut *mut Decimal,
) -> Bool {
    let rounded = unsafe { amount(at).round(scale, rounding(mode)) };
    unsafe { answered(rounded.as_ref().map(decimal_of), out) }
}

/// `Decimal.divide`, written through `out` where the scale is one a `Decimal` has and the quotient
/// at it is one a `Decimal` holds.
///
/// # Safety
///
/// As [`souther_decimal_round`], and the divisor is not nought, which the caller has already
/// answered as a case ([`souther_decimal_is_zero`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_decimal_divide(
    dividend: *const Decimal,
    divisor: *const Decimal,
    scale: i64,
    mode: *const Value,
    out: *mut *mut Decimal,
) -> Bool {
    let quotient = unsafe { amount(dividend).divide(&amount(divisor), scale, rounding(mode)) };
    unsafe { answered(quotient.as_ref().map(decimal_of), out) }
}

/// `String.toDecimal`, written through `out` where the text is decimal text.
///
/// # Safety
///
/// As [`crate::souther_string_compare`], and `out` is room for the address of a `Decimal`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_to_decimal(s: *const Text, out: *mut *mut Decimal) -> Bool {
    let read = souther_text::decimal_text(unsafe { text(&s) }).map(of_decimal_text);
    unsafe { answered(read.as_ref().map(decimal_of), out) }
}

/// `String.fromDecimal`: the plain notation, at the value's scale, written through `out` where a
/// string holds it. A value near either end of the scale range is a couple of billion characters,
/// which is measured before any of it is written.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`], and `out` is room for the address of a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_string_from_decimal(
    at: *const Decimal,
    out: *mut *mut Text,
) -> Bool {
    let written = plain_text(&unsafe { amount(at) });
    unsafe { answered(written.as_deref().map(string_of), out) }
}

/// A `Decimal` at a boundary: its amount, written as `Amount::external_text` says.
///
/// # Safety
///
/// As [`souther_decimal_unscaled`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_decimal(at: *const Decimal) -> *mut Form {
    crate::external::handed(Form::Amount(unsafe { amount(at) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Count, souther_scope_close, souther_scope_open, souther_string_of_utf8};
    use souther_native_abi::{LANGUAGE_UNITS, type_symbol};
    use std::collections::BTreeSet;

    fn made(text: &str) -> *mut Text {
        let mut out = std::ptr::null_mut();
        let admitted =
            unsafe { souther_string_of_utf8(text.as_ptr(), Count(text.len() as i64), &mut out) };
        assert_eq!(admitted, Bool::TRUE, "test text has a place");
        out
    }

    fn said(at: *const Text) -> String {
        String::from(unsafe { text(&at) }.as_str())
    }

    fn of(unscaled: &str, scale: i64) -> *mut Decimal {
        let mut out = ptr::null_mut();
        let made = unsafe {
            souther_decimal_of_parts(
                unscaled.as_ptr(),
                Count(unscaled.len() as i64),
                scale,
                &mut out,
            )
        };
        assert_eq!(made, Bool::TRUE, "{unscaled} at {scale} names a Decimal");
        out
    }

    fn parts(at: *const Decimal) -> (String, i64) {
        unsafe {
            (
                said(souther_decimal_unscaled(at)),
                souther_decimal_scale(at),
            )
        }
    }

    /// A value of `RoundingMode` as generated code lays one out: the token at its front.
    fn mode(token: &[u8; 1]) -> *const Value {
        let room = souther_alloc(Count(SLOT));
        unsafe { room.cast::<*const u8>().write(token.as_ptr()) };
        room.cast_const().cast()
    }

    /// What a host hands over is what it reads back, the scale as it was and nought unsigned.
    #[test]
    fn a_decimal_reads_back_as_the_integer_and_scale_it_was_made_of() {
        let scope = souther_scope_open();
        for (unscaled, scale, read) in [
            ("150", 2, "150"),
            ("-150", 2, "-150"),
            ("0", 5, "0"),
            ("-0", 5, "0"),
            ("+7", -3, "7"),
            ("00042", 0, "42"),
            (
                "123456789012345678901234567890",
                2147483647,
                "123456789012345678901234567890",
            ),
        ] {
            assert_eq!(
                parts(of(unscaled, scale)),
                (read.to_string(), scale),
                "{unscaled}"
            );
        }
        souther_scope_close(scope);
    }

    /// Every rounding mode reaches the mode its token names.
    #[test]
    fn a_mode_is_the_case_its_token_names() {
        let scope = souther_scope_open();
        let value = of("25", 1);
        let mut whole = 0;
        for (token, answer) in [
            (&HALF_UP, 3),
            (&HALF_EVEN, 2),
            (&HALF_DOWN, 2),
            (&UP, 3),
            (&DOWN, 2),
            (&CEILING, 3),
            (&FLOOR, 2),
        ] {
            assert_eq!(
                unsafe { souther_decimal_to_int(mode(token), value, &mut whole) },
                Bool::TRUE
            );
            assert_eq!(whole, answer);
        }
        souther_scope_close(scope);
    }

    /// An operation that has no answer writes nothing and says so.
    #[test]
    fn an_operation_with_no_answer_writes_nothing() {
        let scope = souther_scope_open();
        let mut out: *mut Decimal = ptr::null_mut();
        let tiny = of("1", 2147483647);
        assert_eq!(
            unsafe { souther_decimal_multiply(tiny, tiny, &mut out) },
            Bool::FALSE
        );
        assert!(out.is_null());
        assert_eq!(
            unsafe { souther_decimal_round(2147483648, mode(&UP), tiny, &mut out) },
            Bool::FALSE
        );
        assert!(out.is_null());
        assert_eq!(
            unsafe { souther_string_to_decimal(made("1e5"), &mut out) },
            Bool::FALSE
        );
        assert!(out.is_null());
        souther_scope_close(scope);
    }

    #[test]
    fn the_operations_answer_through_the_arena() {
        let scope = souther_scope_open();
        let mut out: *mut Decimal = ptr::null_mut();
        assert_eq!(
            unsafe { souther_decimal_add(of("15", 1), of("225", 2), &mut out) },
            Bool::TRUE
        );
        assert_eq!(parts(out), ("375".to_string(), 2));
        assert_eq!(
            unsafe { souther_decimal_divide(of("10", 0), of("3", 0), 2, mode(&HALF_UP), &mut out) },
            Bool::TRUE
        );
        assert_eq!(parts(out), ("333".to_string(), 2));
        assert_eq!(
            parts(unsafe { souther_decimal_negate(of("5", 1)) }),
            ("-5".to_string(), 1)
        );
        assert_eq!(
            parts(souther_decimal_from_int(i64::MIN)),
            (i64::MIN.to_string(), 0)
        );
        assert_eq!(
            unsafe { souther_decimal_compare(of("10", 1), of("100", 2)) }.0,
            0
        );
        assert_eq!(
            unsafe { souther_decimal_compare(of("-1", 0), of("0", 3)) }.0,
            -1
        );
        assert_eq!(unsafe { souther_decimal_is_zero(of("0", 9)) }, Bool::TRUE);
        assert_eq!(unsafe { souther_decimal_is_zero(of("1", 9)) }, Bool::FALSE);
        assert_eq!(
            unsafe { souther_string_to_decimal(made("001.50"), &mut out) },
            Bool::TRUE
        );
        assert_eq!(parts(out), ("150".to_string(), 2));
        let mut text = std::ptr::null_mut();
        assert_eq!(
            unsafe { souther_string_from_decimal(of("100000", 3), &mut text) },
            Bool::TRUE
        );
        assert_eq!(said(text), "100.000");
        souther_scope_close(scope);
    }

    /// A value whose plain notation is more text than a string holds is refused, before any of the
    /// text is written, at either end of the scale range; one whose text a string holds is not.
    #[test]
    fn the_text_of_a_value_no_string_holds_is_not_written() {
        let scope = souther_scope_open();
        let mut text = std::ptr::null_mut();
        for scale in [1_500_000_000, -1_500_000_000] {
            assert_eq!(
                unsafe { souther_string_from_decimal(of("1", scale), &mut text) },
                Bool::FALSE,
                "{scale}"
            );
        }
        assert_eq!(
            unsafe { souther_string_from_decimal(of("1", -3), &mut text) },
            Bool::TRUE
        );
        assert_eq!(said(text), "1000");
        souther_scope_close(scope);
    }

    /// Every unit the language declares that the table names has a token here, under the symbol a
    /// module's own would be defined under, and each is a place of its own.
    #[test]
    fn every_unit_the_language_declares_has_a_token_of_its_own() {
        let defined: BTreeSet<(&str, &str)> = LANGUAGE_UNIT_TOKENS
            .iter()
            .map(|(module, name, _)| (*module, *name))
            .collect();
        let tabled: BTreeSet<(&str, &str)> = LANGUAGE_UNITS.iter().copied().collect();
        assert_eq!(defined, tabled);
        assert_eq!(LANGUAGE_UNIT_TOKENS.len(), LANGUAGE_UNITS.len());
        let places: BTreeSet<*const u8> = LANGUAGE_UNIT_TOKENS
            .iter()
            .map(|(_, _, token)| token.as_ptr())
            .collect();
        assert_eq!(places.len(), LANGUAGE_UNITS.len());
        // The symbol the macro spells is the one `type_symbol` spells, which is what an object
        // naming the declaration imports.
        let source = include_str!("decimal.rs");
        for (module, name) in LANGUAGE_UNITS {
            let spelt = format!("\"{module}\".\"{name}\"");
            assert!(source.contains(&spelt), "{module}.{name}");
            assert_eq!(
                type_symbol(module, name),
                format!("souther$type${module}${name}")
            );
        }
    }

    /// Parts that name no `Decimal` are answered as that, and end nothing.
    #[test]
    fn parts_that_name_no_decimal_are_refused() {
        let scope = souther_scope_open();
        let mut out = ptr::null_mut();
        for (unscaled, scale) in [
            ("12.5", 0),
            ("", 0),
            ("1e3", 0),
            (" 1", 0),
            ("1", i64::from(i32::MAX) + 1),
            ("1", i64::from(i32::MIN) - 1),
        ] {
            let made = unsafe {
                souther_decimal_of_parts(
                    unscaled.as_ptr(),
                    Count(unscaled.len() as i64),
                    scale,
                    &mut out,
                )
            };
            assert_eq!(made, Bool::FALSE, "{unscaled:?} at {scale}");
        }
        let invalid = [0xff_u8, b'1'];
        assert_eq!(
            unsafe { souther_decimal_of_parts(invalid.as_ptr(), Count(2), 0, &mut out) },
            Bool::FALSE
        );
        assert_eq!(
            unsafe { souther_decimal_of_parts(invalid.as_ptr(), Count(-1), 0, &mut out) },
            Bool::FALSE
        );
        assert_eq!(parts(of("-120", 2)), (String::from("-120"), 2));
        souther_scope_close(scope);
    }
}
