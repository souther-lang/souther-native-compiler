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
use souther_native_abi::{DATE_DAYS, DATE_TIME_SECONDS, INSTANT_SECONDS, SECONDS_PER_DAY, SLOT};
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

/// The years a `Date` holds, which are the ones `java.time.Year` does.
const MIN_YEAR: i64 = -999_999_999;
const MAX_YEAR: i64 = 999_999_999;

/// The days from 1970-01-01 to the first of March of the year the civil calendar counts from, and
/// the rest of the arithmetic below, which is the proleptic Gregorian calendar worked out over
/// whole eras of four hundred years (Howard Hinnant's `days_from_civil` and `civil_from_days`).
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The year, the month and the day a count of days from 1970-01-01 names.
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

const fn is_leap(year: i64) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

const fn month_length(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
    }
}

/// The first and the last day a `Date` holds, as a count of days from 1970-01-01.
const MIN_DAY: i64 = days_from_civil(MIN_YEAR, 1, 1);
const MAX_DAY: i64 = days_from_civil(MAX_YEAR, 12, 31);

/// The first and the last second a `DateTime` holds, counted as its layout counts.
const MIN_LOCAL: i64 = MIN_DAY * SECONDS_PER_DAY;
const MAX_LOCAL: i64 = MAX_DAY * SECONDS_PER_DAY + SECONDS_PER_DAY - 1;

/// The first and the last second an `Instant` holds. A year wider than a `Date` holds at each end,
/// as `java.time.Instant` has: `-1000000000-01-01T00:00:00Z` to `+1000000000-12-31T23:59:59.999999999Z`.
const MIN_MOMENT: i64 = days_from_civil(-1_000_000_000, 1, 1) * SECONDS_PER_DAY;
const MAX_MOMENT: i64 = (days_from_civil(1_000_000_000, 12, 31) + 1) * SECONDS_PER_DAY - 1;

// What is written: the text `toString` of the type's `java.time` counterpart answers, which is
// what a boundary writes and what the checker reads a literal as.

/// A year as `LocalDate.toString` writes it: at least four digits, a sign for a year past 9999 and
/// for one below nought.
fn write_year(year: i64, into: &mut String) {
    if year < 0 {
        into.push('-');
    } else if year > 9999 {
        into.push('+');
    }
    into.push_str(&format!("{:04}", year.unsigned_abs()));
}

fn date_text(day: i64) -> String {
    let (year, month, day) = civil_from_days(day);
    let mut written = String::new();
    write_year(year, &mut written);
    written.push_str(&format!("-{month:02}-{day:02}"));
    written
}

/// `HH:mm`, and `:ss` where the second is not nought, as `LocalTime.toString` writes a time held
/// to the second.
fn time_text(second_of_day: i64) -> String {
    let (hour, minute, second) = (
        second_of_day / 3600,
        second_of_day % 3600 / 60,
        second_of_day % 60,
    );
    if second == 0 {
        format!("{hour:02}:{minute:02}")
    } else {
        format!("{hour:02}:{minute:02}:{second:02}")
    }
}

fn date_time_text(second: i64) -> String {
    format!(
        "{}T{}",
        date_text(second.div_euclid(SECONDS_PER_DAY)),
        time_text(second.rem_euclid(SECONDS_PER_DAY))
    )
}

/// In UTC, with the second always written and a fraction only where there is one, in as many
/// groups of three digits as it takes, as `Instant.toString` writes one.
fn instant_text(second: i64, nano: i64) -> String {
    let of_day = second.rem_euclid(SECONDS_PER_DAY);
    let mut written = date_text(second.div_euclid(SECONDS_PER_DAY));
    written.push_str(&format!(
        "T{:02}:{:02}:{:02}",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    ));
    if nano != 0 {
        if nano % 1_000_000 == 0 {
            written.push_str(&format!(".{:03}", nano / 1_000_000));
        } else if nano % 1_000 == 0 {
            written.push_str(&format!(".{:06}", nano / 1_000));
        } else {
            written.push_str(&format!(".{nano:09}"));
        }
    }
    written.push('Z');
    written
}

// What is read: the text Raoh accepts for each type, which is a regular language over ASCII that
// this reads directly, and which everything `toString` of the counterpart writes belongs to.

/// Why text is not a value of a type that only some text is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Refusal {
    /// It is not written as the type is, or it names what the calendar does not have.
    Format,
    /// It names a time a `Time` or a `DateTime` would have to round: it carries a fraction of a
    /// second that is not nought. Neither drops it (spec §a-local-temporal-is-held-to-the-second).
    Fraction,
}

struct Reader<'a> {
    text: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn of(text: &'a [u8]) -> Self {
        Reader { text, at: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        let there = self.peek() == Some(byte);
        if there {
            self.at += 1;
        }
        there
    }

    /// Exactly `count` ASCII digits, as the number they write.
    fn digits(&mut self, count: usize) -> Option<i64> {
        let run = self.text.get(self.at..self.at + count)?;
        if !run.iter().all(u8::is_ascii_digit) {
            return None;
        }
        self.at += count;
        Some(
            run.iter()
                .fold(0, |sum, digit| sum * 10 + i64::from(digit - b'0')),
        )
    }

    /// Every ASCII digit that follows.
    fn run(&mut self) -> &'a [u8] {
        let start = self.at;
        while self.peek().is_some_and(|it| it.is_ascii_digit()) {
            self.at += 1;
        }
        &self.text[start..self.at]
    }

    fn finished(&self) -> bool {
        self.at == self.text.len()
    }
}

/// A year as `LocalDate.toString` writes it: four digits and no sign for 0000 to 9999, and
/// otherwise a sign and as many digits as it takes but no fewer than four and no leading zero
/// beyond them. So `+2024` and `-0000` are no year. Not held to what a `Date` holds: an `Instant`
/// has years past it.
fn read_year(from: &mut Reader) -> Option<i64> {
    let sign = match from.peek() {
        Some(b'+') => Some(false),
        Some(b'-') => Some(true),
        _ => None,
    };
    if sign.is_some() {
        from.at += 1;
    }
    let run = from.run();
    let long = (5..=10).contains(&run.len()) && run[0] != b'0';
    let fits = match sign {
        None => run.len() == 4,
        Some(false) => long,
        Some(true) => (run.len() == 4 && run != b"0000") || long,
    };
    if !fits {
        return None;
    }
    let year = run
        .iter()
        .fold(0, |sum, digit| sum * 10 + i64::from(digit - b'0'));
    Some(if sign == Some(true) { -year } else { year })
}

/// `yyyy-MM-dd`, as the year, the month and the day, of a day the calendar has.
fn read_date(from: &mut Reader) -> Option<(i64, i64, i64)> {
    let year = read_year(from)?;
    if !from.eat(b'-') {
        return None;
    }
    let month = from.digits(2)?;
    if !from.eat(b'-') {
        return None;
    }
    let day = from.digits(2)?;
    ((1..=12).contains(&month) && (1..=month_length(year, month)).contains(&day))
        .then_some((year, month, day))
}

/// `HH:mm`, and then `:ss` with as many as nine digits of fraction after a point. Whether the
/// numbers name a time is the caller's: an `Instant` takes hour 24, a `Time` does not.
struct Clock {
    hour: i64,
    minute: i64,
    second: i64,
    nano: i64,
}

fn read_clock(from: &mut Reader, seconds_required: bool) -> Option<Clock> {
    let hour = from.digits(2)?;
    if !from.eat(b':') {
        return None;
    }
    let minute = from.digits(2)?;
    let (mut second, mut nano) = (0, 0);
    if from.eat(b':') {
        second = from.digits(2)?;
        if from.eat(b'.') {
            let fraction = from.run();
            if !(1..=9).contains(&fraction.len()) {
                return None;
            }
            nano = fraction
                .iter()
                .chain(core::iter::repeat_n(&b'0', 9 - fraction.len()))
                .fold(0, |sum, digit| sum * 10 + i64::from(digit - b'0'));
        }
    } else if seconds_required {
        return None;
    }
    Some(Clock {
        hour,
        minute,
        second,
        nano,
    })
}

/// The time of day a local clock names, held to the second: none where the numbers name no time,
/// and a refusal where they name one with a fraction of a second.
fn local_time(clock: &Clock) -> Result<i64, Refusal> {
    if clock.hour > 23 || clock.minute > 59 || clock.second > 59 {
        return Err(Refusal::Format);
    }
    if clock.nano != 0 {
        return Err(Refusal::Fraction);
    }
    Ok(clock.hour * 3600 + clock.minute * 60 + clock.second)
}

/// The day the text names, which is one a `Date` holds.
pub(crate) fn parse_date(text: &[u8]) -> Option<i64> {
    let mut from = Reader::of(text);
    let (year, month, day) = read_date(&mut from)?;
    (from.finished() && (MIN_YEAR..=MAX_YEAR).contains(&year))
        .then(|| days_from_civil(year, month, day))
}

/// The second of the day the text names.
pub(crate) fn parse_time(text: &[u8]) -> Result<i64, Refusal> {
    let mut from = Reader::of(text);
    let clock = read_clock(&mut from, false).ok_or(Refusal::Format)?;
    if !from.finished() {
        return Err(Refusal::Format);
    }
    local_time(&clock)
}

/// The second the text names, counted as a `DateTime` counts it.
pub(crate) fn parse_date_time(text: &[u8]) -> Result<i64, Refusal> {
    let mut from = Reader::of(text);
    let (year, month, day) = read_date(&mut from).ok_or(Refusal::Format)?;
    if !(MIN_YEAR..=MAX_YEAR).contains(&year) || !from.eat(b'T') {
        return Err(Refusal::Format);
    }
    let clock = read_clock(&mut from, false).ok_or(Refusal::Format)?;
    if !from.finished() {
        return Err(Refusal::Format);
    }
    Ok(days_from_civil(year, month, day) * SECONDS_PER_DAY + local_time(&clock)?)
}

/// `Z`, or a sign and `HH:mm` and then `:ss`, as the seconds it is east of UTC. Held to what
/// `ZoneOffset` holds: eighteen hours either way.
fn read_offset(from: &mut Reader) -> Option<i64> {
    if from.eat(b'Z') {
        return Some(0);
    }
    let sign = if from.eat(b'+') {
        1
    } else if from.eat(b'-') {
        -1
    } else {
        return None;
    };
    let hours = from.digits(2)?;
    if !from.eat(b':') {
        return None;
    }
    let minutes = from.digits(2)?;
    let seconds = if from.eat(b':') { from.digits(2)? } else { 0 };
    let total = hours * 3600 + minutes * 60 + seconds;
    (minutes <= 59 && seconds <= 59 && total <= 18 * 3600).then_some(sign * total)
}

/// The moment the text names, as its second and its nanosecond in UTC: a date, a `T`, a time with
/// its seconds and perhaps a fraction of one, and where it is written from.
///
/// The end of a day, 24:00:00, is the start of the next. A leap second is no moment: the second
/// after 59 is refused and not read as the one before it, which would be a different moment than
/// the text says (spec §a-leap-second-is-no-moment).
pub(crate) fn parse_instant(text: &[u8]) -> Option<(i64, i64)> {
    let mut from = Reader::of(text);
    let (year, month, date) = read_date(&mut from)?;
    if !from.eat(b'T') {
        return None;
    }
    let clock = read_clock(&mut from, true)?;
    let offset = read_offset(&mut from)?;
    if !from.finished() || clock.minute > 59 || clock.second > 59 {
        return None;
    }
    let end_of_day = clock.hour == 24;
    if clock.hour > 24 || (end_of_day && (clock.minute, clock.second, clock.nano) != (0, 0, 0)) {
        return None;
    }
    let second = (days_from_civil(year, month, date) * SECONDS_PER_DAY)
        .checked_add(clock.hour * 3600 + clock.minute * 60 + clock.second)?
        .checked_sub(offset)?;
    (MIN_MOMENT..=MAX_MOMENT)
        .contains(&second)
        .then_some((second, clock.nano))
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

pub(crate) fn date_of(day: i64) -> *mut Date {
    stored(&[day])
}

pub(crate) fn time_of(second_of_day: i64) -> *mut Time {
    stored(&[second_of_day])
}

pub(crate) fn date_time_of(second: i64) -> *mut DateTime {
    stored(&[second])
}

pub(crate) fn instant_of(second: i64, nano: i64) -> *mut Instant {
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
unsafe fn written(at: &*const Text) -> &[u8] {
    unsafe { text(at) }.as_str().as_bytes()
}

/// A `Date` of the text that names one, for a caller outside a Souther program. A literal is not
/// made of text ([`souther_date_literal`]).
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

/// A `Date` literal, as the checker read it: its day, counted from 1970-01-01.
///
/// # Panics
///
/// Where the day is not one a `Date` holds, which the driver has already held the literal to
/// (`DATE_DAYS`): a literal the compiler wrote is never one.
#[unsafe(no_mangle)]
pub extern "C" fn souther_date_literal(day: i64) -> *mut Date {
    assert!(
        DATE_DAYS.contains(&day),
        "a Date literal is a day a Date holds"
    );
    date_of(day)
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

/// A `Time` literal, as the checker read it: its second of the day.
///
/// # Panics
///
/// Where the second is not one a day has, as [`souther_date_literal`].
#[unsafe(no_mangle)]
pub extern "C" fn souther_time_literal(second: i64) -> *mut Time {
    assert!(
        (0..SECONDS_PER_DAY).contains(&second),
        "a Time literal is a second a day has"
    );
    time_of(second)
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

/// A `DateTime` literal, as the checker read it: its second, counted from 1970-01-01T00:00:00 as
/// though it were in UTC.
///
/// # Panics
///
/// Where the second is not one a `DateTime` holds, as [`souther_date_literal`].
#[unsafe(no_mangle)]
pub extern "C" fn souther_datetime_literal(second: i64) -> *mut DateTime {
    assert!(
        DATE_TIME_SECONDS.contains(&second),
        "a DateTime literal is a second a DateTime holds"
    );
    date_time_of(second)
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

/// An `Instant` literal, as the checker read it: its second from the epoch and the nanosecond within
/// it.
///
/// # Panics
///
/// Where either is not one an `Instant` holds, as [`souther_date_literal`].
#[unsafe(no_mangle)]
pub extern "C" fn souther_instant_literal(second: i64, nano: i64) -> *mut Instant {
    assert!(
        INSTANT_SECONDS.contains(&second) && (0..1_000_000_000).contains(&nano),
        "an Instant literal is a moment an Instant holds"
    );
    instant_of(second, nano)
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

    /// The bounds are the ones `java.time` states: `LocalDate.MIN`/`MAX` as epoch days,
    /// `Instant.MIN`/`MAX` as epoch seconds.
    #[test]
    fn the_calendar_holds_what_the_bounds_the_driver_holds_a_literal_to_say() {
        // Two ends of one fact: the driver holds a literal to numbers `java.time` states
        // (`souther_native_abi`), and the runtime's calendar has to reach exactly them.
        assert_eq!(MIN_DAY..=MAX_DAY, DATE_DAYS);
        assert_eq!(MIN_LOCAL..=MAX_LOCAL, DATE_TIME_SECONDS);
        assert_eq!(MIN_MOMENT..=MAX_MOMENT, INSTANT_SECONDS);
        assert_eq!(MIN_DAY, -365_243_219_162);
        assert_eq!(MAX_MOMENT, 31_556_889_864_403_199);
    }

    #[test]
    fn a_day_counted_from_the_epoch_is_the_date_it_names_and_back() {
        for days in [MIN_DAY, -719_468, -1, 0, 1, 10_956, 11_016, 20_659, MAX_DAY] {
            let (year, month, date) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, date), days, "{days}");
            assert!((1..=12).contains(&month) && (1..=month_length(year, month)).contains(&date));
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 7, 25), 20_659);
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn a_date_is_written_as_local_date_writes_one() {
        for (days, written) in [
            (days_from_civil(2026, 7, 25), "2026-07-25"),
            (days_from_civil(0, 1, 1), "0000-01-01"),
            (days_from_civil(-1, 12, 31), "-0001-12-31"),
            (days_from_civil(9999, 12, 31), "9999-12-31"),
            (days_from_civil(10_000, 1, 1), "+10000-01-01"),
            (days_from_civil(-10_000, 1, 1), "-10000-01-01"),
            (MIN_DAY, "-999999999-01-01"),
            (MAX_DAY, "+999999999-12-31"),
        ] {
            assert_eq!(date_text(days), written);
            assert_eq!(parse_date(written.as_bytes()), Some(days), "{written}");
        }
    }

    #[test]
    fn text_that_is_no_date_is_refused() {
        for refused in [
            "2026-7-25",
            "2026-02-30",
            "2023-02-29",
            "2026-13-01",
            "2026-00-10",
            "+2026-07-25",
            "-0000-01-01",
            "02026-07-25",
            "+0999999999-01-01",
            "+1000000000-01-01",
            "2026-07-25T00:00",
            " 2026-07-25",
            "2026-07-25 ",
            "",
        ] {
            assert_eq!(parse_date(refused.as_bytes()), None, "{refused:?}");
        }
        assert!(parse_date("2024-02-29".as_bytes()).is_some());
    }

    /// A time is held to the second, and its seconds are written only where there are some.
    #[test]
    fn a_time_is_written_as_local_time_writes_one_held_to_the_second() {
        for (second, written) in [
            (0, "00:00"),
            (9 * 3600 + 30 * 60, "09:30"),
            (9 * 3600 + 30 * 60 + 5, "09:30:05"),
            (86_399, "23:59:59"),
        ] {
            assert_eq!(time_text(second), written);
            assert_eq!(parse_time(written.as_bytes()), Ok(second));
        }
        assert_eq!(parse_time("09:30:00".as_bytes()), Ok(9 * 3600 + 30 * 60));
        assert_eq!(
            parse_time("09:30:00.000".as_bytes()),
            Ok(9 * 3600 + 30 * 60)
        );
        assert_eq!(parse_time("09:30:00.5".as_bytes()), Err(Refusal::Fraction));
        assert_eq!(
            parse_time("09:30:00.000000001".as_bytes()),
            Err(Refusal::Fraction)
        );
        for refused in [
            "24:00",
            "9:30",
            "09:60",
            "09:30:60",
            "09:30:5",
            "09:30:00.",
            "09:30:00.1234567890",
            "09:30.5",
            "T09:30",
            "09:30Z",
        ] {
            assert_eq!(
                parse_time(refused.as_bytes()),
                Err(Refusal::Format),
                "{refused:?}"
            );
        }
    }

    #[test]
    fn a_date_time_is_a_date_and_a_time() {
        let second = days_from_civil(2026, 7, 25) * SECONDS_PER_DAY + 9 * 3600 + 30 * 60;
        assert_eq!(date_time_text(second), "2026-07-25T09:30");
        assert_eq!(parse_date_time("2026-07-25T09:30".as_bytes()), Ok(second));
        assert_eq!(
            parse_date_time("2026-07-25T09:30:00".as_bytes()),
            Ok(second)
        );
        assert_eq!(
            parse_date_time("2026-07-25T09:30:00.1".as_bytes()),
            Err(Refusal::Fraction)
        );
        assert_eq!(
            parse_date_time("2026-07-25".as_bytes()),
            Err(Refusal::Format)
        );
        assert_eq!(
            parse_date_time("2026-07-25t09:30".as_bytes()),
            Err(Refusal::Format)
        );
        let before = days_from_civil(1969, 12, 31) * SECONDS_PER_DAY + 86_399;
        assert_eq!(date_time_text(before), "1969-12-31T23:59:59");
        assert_eq!(date_time_text(MIN_LOCAL), "-999999999-01-01T00:00");
        assert_eq!(date_time_text(MAX_LOCAL), "+999999999-12-31T23:59:59");
    }

    #[test]
    fn an_instant_is_written_in_utc_to_the_nanosecond_it_holds() {
        for (written, second, nano) in [
            ("1970-01-01T00:00:00Z", 0, 0),
            ("2026-07-25T00:00:00Z", 20_659 * SECONDS_PER_DAY, 0),
            (
                "2026-07-25T00:00:00.5Z",
                20_659 * SECONDS_PER_DAY,
                500_000_000,
            ),
            ("1969-12-31T23:59:59.999999999Z", -1, 999_999_999),
        ] {
            let read = parse_instant(written.as_bytes()).expect(written);
            assert_eq!(read, (second, nano), "{written}");
        }
        assert_eq!(instant_text(0, 0), "1970-01-01T00:00:00Z");
        assert_eq!(instant_text(0, 500_000_000), "1970-01-01T00:00:00.500Z");
        assert_eq!(instant_text(0, 120_000), "1970-01-01T00:00:00.000120Z");
        assert_eq!(instant_text(0, 1), "1970-01-01T00:00:00.000000001Z");
        assert_eq!(instant_text(MIN_MOMENT, 0), "-1000000000-01-01T00:00:00Z");
        assert_eq!(
            instant_text(MAX_MOMENT, 999_999_999),
            "+1000000000-12-31T23:59:59.999999999Z"
        );
    }

    /// An offset is a different spelling of a moment, and is read as the moment.
    #[test]
    fn an_offset_names_the_moment_the_z_form_of_it_names() {
        assert_eq!(
            parse_instant("2026-07-25T09:30:00+09:00".as_bytes()),
            parse_instant("2026-07-25T00:30:00Z".as_bytes())
        );
        assert_eq!(
            parse_instant("2026-07-25T00:30:00-01:30:15".as_bytes()),
            parse_instant("2026-07-25T02:00:15Z".as_bytes())
        );
        assert_eq!(
            parse_instant("2026-07-25T24:00:00Z".as_bytes()),
            parse_instant("2026-07-26T00:00:00Z".as_bytes())
        );
        assert!(parse_instant("2026-07-25T00:00:00+18:00".as_bytes()).is_some());
        assert_eq!(
            parse_instant("2026-07-25T00:00:00+18:00:01".as_bytes()),
            None
        );
    }

    #[test]
    fn text_that_is_no_moment_is_refused() {
        for refused in [
            // A leap second is no moment, at any offset.
            "2016-12-31T23:59:60Z",
            "2016-12-31T23:59:60+09:00",
            "2026-07-25T00:00Z",
            "2026-07-25T00:00:00",
            "2026-07-25T00:00:00z",
            "2026-07-25t00:00:00Z",
            "2026-07-25T00:00:00+09",
            "2026-07-25T00:00:00+09:60",
            "2026-07-25T00:00:00+19:00",
            "2026-07-25T25:00:00Z",
            "2026-07-25T24:00:01Z",
            "2026-07-25T24:00:00.1Z",
            "2026-07-25T00:00:00.Z",
            "2026-07-25T00:00:00.1234567890Z",
            "+1000000000-12-31T23:59:59.999999999+00:00 ",
            "+1000000001-01-01T00:00:00Z",
            "-1000000001-12-31T23:59:59Z",
            "2026-02-30T00:00:00Z",
        ] {
            assert_eq!(parse_instant(refused.as_bytes()), None, "{refused:?}");
        }
        assert!(parse_instant("+1000000000-12-31T23:59:59.999999999Z".as_bytes()).is_some());
        assert!(parse_instant("-1000000000-01-01T00:00:00Z".as_bytes()).is_some());
        assert!(
            parse_instant("+1000000000-12-31T23:59:59.999999999-00:00:01".as_bytes()).is_none()
        );
    }
}
