//! What a `Date`, a `Time`, a `DateTime` and an `Instant` are as text, and the calendar that says
//! which day a count of days is (spec §primitives, §temporal-literal).
//!
//! The text is ISO 8601 as `java.time` writes it, and what is read is what Raoh accepts for each
//! type, a regular language over ASCII: everything `toString` of the counterpart writes belongs to
//! it. A `Date` is a `LocalDate`, a `Time` a `LocalTime` held to the second, a `DateTime` a
//! `LocalDateTime` held to the second, and an `Instant` a `java.time.Instant`, each holding what
//! its counterpart holds and nothing past it.
//!
//! Numbers and text only, as the rest of this crate is: a day is a count of days from 1970-01-01,
//! a time of day a count of seconds, a `DateTime` a count of seconds as though it were in UTC
//! (which no zone is a claim of, and which is what makes two of them compare by the number), and an
//! `Instant` that count and a nanosecond. Where a value is kept is the runtime's, and what an
//! operation on one answers is written beside where it is kept.

use crate::Text;
use alloc::format;
use alloc::string::String;

pub const SECONDS_PER_DAY: i64 = 86_400;

/// The years a `Date` holds, which are the ones `java.time.Year` does.
pub const MIN_YEAR: i64 = -999_999_999;
pub const MAX_YEAR: i64 = 999_999_999;

/// The days from 1970-01-01 to the first of March of the year the civil calendar counts from, and
/// the rest of the arithmetic below, which is the proleptic Gregorian calendar worked out over
/// whole eras of four hundred years (Howard Hinnant's `days_from_civil` and `civil_from_days`).
pub const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The year, the month and the day a count of days from 1970-01-01 names.
pub const fn civil_from_days(days: i64) -> (i64, i64, i64) {
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

pub const fn is_leap(year: i64) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

pub const fn month_length(year: i64, month: i64) -> i64 {
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
pub const MIN_DAY: i64 = days_from_civil(MIN_YEAR, 1, 1);
pub const MAX_DAY: i64 = days_from_civil(MAX_YEAR, 12, 31);

/// The first and the last second a `DateTime` holds, counted as its layout counts.
pub const MIN_LOCAL: i64 = MIN_DAY * SECONDS_PER_DAY;
pub const MAX_LOCAL: i64 = MAX_DAY * SECONDS_PER_DAY + SECONDS_PER_DAY - 1;

/// The first and the last second an `Instant` holds. A year wider than a `Date` holds at each end,
/// as `java.time.Instant` has: `-1000000000-01-01T00:00:00Z` to `+1000000000-12-31T23:59:59.999999999Z`.
pub const MIN_MOMENT: i64 = days_from_civil(-1_000_000_000, 1, 1) * SECONDS_PER_DAY;
pub const MAX_MOMENT: i64 = (days_from_civil(1_000_000_000, 12, 31) + 1) * SECONDS_PER_DAY - 1;

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

pub fn date_text(day: i64) -> String {
    let (year, month, day) = civil_from_days(day);
    let mut written = String::new();
    write_year(year, &mut written);
    written.push_str(&format!("-{month:02}-{day:02}"));
    written
}

/// `HH:mm`, and `:ss` where the second is not nought, as `LocalTime.toString` writes a time held
/// to the second.
pub fn time_text(second_of_day: i64) -> String {
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

pub fn date_time_text(second: i64) -> String {
    format!(
        "{}T{}",
        date_text(second.div_euclid(SECONDS_PER_DAY)),
        time_text(second.rem_euclid(SECONDS_PER_DAY))
    )
}

/// In UTC, with the second always written and a fraction only where there is one, in as many
/// groups of three digits as it takes, as `Instant.toString` writes one.
pub fn instant_text(second: i64, nano: i64) -> String {
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
pub enum Refusal {
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
pub fn parse_date(text: Text<'_>) -> Option<i64> {
    let mut from = Reader::of(text.as_bytes());
    let (year, month, day) = read_date(&mut from)?;
    (from.finished() && (MIN_YEAR..=MAX_YEAR).contains(&year))
        .then(|| days_from_civil(year, month, day))
}

/// The second of the day the text names.
pub fn parse_time(text: Text<'_>) -> Result<i64, Refusal> {
    let mut from = Reader::of(text.as_bytes());
    let clock = read_clock(&mut from, false).ok_or(Refusal::Format)?;
    if !from.finished() {
        return Err(Refusal::Format);
    }
    local_time(&clock)
}

/// The second the text names, counted as a `DateTime` counts it.
pub fn parse_date_time(text: Text<'_>) -> Result<i64, Refusal> {
    let mut from = Reader::of(text.as_bytes());
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
pub fn parse_instant(text: Text<'_>) -> Option<(i64, i64)> {
    let mut from = Reader::of(text.as_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The bounds are the ones `java.time` states: `LocalDate.MIN`/`MAX` as epoch days,
    /// `Instant.MIN`/`MAX` as epoch seconds.
    #[test]
    fn the_bounds_are_what_java_time_holds() {
        assert_eq!(MIN_DAY, -365_243_219_162);
        assert_eq!(MAX_DAY, 365_241_780_471);
        assert_eq!(MIN_MOMENT, -31_557_014_167_219_200);
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
            assert_eq!(parse_date(Text::held(written)), Some(days), "{written}");
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
            assert_eq!(parse_date(Text::held(refused)), None, "{refused:?}");
        }
        assert!(parse_date(Text::held("2024-02-29")).is_some());
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
            assert_eq!(parse_time(Text::held(written)), Ok(second));
        }
        assert_eq!(parse_time(Text::held("09:30:00")), Ok(9 * 3600 + 30 * 60));
        assert_eq!(
            parse_time(Text::held("09:30:00.000")),
            Ok(9 * 3600 + 30 * 60)
        );
        assert_eq!(parse_time(Text::held("09:30:00.5")), Err(Refusal::Fraction));
        assert_eq!(
            parse_time(Text::held("09:30:00.000000001")),
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
                parse_time(Text::held(refused)),
                Err(Refusal::Format),
                "{refused:?}"
            );
        }
    }

    #[test]
    fn a_date_time_is_a_date_and_a_time() {
        let second = days_from_civil(2026, 7, 25) * SECONDS_PER_DAY + 9 * 3600 + 30 * 60;
        assert_eq!(date_time_text(second), "2026-07-25T09:30");
        assert_eq!(parse_date_time(Text::held("2026-07-25T09:30")), Ok(second));
        assert_eq!(
            parse_date_time(Text::held("2026-07-25T09:30:00")),
            Ok(second)
        );
        assert_eq!(
            parse_date_time(Text::held("2026-07-25T09:30:00.1")),
            Err(Refusal::Fraction)
        );
        assert_eq!(
            parse_date_time(Text::held("2026-07-25")),
            Err(Refusal::Format)
        );
        assert_eq!(
            parse_date_time(Text::held("2026-07-25t09:30")),
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
            let read = parse_instant(Text::held(written)).expect(written);
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
            parse_instant(Text::held("2026-07-25T09:30:00+09:00")),
            parse_instant(Text::held("2026-07-25T00:30:00Z"))
        );
        assert_eq!(
            parse_instant(Text::held("2026-07-25T00:30:00-01:30:15")),
            parse_instant(Text::held("2026-07-25T02:00:15Z"))
        );
        assert_eq!(
            parse_instant(Text::held("2026-07-25T24:00:00Z")),
            parse_instant(Text::held("2026-07-26T00:00:00Z"))
        );
        assert!(parse_instant(Text::held("2026-07-25T00:00:00+18:00")).is_some());
        assert_eq!(
            parse_instant(Text::held("2026-07-25T00:00:00+18:00:01")),
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
            assert_eq!(parse_instant(Text::held(refused)), None, "{refused:?}");
        }
        assert!(parse_instant(Text::held("+1000000000-12-31T23:59:59.999999999Z")).is_some());
        assert!(parse_instant(Text::held("-1000000000-01-01T00:00:00Z")).is_some());
        assert!(
            parse_instant(Text::held("+1000000000-12-31T23:59:59.999999999-00:00:01")).is_none()
        );
    }
}
