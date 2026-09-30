package souther

import (
	"errors"
	"testing"
	"time"
)

func TestADateIsWrittenAsLocalDateWritesIt(t *testing.T) {
	for _, tc := range []struct {
		year       int32
		month, day uint8
		written    string
	}{
		{2024, 2, 29, "2024-02-29"},
		{-44, 3, 15, "-0044-03-15"},
		{0, 1, 1, "0000-01-01"},
		{10000, 12, 31, "+10000-12-31"},
		{-999_999_999, 1, 1, "-999999999-01-01"},
		{999_999_999, 12, 31, "+999999999-12-31"},
	} {
		date, err := NewDate(tc.year, tc.month, tc.day)
		if err != nil {
			t.Fatalf("%v: %v", tc, err)
		}
		if got := date.String(); got != tc.written {
			t.Errorf("wrote %s, want %s", got, tc.written)
		}
	}
}

func TestNumbersThatNameNoDayAreRefused(t *testing.T) {
	for _, tc := range []struct {
		year       int32
		month, day uint8
	}{{2023, 2, 29}, {2024, 13, 1}, {2024, 0, 1}, {2024, 4, 31}, {1_000_000_000, 1, 1}} {
		if _, err := NewDate(tc.year, tc.month, tc.day); !errors.Is(err, ErrNotTemporal) {
			t.Errorf("%v: %v", tc, err)
		}
	}
	if _, err := NewTime(24, 0, 0); !errors.Is(err, ErrNotTemporal) {
		t.Errorf("hour 24: %v", err)
	}
	if _, err := NewTime(0, 60, 0); !errors.Is(err, ErrNotTemporal) {
		t.Errorf("minute 60: %v", err)
	}
}

func TestATimeIsWrittenToTheSecondAndOnlyWhereItIsNotNought(t *testing.T) {
	for written, want := range map[string]Time{
		"10:15":    {10, 15, 0},
		"10:15:30": {10, 15, 30},
		"00:00":    {},
	} {
		if got := want.String(); got != written {
			t.Errorf("wrote %s, want %s", got, written)
		}
	}
}

func TestADateTimeIsItsDateAndItsTime(t *testing.T) {
	date, _ := NewDate(2024, 2, 29)
	clock, _ := NewTime(23, 59, 1)
	dt := NewDateTime(date, clock)
	if dt.String() != "2024-02-29T23:59:01" {
		t.Errorf("wrote %s", dt)
	}
}

func TestAnInstantIsWrittenInUTCWithAFractionOnlyWhereThereIsOne(t *testing.T) {
	for _, tc := range []struct {
		second  int64
		nano    uint32
		written string
	}{
		{0, 0, "1970-01-01T00:00:00Z"},
		{1_700_000_000, 0, "2023-11-14T22:13:20Z"},
		{1_700_000_000, 500_000_000, "2023-11-14T22:13:20.500Z"},
		{1_700_000_000, 123_456_000, "2023-11-14T22:13:20.123456Z"},
		{1_700_000_000, 123_456_789, "2023-11-14T22:13:20.123456789Z"},
		{-1, 999_999_999, "1969-12-31T23:59:59.999999999Z"},
		{minMoment, 0, "-1000000000-01-01T00:00:00Z"},
		{maxMoment, 999_999_999, "+1000000000-12-31T23:59:59.999999999Z"},
	} {
		instant, err := NewInstant(tc.second, tc.nano)
		if err != nil {
			t.Fatalf("%v: %v", tc, err)
		}
		if got := instant.String(); got != tc.written {
			t.Errorf("wrote %s, want %s", got, tc.written)
		}
	}
}

func TestAMomentPastWhatAnInstantHoldsIsRefused(t *testing.T) {
	if _, err := NewInstant(minMoment-1, 0); !errors.Is(err, ErrNotTemporal) {
		t.Errorf("before: %v", err)
	}
	if _, err := NewInstant(maxMoment+1, 0); !errors.Is(err, ErrNotTemporal) {
		t.Errorf("after: %v", err)
	}
	if _, err := NewInstant(0, 1_000_000_000); !errors.Is(err, ErrNotTemporal) {
		t.Errorf("a whole second of nanoseconds: %v", err)
	}
}

func TestAnInstantIsTheSameMomentAsATimeInAnyZone(t *testing.T) {
	zone := time.FixedZone("far", 9*3600)
	moment := time.Date(2024, 2, 29, 12, 30, 45, 123, zone)
	instant, err := InstantOf(moment)
	if err != nil {
		t.Fatal(err)
	}
	if !instant.Time().Equal(moment) || instant.Time().Location() != time.UTC {
		t.Errorf("got %v", instant.Time())
	}
}
