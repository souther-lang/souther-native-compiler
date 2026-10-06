package souther

import (
	"encoding/json"
	"testing"
)

// A temporal's text is read back as the value it writes, over the whole range the type holds, as
// java.time and notation-199x write it: years past 9999 and below nought, and an instant a billion
// years either way. What time.Parse reads stops at year 9999, which is why these are not read
// with it.
func TestATemporalsTextIsReadBackOverTheWholeRangeItHolds(t *testing.T) {
	for _, text := range []string{"2026-01-31", "0000-01-01", "-0001-12-31", "+10000-01-01",
		"+999999999-12-31", "-999999999-01-01"} {
		read, err := dateOfText(text)
		if err != nil || read.String() != text {
			t.Errorf("date %s read as %v, %v", text, read, err)
		}
	}
	for _, text := range []string{"00:00", "10:00:30", "23:59:59"} {
		read, err := timeOfText(text)
		if err != nil || read.String() != text {
			t.Errorf("time %s read as %v, %v", text, read, err)
		}
	}
	for _, text := range []string{"2026-01-31T10:00", "+10000-01-01T00:00:01", "-0001-12-31T23:59"} {
		read, err := dateTimeOfText(text)
		if err != nil || read.String() != text {
			t.Errorf("date-time %s read as %v, %v", text, read, err)
		}
	}
	for _, text := range []string{"1970-01-01T00:00:00Z", "2026-01-31T10:00:00.500Z",
		"+1000000000-12-31T23:59:59.999999999Z", "-1000000000-01-01T00:00:00Z", "-0001-12-31T23:59:59.000001Z"} {
		read, err := instantOfText(text)
		if err != nil || read.String() != text {
			t.Errorf("instant %s read as %v, %v", text, read, err)
		}
	}
	for _, text := range []string{"2026-02-30", "10000-01-01", "+1000000000-01-01"} {
		if _, err := dateOfText(text); err == nil {
			t.Errorf("date %s was read", text)
		}
	}
	if _, err := timeOfText("10:00:30.5"); err == nil {
		t.Error("a time past the second was read")
	}
}

// A temporal is written to JSON as Raoh observes it, the string of its text, which is what an
// issue rendered by raoh writes its metadata as.
func TestATemporalIsWrittenToJsonAsItsText(t *testing.T) {
	date, _ := dateOfText("+10000-01-01")
	clock, _ := timeOfText("10:00:30")
	stamp, _ := dateTimeOfText("2026-01-31T10:00")
	moment, _ := instantOfText("2026-01-31T10:00:00Z")
	written, err := json.Marshal([]any{date, clock, stamp, moment})
	if err != nil {
		t.Fatal(err)
	}
	if string(written) != `["+10000-01-01","10:00:30","2026-01-31T10:00","2026-01-31T10:00:00Z"]` {
		t.Errorf("written as %s", written)
	}
}
