//go:build unix

package souther

/*
#include <stdint.h>

// The runtime functions every library exports, called through the address the library was loaded
// with. Their words are the ones the manifest says, which the generator holds a manifest to.
// A value's word is an address in the arena and crosses as a void pointer.
static int64_t call_string_length(void *fn, void *text) { return ((int64_t (*)(void *))fn)(text); }
static const uint8_t *call_string_bytes(void *fn, void *text) {
	return ((const uint8_t *(*)(void *))fn)(text);
}
static uint8_t call_string_of_utf8(void *fn, const uint8_t *bytes, int64_t length, void **out) {
	return ((uint8_t (*)(const uint8_t *, int64_t, void **))fn)(bytes, length, out);
}
static int32_t call_decoded_outcome(void *fn, void *decoded) { return ((int32_t (*)(void *))fn)(decoded); }
static void *call_decoded_value(void *fn, void *decoded) { return ((void *(*)(void *))fn)(decoded); }
static int64_t call_decoded_malformed_at(void *fn, void *decoded) { return ((int64_t (*)(void *))fn)(decoded); }
static int64_t call_decoded_issue_count(void *fn, void *decoded) { return ((int64_t (*)(void *))fn)(decoded); }
static void *call_decoded_issue(void *fn, void *decoded, int64_t at) {
	return ((void *(*)(void *, int64_t))fn)(decoded, at);
}
static void *call_issue_text(void *fn, void *issue) { return ((void *(*)(void *))fn)(issue); }
static void *call_decimal_of_parts(void *fn, const uint8_t *bytes, int64_t length, int64_t scale) {
	return ((void *(*)(const uint8_t *, int64_t, int64_t))fn)(bytes, length, scale);
}
static void *call_decimal_unscaled(void *fn, void *decimal) { return ((void *(*)(void *))fn)(decimal); }
static int64_t call_decimal_scale(void *fn, void *decimal) { return ((int64_t (*)(void *))fn)(decimal); }
static void *call_temporal(void *fn, void *text) { return ((void *(*)(void *))fn)(text); }
*/
import "C"

import (
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"strconv"
	"strings"
	"unicode/utf8"
	"unsafe"

	"github.com/raoh-project/raoh-go"
)

// ErrNotUTF8 is text handed to the library that is not UTF-8, which a String holds only as.
var ErrNotUTF8 = errors.New("text handed to the library is not valid UTF-8")

// Text is the text of a String the library answered, copied into a Go string: the bytes are in the
// arena, and are good only until the run they were made in ends. word is the address the library
// answered, and r the run it was made in or one inside it.
func Text[B any](r *Run[B], word unsafe.Pointer) string {
	r.checkReading()
	lib := r.lib
	length := int64(C.call_string_length(lib.Symbol("souther_string_length"), word))
	if length == 0 {
		return ""
	}
	bytes := C.call_string_bytes(lib.Symbol("souther_string_bytes"), word)
	// Copied once, and by the length the library says and not one an int can hold.
	return string(unsafe.Slice((*byte)(unsafe.Pointer(bytes)), length))
}

// String is text as the library holds a String, made in r, for a call about to be made in it.
//
// The library puts text in NFC where it takes it, by the Unicode version the language names, and
// asks of the host only that it is UTF-8. It returns [ErrNotUTF8] where it is not, and an
// [*Abort] of REQUIRED_FORM_HAS_NO_PLACE where the canonical value is longer than a String holds.
func String[B any](r *Run[B], text string) (unsafe.Pointer, error) {
	r.checkMaking()
	if !utf8.ValidString(text) {
		return nil, ErrNotUTF8
	}
	// The library reads no byte where there are none, and the address is still to be one.
	var none [1]byte
	bytes := unsafe.Pointer(&none[0])
	if len(text) > 0 {
		bytes = unsafe.Pointer(unsafe.StringData(text))
	}
	var word unsafe.Pointer
	admitted := C.call_string_of_utf8(r.lib.Symbol("souther_string_of_utf8"), (*C.uint8_t)(bytes),
		C.int64_t(len(text)), &word)
	if admitted != 0 {
		return word, nil
	}
	const name = "REQUIRED_FORM_HAS_NO_PLACE"
	status, ok := r.lib.rt.statuses.Named(name)
	if !ok {
		return nil, ErrProtocolViolation
	}
	return nil, &Abort{Status: status, Name: name}
}

// Bytes is where the bytes of b are, to hand to the library for the length of a call: an address
// even where there are none, which the library then reads none of.
func Bytes(b []byte) unsafe.Pointer {
	if len(b) == 0 {
		var none [1]byte
		return unsafe.Pointer(&none[0])
	}
	return unsafe.Pointer(&b[0])
}

// Addr is where the elements of s are, to hand to the library for the length of a call: an address
// even where there are none, which the library then reads none of.
func Addr[T any](s []T) unsafe.Pointer {
	if len(s) == 0 {
		var none [1]byte
		return unsafe.Pointer(&none[0])
	}
	return unsafe.Pointer(&s[0])
}

// Decimal is d as the library holds a Decimal, made in r. The unscaled digits are the integer's
// text and never the value's written form, so they are never fallible on what a String holds.
func Decimal[B any](r *Run[B], d raoh.Decimal) unsafe.Pointer {
	r.checkMaking()
	digits := d.Unscaled().String()
	return C.call_decimal_of_parts(r.lib.Symbol("souther_decimal_of_parts"),
		(*C.uint8_t)(unsafe.Pointer(unsafe.StringData(digits))), C.int64_t(len(digits)),
		C.int64_t(d.Scale()))
}

// Amount is the Decimal the library answered, with the scale it has: 1.50 is 150 at scale 2 and 1.5
// is 15 at scale 1, and Scale says which. Two are equal as amounts by Cmp, and as Go values only
// where they are written alike.
func Amount[B any](r *Run[B], word unsafe.Pointer) raoh.Decimal {
	r.checkReading()
	unscaled := Text(r, C.call_decimal_unscaled(r.lib.Symbol("souther_decimal_unscaled"), word))
	scale := int64(C.call_decimal_scale(r.lib.Symbol("souther_decimal_scale"), word))
	d, err := raoh.ParseDecimal(unscaled + "E" + strconv.FormatInt(-scale, 10))
	if err != nil {
		panic(fmt.Sprintf("souther: the library's Decimal is one: %v", err))
	}
	return d
}

// Temporal is the text of a temporal value handed to the library, which reads it as the value
// whose name it is: the address of the value made by the function of the library at symbol.
func temporal[B any](r *Run[B], symbol, text string) (unsafe.Pointer, error) {
	iso, err := String(r, text)
	if err != nil {
		return nil, err
	}
	return C.call_temporal(r.lib.Symbol(symbol), iso), nil
}

// DateWord is date as the library holds a Date, made in r.
func DateWord[B any](r *Run[B], date Date) (unsafe.Pointer, error) {
	return temporal(r, "souther_date_of_iso", date.String())
}

// TimeWord is t as the library holds a Time, made in r.
func TimeWord[B any](r *Run[B], t Time) (unsafe.Pointer, error) {
	return temporal(r, "souther_time_of_iso", t.String())
}

// DateTimeWord is dt as the library holds a DateTime, made in r.
func DateTimeWord[B any](r *Run[B], dt DateTime) (unsafe.Pointer, error) {
	return temporal(r, "souther_datetime_of_iso", dt.String())
}

// InstantWord is i as the library holds an Instant, made in r.
func InstantWord[B any](r *Run[B], i Instant) (unsafe.Pointer, error) {
	return temporal(r, "souther_instant_of_iso", i.String())
}

func isoText[B any](r *Run[B], symbol string, word unsafe.Pointer) string {
	r.checkReading()
	return Text(r, C.call_temporal(r.lib.Symbol(symbol), word))
}

// DateOf is the Date the library answered.
func DateOf[B any](r *Run[B], word unsafe.Pointer) Date {
	return writtenDate(isoText(r, "souther_date_iso", word))
}

// TimeOf is the Time the library answered.
func TimeOf[B any](r *Run[B], word unsafe.Pointer) Time {
	return writtenTime(isoText(r, "souther_time_iso", word))
}

// DateTimeOf is the DateTime the library answered.
func DateTimeOf[B any](r *Run[B], word unsafe.Pointer) DateTime {
	return writtenDateTime(isoText(r, "souther_datetime_iso", word))
}

// InstantOfWord is the Instant the library answered.
func InstantOfWord[B any](r *Run[B], word unsafe.Pointer) Instant {
	return writtenInstant(isoText(r, "souther_instant_iso", word))
}

// Constructed is what a constructor's call came to: err where it answered, an
// invariant_violation issue where the library says the invariant was not held, and the failure
// otherwise.
func Constructed(err error) error {
	var abort *Abort
	if errors.As(err, &abort) && abort.Name == "INVARIANT_NOT_HELD" {
		return raoh.Invalid(raoh.NewIssue("invariant_violation"))
	}
	return err
}

// Reading is what reading a value out of its external form came to: the address of the value the
// library read, or the issues found in it as a [*raoh.Issues] error, or an invalid_format issue
// where the text is not JSON.
func Reading[B any](r *Run[B], decoded unsafe.Pointer) (unsafe.Pointer, error) {
	r.checkReading()
	lib := r.lib
	outcome := int32(C.call_decoded_outcome(lib.Symbol("souther_decoded_outcome"), decoded))
	switch outcome {
	case lib.outcomes["VALUE"]:
		value := C.call_decoded_value(lib.Symbol("souther_decoded_value"), decoded)
		if value == nil {
			panic("souther: a reading that read a value answers it")
		}
		return value, nil
	case lib.outcomes["ISSUES"]:
		count := int64(C.call_decoded_issue_count(lib.Symbol("souther_decoded_issue_count"), decoded))
		issues := make([]raoh.Issue, 0, count)
		for at := range count {
			issue := C.call_decoded_issue(lib.Symbol("souther_decoded_issue"), decoded, C.int64_t(at))
			issues = append(issues, issueOf(r, issue))
		}
		return nil, raoh.Invalid(issues...)
	}
	at := int64(C.call_decoded_malformed_at(lib.Symbol("souther_decoded_malformed_at"), decoded))
	return nil, raoh.Invalid(raoh.NewIssue("invalid_format").
		WithMessage(fmt.Sprintf("the text stops being JSON at byte %d", at)))
}

// issueOf is one issue a reading found, as Raoh holds one. The code, the message key and the
// metadata are Raoh's already, so this changes how they are held and not what they say; the
// library gives no message, and a resolver words one by the key.
func issueOf[B any](r *Run[B], issue unsafe.Pointer) raoh.Issue {
	text := func(name string) string {
		return Text(r, C.call_issue_text(r.lib.Symbol("souther_issue_"+name), issue))
	}
	made := raoh.NewIssue(text("code")).WithMessageKey(text("message_key")).At(pathOf(text("path")))
	var meta map[string]any
	decoder := json.NewDecoder(strings.NewReader(text("meta")))
	decoder.UseNumber()
	if err := decoder.Decode(&meta); err != nil {
		panic(fmt.Sprintf("souther: the library writes metadata as a JSON object: %v", err))
	}
	for name, said := range meta {
		made = made.WithMeta(name, plain(said))
	}
	return made
}

// pathOf is a JSON Pointer as Raoh holds one.
func pathOf(pointer string) raoh.Path {
	var path raoh.Path
	if pointer == "" {
		return path
	}
	for _, segment := range strings.Split(pointer[1:], "/") {
		segment = strings.NewReplacer("~1", "/", "~0", "~").Replace(segment)
		if at, err := strconv.Atoi(segment); err == nil && strconv.Itoa(at) == segment && at >= 0 {
			path = path.Index(at)
		} else {
			path = path.Key(segment)
		}
	}
	return path
}

// plain is a decoded JSON value with its numbers as Go's: a whole number that fits is an int64,
// and any other is a float64.
func plain(value any) any {
	switch it := value.(type) {
	case json.Number:
		if whole, err := it.Int64(); err == nil {
			return whole
		}
		if decimal, err := it.Float64(); err == nil && !math.IsInf(decimal, 0) {
			return decimal
		}
		return it.String()
	case []any:
		for at := range it {
			it[at] = plain(it[at])
		}
		return it
	case map[string]any:
		for name := range it {
			it[name] = plain(it[name])
		}
		return it
	}
	return value
}
