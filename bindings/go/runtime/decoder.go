package souther

import (
	"encoding/json"

	"github.com/raoh-project/raoh-go"
)

// Decoder is decode, a type's reading of its external form, as a raoh decoder of what a host has
// decoded already, reading in r: a host composes it with decoders of its own, as a JVM host
// composes a type's decoder(), so that the issues of both come back together.
//
// What it is handed is a value of raoh's input model, such as what raoh.DecodeJSON read, and the
// library reads text, so the value is written back as the JSON it is: an object's members in the
// order they came in, and a number as it was written. What the library finds wrong is found at the
// path the decoder was reached at. A member that is not there is raoh's to report, as it is for any
// field, since there is no text to hand over; null is handed over, and the type says whether it
// takes one. A value that no JSON writes is a type_mismatch. Any other error of decode, the run
// ending, stops the decode and is not taken for invalid input.
func Decoder[B, T any](r *Run[B], decode func(*Run[B], []byte) (T, error)) raoh.Decoder[any, T] {
	return raoh.NewDecoder(func(in any) (T, error) {
		var zero T
		if raoh.IsMissing(in) {
			return zero, raoh.Invalid(raoh.NewIssue(raoh.CodeRequired))
		}
		text, err := json.Marshal(in)
		if err != nil {
			return zero, raoh.Invalid(raoh.NewIssue(raoh.CodeTypeMismatch).
				WithMessage("a value no JSON writes: " + err.Error()))
		}
		return decode(r, text)
	})
}
