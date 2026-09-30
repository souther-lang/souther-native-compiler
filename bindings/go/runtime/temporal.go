package souther

import (
	"errors"
	"fmt"
	"time"
)

// A Souther Date, Time, DateTime and Instant as Go holds one.
//
// Each is held here as the numbers it means and handed to the library as those numbers, which is how
// the library takes one and answers one (souther-native-compiler#137). It is checked where it is
// made against what the type holds, so that a value a Go program has is always one: the library
// decides the same again where it is handed one, and a refusal from it is this package and the
// library disagreeing. The text java.time writes for each is for a Go program to show, and never
// crosses to the library. They are held apart
// from time.Time because none of them is what it is: a Date and a Time have no zone and no
// instant, and an Instant reaches a billion years either way, where time.Time's text stops at
// year 9999.

const (
	minYear = -999_999_999
	maxYear = 999_999_999

	secondsPerDay = 86_400

	// minMoment and maxMoment are the first and the last second an Instant holds, as
	// java.time.Instant has them: -1000000000-01-01T00:00:00Z to
	// +1000000000-12-31T23:59:59.999999999Z.
	minMoment = -31_557_014_167_219_200
	maxMoment = 31_556_889_864_403_199
)

// ErrNotTemporal is numbers that name no value of a temporal type.
var ErrNotTemporal = errors.New("numbers that name no value of a temporal type")

func notTemporal(format string, arguments ...any) error {
	return fmt.Errorf("%w: %s", ErrNotTemporal, fmt.Sprintf(format, arguments...))
}

// civilFromDays is the year, the month and the day a count of days from 1970-01-01 names.
func civilFromDays(days int64) (year, month, day int64) {
	days += 719_468
	era := floorDiv(days, 146_097)
	dayOfEra := days - era*146_097
	yearOfEra := (dayOfEra - dayOfEra/1_460 + dayOfEra/36_524 - dayOfEra/146_096) / 365
	dayOfYear := dayOfEra - (365*yearOfEra + yearOfEra/4 - yearOfEra/100)
	monthFromMarch := (5*dayOfYear + 2) / 153
	day = dayOfYear - (153*monthFromMarch+2)/5 + 1
	month = monthFromMarch - 9
	if monthFromMarch < 10 {
		month = monthFromMarch + 3
	}
	year = yearOfEra + era*400
	if month <= 2 {
		year++
	}
	return year, month, day
}

func floorDiv(a, b int64) int64 {
	q := a / b
	if (a%b != 0) && ((a < 0) != (b < 0)) {
		q--
	}
	return q
}

func floorMod(a, b int64) int64 { return a - floorDiv(a, b)*b }

func isLeap(year int64) bool {
	return floorMod(year, 4) == 0 && (floorMod(year, 100) != 0 || floorMod(year, 400) == 0)
}

func monthLength(year, month int64) int64 {
	switch month {
	case 1, 3, 5, 7, 8, 10, 12:
		return 31
	case 4, 6, 9, 11:
		return 30
	}
	if isLeap(year) {
		return 29
	}
	return 28
}

// dateText is a date as LocalDate.toString writes it: a year of at least four digits, and a sign
// past 9999 and below nought.
func dateText(year, month, day int64) string {
	sign := ""
	switch {
	case year < 0:
		sign = "-"
		year = -year
	case year > 9999:
		sign = "+"
	}
	return fmt.Sprintf("%s%04d-%02d-%02d", sign, year, month, day)
}

// Date is a Souther Date: a day of the proleptic ISO calendar, in a year a java.time.LocalDate
// holds. It is comparable and its zero value is no date: make one with [NewDate].
type Date struct {
	year  int32
	month uint8
	day   uint8
}

// NewDate is the Date of year, month and day, or [ErrNotTemporal] where the calendar has no such
// day, or the year is past what a Date holds.
func NewDate(year int32, month, day uint8) (Date, error) {
	y, m, d := int64(year), int64(month), int64(day)
	if y < minYear || y > maxYear || m < 1 || m > 12 || d < 1 || d > monthLength(y, m) {
		return Date{}, notTemporal("%d-%d-%d is no day a Date holds", year, month, day)
	}
	return Date{year, month, day}, nil
}

// Year is the year, which is negative before year 1 as the ISO calendar counts.
func (d Date) Year() int32 { return d.year }

// Month is the month, from 1.
func (d Date) Month() uint8 { return d.month }

// Day is the day of the month, from 1.
func (d Date) Day() uint8 { return d.day }

// String is the text LocalDate.toString writes for it.
func (d Date) String() string { return dateText(int64(d.year), int64(d.month), int64(d.day)) }

// Time is a Souther Time: a time of day, held to the second. Its zero value is midnight.
type Time struct {
	hour, minute, second uint8
}

// NewTime is the Time of hour, minute and second, or [ErrNotTemporal] where they name no time of
// day.
func NewTime(hour, minute, second uint8) (Time, error) {
	if hour > 23 || minute > 59 || second > 59 {
		return Time{}, notTemporal("%d:%d:%d is no time of day", hour, minute, second)
	}
	return Time{hour, minute, second}, nil
}

// Hour is the hour, from 0.
func (t Time) Hour() uint8 { return t.hour }

// Minute is the minute, from 0.
func (t Time) Minute() uint8 { return t.minute }

// Second is the second, from 0.
func (t Time) Second() uint8 { return t.second }

// String is the text LocalTime.toString writes for it, held to the second: HH:mm, and :ss where the
// second is not nought.
func (t Time) String() string {
	if t.second == 0 {
		return fmt.Sprintf("%02d:%02d", t.hour, t.minute)
	}
	return fmt.Sprintf("%02d:%02d:%02d", t.hour, t.minute, t.second)
}

// DateTime is a Souther DateTime: a date and a time of day, in no zone.
type DateTime struct {
	date Date
	time Time
}

// NewDateTime is the DateTime of date at time.
func NewDateTime(date Date, time Time) DateTime { return DateTime{date, time} }

// Date is the day.
func (d DateTime) Date() Date { return d.date }

// Time is the time of day.
func (d DateTime) Time() Time { return d.time }

// String is the text LocalDateTime.toString writes for it.
func (d DateTime) String() string { return d.date.String() + "T" + d.time.String() }

// Instant is a Souther Instant: a moment, as the second from 1970-01-01T00:00:00Z and the
// nanosecond within it, in the range java.time.Instant holds.
type Instant struct {
	second int64
	nano   uint32
}

// NewInstant is the Instant second seconds and nano nanoseconds after 1970-01-01T00:00:00Z, or
// [ErrNotTemporal] where the nanosecond is not below a second, or the moment is past what an
// Instant holds.
func NewInstant(second int64, nano uint32) (Instant, error) {
	if nano >= 1_000_000_000 || second < minMoment || second > maxMoment {
		return Instant{}, notTemporal("%d seconds and %d nanoseconds is no moment an Instant holds",
			second, nano)
	}
	return Instant{second, nano}, nil
}

// InstantOf is the Instant of t, which is the same moment in any zone.
func InstantOf(t time.Time) (Instant, error) { return NewInstant(t.Unix(), uint32(t.Nanosecond())) }

// Second is the second from 1970-01-01T00:00:00Z.
func (i Instant) Second() int64 { return i.second }

// Nano is the nanosecond within the second.
func (i Instant) Nano() uint32 { return i.nano }

// Time is the moment as a time.Time in UTC.
func (i Instant) Time() time.Time { return time.Unix(i.second, int64(i.nano)).UTC() }

// String is the text Instant.toString writes for it: in UTC, with a fraction only where there is
// one.
func (i Instant) String() string {
	year, month, day := civilFromDays(floorDiv(i.second, secondsPerDay))
	ofDay := floorMod(i.second, secondsPerDay)
	written := fmt.Sprintf("%sT%02d:%02d:%02d", dateText(year, month, day), ofDay/3600, ofDay%3600/60, ofDay%60)
	switch {
	case i.nano == 0:
	case i.nano%1_000_000 == 0:
		written += fmt.Sprintf(".%03d", i.nano/1_000_000)
	case i.nano%1_000 == 0:
		written += fmt.Sprintf(".%06d", i.nano/1_000)
	default:
		written += fmt.Sprintf(".%09d", i.nano)
	}
	return written + "Z"
}
