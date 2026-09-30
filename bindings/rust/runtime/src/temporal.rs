//! A Souther `Date`, `Time`, `DateTime` and `Instant` as Rust holds one.
//!
//! Each is held here as the numbers it means and handed to the library as those numbers, which is
//! how the library takes one and answers one (souther-native-compiler#137). It is checked where it
//! is made against what the type holds, so that a value a Rust program has is always one: the
//! library decides the same again where it is handed one, and a refusal from it is this crate and
//! the library disagreeing. The text `java.time` writes for each ([`Date::iso`] and the rest) is
//! for a Rust program to show, and never crosses to the library.

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

    /// The text `LocalDate.toString` writes for it.
    pub fn iso(&self) -> String {
        date_text(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        )
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

    /// The text `LocalTime.toString` writes for it, held to the second: `HH:mm`, and `:ss` where
    /// the second is not nought.
    pub fn iso(&self) -> String {
        if self.second == 0 {
            format!("{:02}:{:02}", self.hour, self.minute)
        } else {
            format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
        }
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

    /// The text `LocalDateTime.toString` writes for it.
    pub fn iso(&self) -> String {
        format!("{}T{}", self.date.iso(), self.time.iso())
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

    /// The text `Instant.toString` writes for it: in UTC, with a fraction only where there is one.
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
}

impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iso())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
