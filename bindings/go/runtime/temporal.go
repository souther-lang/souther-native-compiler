package souther

import (
	"errors"
	"fmt"
	"strconv"
	"strings"
	"time"
)

// A Souther Date, Time, DateTime and Instant as Go holds one.
//
// The library takes one as the text that names it and ends the process on text that names none, so
// each is held here as its numbers, checked where it is made against what the type holds, and
// handed over as the text java.time writes for it, which the library reads. What the library
// answers is read back out of the text it writes, which is that same form. They are held apart
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

// daysFromCivil is the days from 1970-01-01 to a day of the proleptic Gregorian calendar (Howard
// Hinnant's days_from_civil), as the library's runtime counts them.
func daysFromCivil(year, month, day int64) int64 {
	if month <= 2 {
		year--
	}
	era := floorDiv(year, 400)
	yearOfEra := year - era*400
	monthFromMarch := month + 9
	if month > 2 {
		monthFromMarch = month - 3
	}
	dayOfYear := (153*monthFromMarch+2)/5 + day - 1
	dayOfEra := yearOfEra*365 + yearOfEra/4 - yearOfEra/100 + dayOfYear
	return era*146_097 + dayOfEra - 719_468
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

// String is the text the library takes for it, which is what LocalDate.toString writes.
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

// String is the text the library takes for it: HH:mm, and :ss where the second is not nought, as
// LocalTime.toString writes a time held to the second.
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

// String is the text the library takes for it, as LocalDateTime.toString writes one.
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

// String is the text the library takes for it: in UTC, with a fraction only where there is one, as
// Instant.toString writes one.
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

// readCivil is the year, month and day at the start of what LocalDate.toString wrote, and what
// follows.
func readCivil(text string) (year, month, day int64, rest string, ok bool) {
	sign := int64(1)
	switch {
	case strings.HasPrefix(text, "-"):
		sign, text = -1, text[1:]
	case strings.HasPrefix(text, "+"):
		text = text[1:]
	}
	digits := strings.IndexByte(text, '-')
	if digits < 0 {
		return 0, 0, 0, "", false
	}
	y, err := strconv.ParseInt(text[:digits], 10, 64)
	if err != nil {
		return 0, 0, 0, "", false
	}
	tail := text[digits:]
	if len(tail) < 6 {
		return 0, 0, 0, "", false
	}
	m, errM := strconv.ParseInt(tail[1:3], 10, 64)
	d, errD := strconv.ParseInt(tail[4:6], 10, 64)
	if errM != nil || errD != nil {
		return 0, 0, 0, "", false
	}
	return sign * y, m, d, tail[6:], true
}

func readDate(text string) (Date, string, bool) {
	year, month, day, rest, ok := readCivil(text)
	if !ok || year < minYear || year > maxYear {
		return Date{}, "", false
	}
	date, err := NewDate(int32(year), uint8(month), uint8(day))
	return date, rest, err == nil
}

func readTime(text string) (Time, string, bool) {
	two := func(at int) (uint8, bool) {
		if len(text) < at+2 {
			return 0, false
		}
		n, err := strconv.ParseUint(text[at:at+2], 10, 8)
		return uint8(n), err == nil
	}
	hour, okH := two(0)
	minute, okM := two(3)
	if !okH || !okM {
		return Time{}, "", false
	}
	second, rest := uint8(0), ""
	if len(text) > 5 && text[5] == ':' {
		s, ok := two(6)
		if !ok {
			return Time{}, "", false
		}
		second, rest = s, text[8:]
	} else {
		rest = text[5:]
	}
	t, err := NewTime(hour, minute, second)
	return t, rest, err == nil
}

// writtenDate is the Date of the text the library writes for one, which is what LocalDate writes.
func writtenDate(text string) Date {
	date, rest, ok := readDate(text)
	if !ok || rest != "" {
		panic(fmt.Sprintf("souther: the library writes a Date as LocalDate does, and wrote %q", text))
	}
	return date
}

func writtenTime(text string) Time {
	t, rest, ok := readTime(text)
	if !ok || rest != "" {
		panic(fmt.Sprintf("souther: the library writes a Time as LocalTime does, and wrote %q", text))
	}
	return t
}

func writtenDateTime(text string) DateTime {
	date, rest, ok := readDate(text)
	if ok && strings.HasPrefix(rest, "T") {
		if t, tail, okT := readTime(rest[1:]); okT && tail == "" {
			return DateTime{date, t}
		}
	}
	panic(fmt.Sprintf("souther: the library writes a DateTime as LocalDateTime does, and wrote %q", text))
}

func writtenInstant(text string) Instant {
	fail := func() Instant {
		panic(fmt.Sprintf("souther: the library writes an Instant as Instant does, and wrote %q", text))
	}
	year, month, day, rest, ok := readCivil(text)
	if !ok || !strings.HasPrefix(rest, "T") || len(rest) < 9 {
		return fail()
	}
	rest = rest[1:]
	clock := func(from string) int64 {
		n, err := strconv.ParseInt(from, 10, 64)
		if err != nil {
			fail()
		}
		return n
	}
	hour, minute, second := clock(rest[0:2]), clock(rest[3:5]), clock(rest[6:8])
	rest = rest[8:]
	nano := uint32(0)
	if strings.HasPrefix(rest, ".") {
		end := strings.IndexByte(rest, 'Z')
		if end < 0 {
			return fail()
		}
		fraction := (rest[1:end] + "000000000")[:9]
		n, err := strconv.ParseUint(fraction, 10, 32)
		if err != nil {
			return fail()
		}
		nano, rest = uint32(n), rest[end:]
	}
	if rest != "Z" {
		return fail()
	}
	return Instant{daysFromCivil(year, month, day)*secondsPerDay + hour*3600 + minute*60 + second, nano}
}
