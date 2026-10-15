package souther

import (
	"encoding/json"
	"errors"
	"fmt"
	"strconv"
	"strings"
	"time"

	notation199x "github.com/raoh-project/notation-199x/go"
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

// daysFromCivil is the count of days from 1970-01-01 to the day year, month and day name, the
// inverse of civilFromDays.
func daysFromCivil(year, month, day int64) int64 {
	if month <= 2 {
		year--
	}
	era := floorDiv(year, 400)
	yearOfEra := year - era*400
	monthFromMarch := (month + 9) % 12
	dayOfYear := (153*monthFromMarch+2)/5 + day - 1
	dayOfEra := yearOfEra*365 + yearOfEra/4 - yearOfEra/100 + dayOfYear
	return era*146_097 + dayOfEra - 719_468
}

// admitted is text of kind as notation-199x reads one, which is the one statement of the form; the
// fields are read from it only once it is.
func admitted(kind notation199x.TemporalKind, text string) error {
	if answer := notation199x.CheckTemporal(kind, text); answer != notation199x.Admitted {
		return notTemporal("%q is no %v: %v", text, kind, answer)
	}
	return nil
}

// civilOfText is the year, the month and the day of text that is a date as notation-199x admits
// one: a year of at least four digits, signed past 9999 and below nought.
func civilOfText(text string) (year, month, day int64, err error) {
	if err := admitted(notation199x.Date, text); err != nil {
		return 0, 0, 0, err
	}
	year, month, day = civilFields(text)
	return year, month, day, nil
}

// civilFields is the year, the month and the day text writes, which was admitted as a date or as
// the date of an instant, whose years reach one further either way.
func civilFields(text string) (year, month, day int64) {
	sign := int64(1)
	switch text[0] {
	case '-':
		sign, text = -1, text[1:]
	case '+':
		text = text[1:]
	}
	parts := strings.Split(text, "-")
	year, _ = strconv.ParseInt(parts[0], 10, 64)
	month, _ = strconv.ParseInt(parts[1], 10, 64)
	day, _ = strconv.ParseInt(parts[2], 10, 64)
	return sign * year, month, day
}

// dateOfText is the Date text writes, as LocalDate.toString and notation-199x write one.
func dateOfText(text string) (Date, error) {
	year, month, day, err := civilOfText(text)
	if err != nil {
		return Date{}, err
	}
	return NewDate(int32(year), uint8(month), uint8(day))
}

// clockOfText is the hour, the minute, the second and the nanosecond of text that is a time of
// day as notation-199x admits one.
func clockOfText(text string) (hour, minute, second int64, nano uint32, err error) {
	if err := admitted(notation199x.Time, text); err != nil {
		return 0, 0, 0, 0, err
	}
	clock, fraction, _ := strings.Cut(text, ".")
	parts := strings.Split(clock, ":")
	hour, _ = strconv.ParseInt(parts[0], 10, 64)
	minute, _ = strconv.ParseInt(parts[1], 10, 64)
	if len(parts) > 2 {
		second, _ = strconv.ParseInt(parts[2], 10, 64)
	}
	if fraction != "" {
		nanos, _ := strconv.ParseUint((fraction + "000000000")[:9], 10, 32)
		nano = uint32(nanos)
	}
	return hour, minute, second, nano, nil
}

// timeOfText is the Time text writes. A Souther Time is held to the second, so a fraction is no
// Time.
func timeOfText(text string) (Time, error) {
	hour, minute, second, nano, err := clockOfText(text)
	if err != nil {
		return Time{}, err
	}
	if nano != 0 {
		return Time{}, notTemporal("%q is past the second a Time is held to", text)
	}
	return NewTime(uint8(hour), uint8(minute), uint8(second))
}

// dateTimeOfText is the DateTime text writes: a date, T and a time.
func dateTimeOfText(text string) (DateTime, error) {
	if err := admitted(notation199x.DateTime, text); err != nil {
		return DateTime{}, err
	}
	date, clock, _ := strings.Cut(text, "T")
	day, err := dateOfText(date)
	if err != nil {
		return DateTime{}, err
	}
	of, err := timeOfText(clock)
	if err != nil {
		return DateTime{}, err
	}
	return NewDateTime(day, of), nil
}

// instantOfText is the Instant text writes, as Instant.toString and notation-199x write one: a
// date-time in UTC with its seconds, and Z.
func instantOfText(text string) (Instant, error) {
	if err := admitted(notation199x.Instant, text); err != nil {
		return Instant{}, err
	}
	date, clock, _ := strings.Cut(strings.TrimSuffix(text, "Z"), "T")
	year, month, day := civilFields(date)
	hour, minute, second, nano, err := clockOfText(clock)
	if err != nil {
		return Instant{}, err
	}
	return NewInstant(daysFromCivil(year, month, day)*secondsPerDay+hour*3600+minute*60+second, nano)
}

// MarshalJSON writes the Date as Raoh observes one: the JSON string of its text.
func (d Date) MarshalJSON() ([]byte, error) { return json.Marshal(d.String()) }

// MarshalJSON writes the Time as Raoh observes one: the JSON string of its text.
func (t Time) MarshalJSON() ([]byte, error) { return json.Marshal(t.String()) }

// MarshalJSON writes the DateTime as Raoh observes one: the JSON string of its text.
func (d DateTime) MarshalJSON() ([]byte, error) { return json.Marshal(d.String()) }

// MarshalJSON writes the Instant as Raoh observes one: the JSON string of its text.
func (i Instant) MarshalJSON() ([]byte, error) { return json.Marshal(i.String()) }
