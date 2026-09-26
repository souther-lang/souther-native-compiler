//! A Souther `Date`, `Time`, `DateTime` and `Instant` as Rust holds one.
//!
//! The library takes one as the text that names it and ends the process on text that names none,
//! so each is held here as its numbers, checked where it is made against what the type holds, and
//! handed over as the text `java.time` writes for it, which the library reads. What the library
//! answers is read back out of the text it writes, which is that same form.

use std::fmt;

/// The years a `Date` holds, which are the ones `java.time.Year` does.
const MIN_YEAR: i64 = -999_999_999;
const MAX_YEAR: i64 = 999_999_999;

const SECONDS_PER_DAY: i64 = 86_400;

/// The first and the last second an `Instant` holds, as `java.time.Instant` has them:
/// `-1000000000-01-01T00:00:00Z` to `+1000000000-12-31T23:59:59.999999999Z`.
const MIN_MOMENT: i64 = days_from_civil(-1_000_000_000, 1, 1) * SECONDS_PER_DAY;
const MAX_MOMENT: i64 = (days_from_civil(1_000_000_000, 12, 31) + 1) * SECONDS_PER_DAY - 1;

/// The days from 1970-01-01 to a day of the proleptic Gregorian calendar (Howard Hinnant's
/// `days_from_civil`), as the library's runtime counts them.
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

/// A year as `LocalDate.toString` writes it: at least four digits, and a sign past 9999 and below
/// nought.
fn write_year(year: i64, into: &mut String) {
    if year < 0 {
        into.push('-');
    } else if year > 9999 {
        into.push('+');
    }
    into.push_str(&format!("{:04}", year.unsigned_abs()));
}

fn date_text(year: i64, month: i64, day: i64) -> String {
    let mut written = String::new();
    write_year(year, &mut written);
    written.push_str(&format!("-{month:02}-{day:02}"));
    written
}

/// Numbers that name no value of a temporal type, and which one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotATemporal(pub String);

impl fmt::Display for NotATemporal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotATemporal {}

/// A Souther `Date`: a day of the proleptic ISO calendar, in a year a `java.time.LocalDate` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

impl Date {
    /// The `Date` of `year`, `month` and `day`.
    ///
    /// # Errors
    ///
    /// [`NotATemporal`] where the calendar has no such day, or the year is past what a `Date`
    /// holds.
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, NotATemporal> {
        let (y, m, d) = (i64::from(year), i64::from(month), i64::from(day));
        if !(MIN_YEAR..=MAX_YEAR).contains(&y)
            || !(1..=12).contains(&m)
            || !(1..=month_length(y, m)).contains(&d)
        {
            return Err(NotATemporal(format!(
                "{year}-{month}-{day} is no day a Date holds"
            )));
        }
        Ok(Date { year, month, day })
    }

    pub fn year(&self) -> i32 {
        self.year
    }

    pub fn month(&self) -> u8 {
        self.month
    }

    pub fn day(&self) -> u8 {
        self.day
    }

    /// The text the library takes for it, which is what `LocalDate.toString` writes.
    pub fn iso(&self) -> String {
        date_text(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        )
    }

    /// The `Date` of the text the library writes for one.
    pub(crate) fn written(text: &str) -> Self {
        let (date, rest) = read_date(text).expect("the library writes a Date as LocalDate does");
        assert!(
            rest.is_empty(),
            "the library writes a Date and nothing after it"
        );
        date
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iso())
    }
}

/// A Souther `Time`: a time of day, held to the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    hour: u8,
    minute: u8,
    second: u8,
}

impl Time {
    /// The `Time` of `hour`, `minute` and `second`.
    ///
    /// # Errors
    ///
    /// [`NotATemporal`] where they name no time of day.
    pub fn new(hour: u8, minute: u8, second: u8) -> Result<Self, NotATemporal> {
        if hour > 23 || minute > 59 || second > 59 {
            return Err(NotATemporal(format!(
                "{hour}:{minute}:{second} is no time of day"
            )));
        }
        Ok(Time {
            hour,
            minute,
            second,
        })
    }

    pub fn hour(&self) -> u8 {
        self.hour
    }

    pub fn minute(&self) -> u8 {
        self.minute
    }

    pub fn second(&self) -> u8 {
        self.second
    }

    /// The text the library takes for it: `HH:mm`, and `:ss` where the second is not nought, as
    /// `LocalTime.toString` writes a time held to the second.
    pub fn iso(&self) -> String {
        if self.second == 0 {
            format!("{:02}:{:02}", self.hour, self.minute)
        } else {
            format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
        }
    }

    pub(crate) fn written(text: &str) -> Self {
        let (time, rest) = read_time(text).expect("the library writes a Time as LocalTime does");
        assert!(
            rest.is_empty(),
            "the library writes a Time and nothing after it"
        );
        time
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iso())
    }
}

/// A Souther `DateTime`: a date and a time of day, in no zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    date: Date,
    time: Time,
}

impl DateTime {
    /// The `DateTime` of `date` at `time`.
    pub fn new(date: Date, time: Time) -> Self {
        DateTime { date, time }
    }

    pub fn date(&self) -> Date {
        self.date
    }

    pub fn time(&self) -> Time {
        self.time
    }

    /// The text the library takes for it, as `LocalDateTime.toString` writes one.
    pub fn iso(&self) -> String {
        format!("{}T{}", self.date.iso(), self.time.iso())
    }

    pub(crate) fn written(text: &str) -> Self {
        let (date, rest) =
            read_date(text).expect("the library writes a DateTime as LocalDateTime does");
        let rest = rest
            .strip_prefix('T')
            .expect("the library writes a DateTime's time after a T");
        let (time, rest) =
            read_time(rest).expect("the library writes a DateTime as LocalDateTime does");
        assert!(
            rest.is_empty(),
            "the library writes a DateTime and nothing after it"
        );
        DateTime { date, time }
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iso())
    }
}

/// A Souther `Instant`: a moment, as the second from 1970-01-01T00:00:00Z and the nanosecond
/// within it, in the range `java.time.Instant` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    second: i64,
    nano: u32,
}

impl Instant {
    /// The `Instant` `second` seconds and `nano` nanoseconds after 1970-01-01T00:00:00Z.
    ///
    /// # Errors
    ///
    /// [`NotATemporal`] where the nanosecond is not below a second, or the moment is past what an
    /// `Instant` holds.
    pub fn new(second: i64, nano: u32) -> Result<Self, NotATemporal> {
        if nano >= 1_000_000_000 || !(MIN_MOMENT..=MAX_MOMENT).contains(&second) {
            return Err(NotATemporal(format!(
                "{second} seconds and {nano} nanoseconds is no moment an Instant holds"
            )));
        }
        Ok(Instant { second, nano })
    }

    /// The second from 1970-01-01T00:00:00Z.
    pub fn second(&self) -> i64 {
        self.second
    }

    /// The nanosecond within the second.
    pub fn nano(&self) -> u32 {
        self.nano
    }

    /// The text the library takes for it: in UTC, with a fraction only where there is one, as
    /// `Instant.toString` writes one.
    pub fn iso(&self) -> String {
        let (year, month, day) = civil_from_days(self.second.div_euclid(SECONDS_PER_DAY));
        let of_day = self.second.rem_euclid(SECONDS_PER_DAY);
        let mut written = date_text(year, month, day);
        written.push_str(&format!(
            "T{:02}:{:02}:{:02}",
            of_day / 3600,
            of_day % 3600 / 60,
            of_day % 60
        ));
        let nano = self.nano;
        if nano != 0 {
            if nano.is_multiple_of(1_000_000) {
                written.push_str(&format!(".{:03}", nano / 1_000_000));
            } else if nano.is_multiple_of(1_000) {
                written.push_str(&format!(".{:06}", nano / 1_000));
            } else {
                written.push_str(&format!(".{nano:09}"));
            }
        }
        written.push('Z');
        written
    }

    pub(crate) fn written(text: &str) -> Self {
        let (year, month, day, rest) =
            read_civil(text).expect("the library writes an Instant as Instant does");
        let rest = rest
            .strip_prefix('T')
            .expect("the library writes an Instant's time after a T");
        let clock = |from: &str| from.parse::<i64>().expect("two digits");
        let (hour, minute, second) = (clock(&rest[0..2]), clock(&rest[3..5]), clock(&rest[6..8]));
        let rest = &rest[8..];
        let (nano, rest) = match rest.strip_prefix('.') {
            Some(fraction) => {
                let digits = fraction.find('Z').expect("an Instant ends with Z");
                let written = &fraction[..digits];
                let nano = format!("{written:0<9}")
                    .parse::<u32>()
                    .expect("a fraction is digits");
                (nano, &fraction[digits..])
            }
            None => (0, rest),
        };
        assert_eq!(rest, "Z", "the library writes an Instant in UTC");
        let second = days_from_civil(year, month, day) * SECONDS_PER_DAY
            + hour * 3600
            + minute * 60
            + second;
        Instant { second, nano }
    }
}

impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iso())
    }
}

/// The year, month and day at the start of what `LocalDate.toString` wrote, and what follows.
fn read_civil(text: &str) -> Option<(i64, i64, i64, &str)> {
    let (sign, unsigned) = match text.as_bytes().first()? {
        b'-' => (-1, &text[1..]),
        b'+' => (1, &text[1..]),
        _ => (1, text),
    };
    let digits = unsigned.find('-')?;
    let year = sign * unsigned[..digits].parse::<i64>().ok()?;
    let rest = &unsigned[digits..];
    let month = rest.get(1..3)?.parse::<i64>().ok()?;
    let day = rest.get(4..6)?.parse::<i64>().ok()?;
    Some((year, month, day, &rest[6..]))
}

fn read_date(text: &str) -> Option<(Date, &str)> {
    let (year, month, day, rest) = read_civil(text)?;
    let date = Date::new(
        i32::try_from(year).ok()?,
        u8::try_from(month).ok()?,
        u8::try_from(day).ok()?,
    )
    .ok()?;
    Some((date, rest))
}

fn read_time(text: &str) -> Option<(Time, &str)> {
    let two = |at: usize| text.get(at..at + 2)?.parse::<u8>().ok();
    let (hour, minute) = (two(0)?, two(3)?);
    let (second, rest) = if text.as_bytes().get(5) == Some(&b':') {
        (two(6)?, &text[8..])
    } else {
        (0, &text[5..])
    };
    Some((Time::new(hour, minute, second).ok()?, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_is_what_is_read_back() {
        for date in [
            Date::new(2024, 2, 29).unwrap(),
            Date::new(-44, 3, 15).unwrap(),
            Date::new(0, 1, 1).unwrap(),
            Date::new(12_345, 12, 31).unwrap(),
        ] {
            assert_eq!(Date::written(&date.iso()), date);
        }
        for time in [Time::new(0, 0, 0).unwrap(), Time::new(23, 59, 1).unwrap()] {
            assert_eq!(Time::written(&time.iso()), time);
        }
        let at = DateTime::new(
            Date::new(1999, 12, 31).unwrap(),
            Time::new(12, 30, 0).unwrap(),
        );
        assert_eq!(DateTime::written(&at.iso()), at);
        for instant in [
            Instant::new(0, 0).unwrap(),
            Instant::new(-1, 500_000_000).unwrap(),
            Instant::new(1_700_000_000, 123_456).unwrap(),
            Instant::new(MIN_MOMENT, 0).unwrap(),
            Instant::new(MAX_MOMENT, 999_999_999).unwrap(),
        ] {
            assert_eq!(Instant::written(&instant.iso()), instant);
        }
    }

    #[test]
    fn a_day_the_calendar_does_not_have_is_refused() {
        assert!(Date::new(2023, 2, 29).is_err());
        assert!(Date::new(2024, 13, 1).is_err());
        assert!(Time::new(24, 0, 0).is_err());
        assert!(Instant::new(0, 1_000_000_000).is_err());
        assert!(Instant::new(MAX_MOMENT + 1, 0).is_err());
    }

    #[test]
    fn a_date_is_written_as_local_date_writes_one() {
        assert_eq!(Date::new(-1, 1, 2).unwrap().iso(), "-0001-01-02");
        assert_eq!(Date::new(10_000, 1, 2).unwrap().iso(), "+10000-01-02");
        assert_eq!(Time::new(7, 5, 0).unwrap().iso(), "07:05");
        assert_eq!(
            Instant::new(-1, 500_000_000).unwrap().iso(),
            "1969-12-31T23:59:59.500Z"
        );
    }
}
