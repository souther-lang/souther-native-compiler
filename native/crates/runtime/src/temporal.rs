//! A `Date`, a `Time`, a `DateTime` and an `Instant` as generated code and a host hold them: an
//! address, and behind it a layout only this file reads.
//!
//! Nothing outside the runtime reads behind the address. Generated code hands it to the functions
//! here and is handed one back, a value of a union carries it in a slot as it carries a string's,
//! and a host makes one of the text that names it and reads that text back. So how one is kept is
//! not part of any contract, and the layout below can change without an object or a binding being
//! built again.
//!
//! What each operation answers is what `java.time` answers for the same program (spec
//! §stdlib-date, §primitives), which the language states in terms of: a `Date` is a `LocalDate`, a
//! `Time` a `LocalTime` held to the second, a `DateTime` a `LocalDateTime` held to the second, and
//! an `Instant` a `java.time.Instant`. The calendar is the proleptic ISO one, and each type holds
//! what its `java.time` counterpart holds and nothing past it.
//!
//! The layout, in the arena and a slot to each number: a `Date` is its day counted from
//! 1970-01-01; a `Time` is its second of the day; a `DateTime` is the second it names counted from
//! 1970-01-01T00:00:00 as though it were in UTC, which no zone is a claim of, and which is what
//! makes two of them compare by the number; an `Instant` is its second counted the same way and
//! then its nanosecond of that second, which is never negative.

use crate::external::{Form, handed};
use crate::kernels::answered;
use crate::{Comparison, Count, Text, souther_alloc, string_of, text};
use souther_native_abi::SLOT;
use souther_text::temporal::{
    MAX_DAY, MAX_LOCAL, MAX_YEAR, MIN_DAY, MIN_LOCAL, MIN_YEAR, SECONDS_PER_DAY, civil_from_days,
    date_text, date_time_text, days_from_civil, instant_text, month_length, parse_date,
    parse_date_time, parse_instant, parse_time, time_text,
};
use std::cmp::Ordering;

/// A `Date`, as the functions here take and answer one: an address, a type of its own for the
/// reason [`Text`] is.
#[repr(C)]
pub struct Date {
    _opaque: [u8; 0],
}

/// A `Time`.
#[repr(C)]
pub struct Time {
    _opaque: [u8; 0],
}

/// A `DateTime`.
#[repr(C)]
pub struct DateTime {
    _opaque: [u8; 0],
}

/// An `Instant`.
#[repr(C)]
pub struct Instant {
    _opaque: [u8; 0],
}

/// Room in the arena holding these numbers, a slot to each.
fn stored<T>(numbers: &[i64]) -> *mut T {
    let at = souther_alloc(Count(numbers.len() as i64 * SLOT));
    for (index, number) in numbers.iter().enumerate() {
        // SAFETY: the room was just taken, as wide as the numbers and aligned to a slot as every
        // room the arena hands out is.
        unsafe { at.add(index * SLOT as usize).cast::<i64>().write(*number) };
    }
    at.cast()
}

/// The number at a slot of a temporal.
///
/// # Safety
///
/// `at` is a temporal the runtime answered, with a slot at `index`, and the mark below it still
/// stands. So for every function here that reads one.
unsafe fn number<T>(at: *const T, index: usize) -> i64 {
    unsafe {
        at.cast::<u8>()
            .add(index * SLOT as usize)
            .cast::<i64>()
            .read()
    }
}

fn date_of(day: i64) -> *mut Date {
    stored(&[day])
}

fn time_of(second_of_day: i64) -> *mut Time {
    stored(&[second_of_day])
}

fn date_time_of(second: i64) -> *mut DateTime {
    stored(&[second])
}

fn instant_of(second: i64, nano: i64) -> *mut Instant {
    stored(&[second, nano])
}

/// The day a `Date` is.
unsafe fn day(at: *const Date) -> i64 {
    unsafe { number(at, 0) }
}

/// The second of the day a `Time` is.
unsafe fn second_of_day(at: *const Time) -> i64 {
    unsafe { number(at, 0) }
}

/// The second a `DateTime` is.
unsafe fn local_second(at: *const DateTime) -> i64 {
    unsafe { number(at, 0) }
}

/// The second and the nanosecond an `Instant` is.
unsafe fn moment(at: *const Instant) -> (i64, i64) {
    unsafe { (number(at, 0), number(at, 1)) }
}

// What a host, a literal and a boundary make one of and read one as.

/// The text of a string of the runtime's layout.
///
/// # Safety
///
/// As [`crate::souther_string_compare`].
unsafe fn written<'a>(at: &'a *const Text) -> souther_text::Text<'a> {
    unsafe { text(at) }
}

/// A `Date` of the text that names one, for a caller outside a Souther program and for a literal
/// the checker has read.
///
/// # Safety
///
/// `iso` is a string of the runtime's layout.
///
/// # Panics
///
/// Where the text names no `Date`, which ends the process as a `Decimal`'s integer that is no
/// integer does: a binding says so first in its own terms.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_of_iso(iso: *const Text) -> *mut Date {
    let day =
        parse_date(unsafe { written(&iso) }).expect("a Date is handed over as text that names one");
    date_of(day)
}

/// A `Date` literal, as the checker read it.
///
/// # Safety
///
/// As [`souther_date_of_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_literal(iso: *const Text) -> *mut Date {
    unsafe { souther_date_of_iso(iso) }
}

/// The text a `Date` is written as: what a boundary writes.
///
/// # Safety
///
/// `at` is a `Date` the runtime answered, and the mark below it still stands. So for every function
/// here that reads one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_iso(at: *const Date) -> *mut Text {
    string_of(&date_text(unsafe { day(at) }))
}

/// A `Time` of the text that names one.
///
/// # Safety
///
/// As [`souther_date_of_iso`].
///
/// # Panics
///
/// Where the text names no `Time`, or names one with a fraction of a second.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_time_of_iso(iso: *const Text) -> *mut Time {
    let second =
        parse_time(unsafe { written(&iso) }).expect("a Time is handed over as text that names one");
    time_of(second)
}

/// A `Time` literal, as the checker read it.
///
/// # Safety
///
/// As [`souther_date_of_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_time_literal(iso: *const Text) -> *mut Time {
    unsafe { souther_time_of_iso(iso) }
}

/// The text a `Time` is written as.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_time_iso(at: *const Time) -> *mut Text {
    string_of(&time_text(unsafe { second_of_day(at) }))
}

/// A `DateTime` of the text that names one.
///
/// # Safety
///
/// As [`souther_time_of_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_of_iso(iso: *const Text) -> *mut DateTime {
    let second = parse_date_time(unsafe { written(&iso) })
        .expect("a DateTime is handed over as text that names one");
    date_time_of(second)
}

/// A `DateTime` literal, as the checker read it.
///
/// # Safety
///
/// As [`souther_date_of_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_literal(iso: *const Text) -> *mut DateTime {
    unsafe { souther_datetime_of_iso(iso) }
}

/// The text a `DateTime` is written as.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_iso(at: *const DateTime) -> *mut Text {
    string_of(&date_time_text(unsafe { local_second(at) }))
}

/// An `Instant` of the text that names one, in UTC or from an offset.
///
/// # Safety
///
/// As [`souther_date_of_iso`].
///
/// # Panics
///
/// Where the text names no `Instant`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_instant_of_iso(iso: *const Text) -> *mut Instant {
    let (second, nano) = parse_instant(unsafe { written(&iso) })
        .expect("an Instant is handed over as text that names one");
    instant_of(second, nano)
}

/// An `Instant` literal, as the checker read it.
///
/// # Safety
///
/// As [`souther_date_of_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_instant_literal(iso: *const Text) -> *mut Instant {
    unsafe { souther_instant_of_iso(iso) }
}

/// The text an `Instant` is written as: in UTC.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_instant_iso(at: *const Instant) -> *mut Text {
    let (second, nano) = unsafe { moment(at) };
    string_of(&instant_text(second, nano))
}

/// Two `Date`s in order: `==`, `<` and the rest are read off the one answer, as they are for text.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_compare(left: *const Date, right: *const Date) -> Comparison {
    Comparison(unsafe { day(left).cmp(&day(right)) } as i64)
}

/// Two `Time`s in order.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_time_compare(left: *const Time, right: *const Time) -> Comparison {
    Comparison(unsafe { second_of_day(left).cmp(&second_of_day(right)) } as i64)
}

/// Two `DateTime`s in order.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_compare(
    left: *const DateTime,
    right: *const DateTime,
) -> Comparison {
    Comparison(unsafe { local_second(left).cmp(&local_second(right)) } as i64)
}

/// Two `Instant`s in order, to the nanosecond.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_instant_compare(
    left: *const Instant,
    right: *const Instant,
) -> Comparison {
    let ordering: Ordering = unsafe { moment(left).cmp(&moment(right)) };
    Comparison(ordering as i64)
}

/// `Date.addDays`, written through `out` where the day it names is one a `Date` holds.
///
/// A shift that does not is what `java.time` refuses with an exception the language turns into an
/// abort (spec §a-shift-off-the-end-of-a-temporal-aborts): a count too large to add is the same
/// as a day past the end, and is not told apart from it.
///
/// # Safety
///
/// As [`souther_date_iso`], and `out` is room for the address of a `Date`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_add_days(
    days: i64,
    of: *const Date,
    out: *mut *mut Date,
) -> i8 {
    let shifted = unsafe { day(of) }
        .checked_add(days)
        .filter(|it| (MIN_DAY..=MAX_DAY).contains(it));
    unsafe { answered(shifted.map(date_of), out) }
}

/// `Date.addMonths`: the same day of a month later, or the last of it where that month is shorter.
///
/// # Safety
///
/// As [`souther_date_add_days`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_add_months(
    months: i64,
    of: *const Date,
    out: *mut *mut Date,
) -> i8 {
    let (year, month, date) = civil_from_days(unsafe { day(of) });
    let shifted = (year * 12 + month - 1)
        .checked_add(months)
        .and_then(|counted| {
            let (year, month) = (counted.div_euclid(12), counted.rem_euclid(12) + 1);
            (MIN_YEAR..=MAX_YEAR)
                .contains(&year)
                .then(|| days_from_civil(year, month, date.min(month_length(year, month))))
        });
    unsafe { answered(shifted.map(date_of), out) }
}

/// `Date.addYears`, the twenty-ninth of February becoming the twenty-eighth where the year is not
/// a leap one.
///
/// # Safety
///
/// As [`souther_date_add_days`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_add_years(
    years: i64,
    of: *const Date,
    out: *mut *mut Date,
) -> i8 {
    let (year, month, date) = civil_from_days(unsafe { day(of) });
    let shifted = year
        .checked_add(years)
        .filter(|it| (MIN_YEAR..=MAX_YEAR).contains(it))
        .map(|year| days_from_civil(year, month, date.min(month_length(year, month))));
    unsafe { answered(shifted.map(date_of), out) }
}

/// `Date.daysBetween`: whole days from the first to the second, below nought where the second is
/// the earlier.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_days_between(from: *const Date, to: *const Date) -> i64 {
    unsafe { day(to) - day(from) }
}

/// `Date.year`.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_year(of: *const Date) -> i64 {
    civil_from_days(unsafe { day(of) }).0
}

/// `Date.month`, from 1 to 12.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_month(of: *const Date) -> i64 {
    civil_from_days(unsafe { day(of) }).1
}

/// `Date.day`, from 1 to 31.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_day(of: *const Date) -> i64 {
    civil_from_days(unsafe { day(of) }).2
}

/// `Date.fromParts`, written through `out` where the three numbers name a day. Nothing is
/// normalised: the thirtieth of February and a year no `Date` holds name none.
///
/// # Safety
///
/// `out` is room for the address of a `Date`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_date_from_parts(
    year: i64,
    month: i64,
    date: i64,
    out: *mut *mut Date,
) -> i8 {
    let named = ((MIN_YEAR..=MAX_YEAR).contains(&year)
        && (1..=12).contains(&month)
        && (1..=month_length(year, month)).contains(&date))
    .then(|| date_of(days_from_civil(year, month, date)));
    unsafe { answered(named, out) }
}

/// `Time.fromParts`, written through `out` where the three numbers name a time of day: hour 24,
/// minute 60 and the leap second name none.
///
/// # Safety
///
/// `out` is room for the address of a `Time`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_time_from_parts(
    hour: i64,
    minute: i64,
    second: i64,
    out: *mut *mut Time,
) -> i8 {
    let named =
        ((0..=23).contains(&hour) && (0..=59).contains(&minute) && (0..=59).contains(&second))
            .then(|| time_of(hour * 3600 + minute * 60 + second));
    unsafe { answered(named, out) }
}

/// `Time.hour`, from 0 to 23.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_time_hour(of: *const Time) -> i64 {
    unsafe { second_of_day(of) / 3600 }
}

/// `Time.minute`, from 0 to 59.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_time_minute(of: *const Time) -> i64 {
    unsafe { second_of_day(of) % 3600 / 60 }
}

/// `Time.second`, from 0 to 59.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_time_second(of: *const Time) -> i64 {
    unsafe { second_of_day(of) % 60 }
}

/// A shift of a `DateTime` by so many seconds a unit is, written through `out` where what it
/// shifts to is one a `DateTime` holds.
unsafe fn shifted(of: *const DateTime, unit: i64, count: i64, out: *mut *mut DateTime) -> i8 {
    let second = count
        .checked_mul(unit)
        .and_then(|seconds| unsafe { local_second(of) }.checked_add(seconds))
        .filter(|it| (MIN_LOCAL..=MAX_LOCAL).contains(it));
    unsafe { answered(second.map(date_time_of), out) }
}

/// `DateTime.addMinutes`, written through `out` where the result is one a `DateTime` holds.
///
/// # Safety
///
/// As [`souther_date_iso`], and `out` is room for the address of a `DateTime`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_add_minutes(
    minutes: i64,
    of: *const DateTime,
    out: *mut *mut DateTime,
) -> i8 {
    unsafe { shifted(of, 60, minutes, out) }
}

/// `DateTime.addHours`, as [`souther_datetime_add_minutes`].
///
/// # Safety
///
/// As [`souther_datetime_add_minutes`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_add_hours(
    hours: i64,
    of: *const DateTime,
    out: *mut *mut DateTime,
) -> i8 {
    unsafe { shifted(of, 3600, hours, out) }
}

/// `DateTime.addDays`, as [`souther_datetime_add_minutes`].
///
/// # Safety
///
/// As [`souther_datetime_add_minutes`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_add_days(
    days: i64,
    of: *const DateTime,
    out: *mut *mut DateTime,
) -> i8 {
    unsafe { shifted(of, SECONDS_PER_DAY, days, out) }
}

/// `DateTime.minutesBetween`: whole minutes from the first to the second, below nought where the
/// second is the earlier, and the part of a minute that is left dropped towards nought.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_minutes_between(
    from: *const DateTime,
    to: *const DateTime,
) -> i64 {
    unsafe { (local_second(to) - local_second(from)) / 60 }
}

/// `DateTime.toDate`.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_to_date(of: *const DateTime) -> *mut Date {
    date_of(unsafe { local_second(of) }.div_euclid(SECONDS_PER_DAY))
}

/// `DateTime.toTime`.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_to_time(of: *const DateTime) -> *mut Time {
    time_of(unsafe { local_second(of) }.rem_euclid(SECONDS_PER_DAY))
}

/// `DateTime.fromDateAndTime`, which cannot fail: whatever could be wrong about the parts was
/// settled when they were made.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_datetime_from_date_and_time(
    date: *const Date,
    time: *const Time,
) -> *mut DateTime {
    date_time_of(unsafe { day(date) } * SECONDS_PER_DAY + unsafe { second_of_day(time) })
}

/// A temporal at a boundary: the text it is written as.
fn external(written: String) -> *mut Form {
    handed(Form::String(written.into_bytes()))
}

/// A `Date` at a boundary.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_date(at: *const Date) -> *mut Form {
    external(date_text(unsafe { day(at) }))
}

/// A `Time` at a boundary.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_time(at: *const Time) -> *mut Form {
    external(time_text(unsafe { second_of_day(at) }))
}

/// A `DateTime` at a boundary.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_datetime(at: *const DateTime) -> *mut Form {
    external(date_time_text(unsafe { local_second(at) }))
}

/// An `Instant` at a boundary, in UTC whatever offset it was read from.
///
/// # Safety
///
/// As [`souther_date_iso`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn souther_external_instant(at: *const Instant) -> *mut Form {
    let (second, nano) = unsafe { moment(at) };
    external(instant_text(second, nano))
}

/// A `Date` that a boundary read, made in the arena.
pub(crate) fn read_date_of(day: i64) -> *mut Date {
    date_of(day)
}

/// A `Time` that a boundary read, made in the arena.
pub(crate) fn read_time_of(second_of_day: i64) -> *mut Time {
    time_of(second_of_day)
}

/// A `DateTime` that a boundary read, made in the arena.
pub(crate) fn read_date_time_of(second: i64) -> *mut DateTime {
    date_time_of(second)
}

/// An `Instant` that a boundary read, made in the arena.
pub(crate) fn read_instant_of(second: i64, nano: i64) -> *mut Instant {
    instant_of(second, nano)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{souther_mark, souther_reset, souther_string_of_utf8};

    fn made(text: &str) -> *mut Text {
        unsafe { souther_string_of_utf8(text.as_ptr(), Count(text.len() as i64)) }
    }

    fn said(at: *const Text) -> String {
        String::from(unsafe { text(&at) }.as_str())
    }

    #[test]
    fn a_shift_runs_off_the_end_or_lands() {
        let mark = souther_mark();
        let mut out = std::ptr::null_mut();
        let jan31 = date_of(days_from_civil(2026, 1, 31));
        assert_eq!(unsafe { souther_date_add_months(1, jan31, &mut out) }, 1);
        assert_eq!(said(unsafe { souther_date_iso(out) }), "2026-02-28");
        assert_eq!(unsafe { souther_date_add_months(-13, jan31, &mut out) }, 1);
        assert_eq!(said(unsafe { souther_date_iso(out) }), "2024-12-31");
        let leap = date_of(days_from_civil(2024, 2, 29));
        assert_eq!(unsafe { souther_date_add_years(1, leap, &mut out) }, 1);
        assert_eq!(said(unsafe { souther_date_iso(out) }), "2025-02-28");
        assert_eq!(unsafe { souther_date_add_years(4, leap, &mut out) }, 1);
        assert_eq!(said(unsafe { souther_date_iso(out) }), "2028-02-29");
        let last = date_of(MAX_DAY);
        assert_eq!(unsafe { souther_date_add_days(1, last, &mut out) }, 0);
        assert_eq!(
            unsafe { souther_date_add_days(i64::MAX, last, &mut out) },
            0
        );
        assert_eq!(unsafe { souther_date_add_months(1, last, &mut out) }, 0);
        assert_eq!(unsafe { souther_date_add_years(1, last, &mut out) }, 0);
        assert_eq!(
            unsafe { souther_date_add_years(i64::MIN, last, &mut out) },
            0
        );
        assert_eq!(unsafe { souther_date_add_days(-1, last, &mut out) }, 1);
        let first = date_time_of(MIN_LOCAL);
        let mut moved = std::ptr::null_mut();
        assert_eq!(
            unsafe { souther_datetime_add_minutes(-1, first, &mut moved) },
            0
        );
        assert_eq!(
            unsafe { souther_datetime_add_hours(i64::MIN, first, &mut moved) },
            0
        );
        assert_eq!(
            unsafe { souther_datetime_add_days(1, first, &mut moved) },
            1
        );
        souther_reset(mark);
    }

    #[test]
    fn a_date_time_between_counts_whole_minutes_towards_nought() {
        let mark = souther_mark();
        let at = |second| date_time_of(second);
        let base = days_from_civil(2026, 7, 25) * SECONDS_PER_DAY;
        unsafe {
            assert_eq!(
                souther_datetime_minutes_between(at(base), at(base + 119)),
                1
            );
            assert_eq!(
                souther_datetime_minutes_between(at(base + 119), at(base)),
                -1
            );
            assert_eq!(souther_datetime_minutes_between(at(base), at(base + 59)), 0);
            assert_eq!(souther_datetime_minutes_between(at(base + 59), at(base)), 0);
            assert_eq!(
                souther_datetime_minutes_between(at(MIN_LOCAL), at(MAX_LOCAL)),
                (MAX_LOCAL - MIN_LOCAL) / 60
            );
        }
        souther_reset(mark);
    }

    #[test]
    fn a_date_is_its_parts_and_parts_that_name_none_are_refused() {
        let mark = souther_mark();
        let mut out = std::ptr::null_mut();
        unsafe {
            assert_eq!(souther_date_from_parts(2024, 2, 29, &mut out), 1);
            assert_eq!(
                (
                    souther_date_year(out),
                    souther_date_month(out),
                    souther_date_day(out)
                ),
                (2024, 2, 29)
            );
            for (year, month, date) in [
                (2023, 2, 29),
                (2026, 13, 1),
                (2026, 0, 1),
                (2026, 1, 0),
                (2026, 4, 31),
                (1_000_000_000, 1, 1),
                (i64::MAX, 1, 1),
                (2026, i64::MAX, 1),
            ] {
                assert_eq!(souther_date_from_parts(year, month, date, &mut out), 0);
            }
            let mut time = std::ptr::null_mut();
            assert_eq!(souther_time_from_parts(23, 59, 59, &mut time), 1);
            assert_eq!(
                (
                    souther_time_hour(time),
                    souther_time_minute(time),
                    souther_time_second(time)
                ),
                (23, 59, 59)
            );
            for (hour, minute, second) in [(24, 0, 0), (0, 60, 0), (0, 0, 60), (-1, 0, 0)] {
                assert_eq!(souther_time_from_parts(hour, minute, second, &mut time), 0);
            }
        }
        souther_reset(mark);
    }

    /// A value a host made of text reads back as the text it is written as, and two made apart
    /// compare equal where they name one moment.
    #[test]
    fn a_host_reads_back_the_text_a_value_is_written_as() {
        let mark = souther_mark();
        unsafe {
            assert_eq!(
                said(souther_date_iso(souther_date_of_iso(made("2026-07-25")))),
                "2026-07-25"
            );
            assert_eq!(
                said(souther_time_iso(souther_time_of_iso(made("09:30:00")))),
                "09:30"
            );
            assert_eq!(
                said(souther_datetime_iso(souther_datetime_of_iso(made(
                    "2026-07-25T09:30:05"
                )))),
                "2026-07-25T09:30:05"
            );
            let apart = souther_instant_of_iso(made("2026-07-25T09:30:00+09:00"));
            let together = souther_instant_of_iso(made("2026-07-25T00:30:00Z"));
            assert_eq!(souther_instant_compare(apart, together), Comparison(0));
            assert_eq!(said(souther_instant_iso(apart)), "2026-07-25T00:30:00Z");
            let later = souther_instant_of_iso(made("2026-07-25T00:30:00.000000001Z"));
            assert_eq!(souther_instant_compare(together, later), Comparison(-1));
            assert_eq!(souther_instant_compare(later, together), Comparison(1));
        }
        souther_reset(mark);
    }
}
