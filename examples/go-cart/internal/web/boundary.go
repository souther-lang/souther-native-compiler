package web

import (
	"errors"

	"github.com/raoh-project/raoh-go"
)

// A raoh decoder writes a value in the form its reader wants and refuses what cannot be written so:
// it canonicalises and validates as one step, and what it refuses has no value. The boundary
// decodes what it owns of a request that way, and the model's decoder reads what those answered.
// The binding offers each type's decoder as a raoh decoder (domain.OrdererDecoder(r)), and a
// constructor answers (T, error) with the issues as a *raoh.Issues, which is what raoh's AndThen
// takes, so both compose with the boundary's decoders as they are: an issue the model finds is
// reported at the path it is reached at, beside the boundary's, and any other error (the run
// ending) stops the decode.

// member is a member of a value the model reads whole, and the boundary's decoder of it.
type member struct {
	name    string
	decoder raoh.Decoder[any, any]
}

// text is a member the boundary writes as text, decoder saying how.
func text(name string, decoder raoh.StringDecoder) member {
	return member{name, decoder.Map(func(s string) any { return s })}
}

// members is a value read whole by model once each of its members the boundary owns is decoded by
// the boundary.
//
// Each member is decoded on its own, and the model reads the value whichever of them was refused,
// so the issues of both come back together: a Pipe would stop at the first refusal, and an orderer
// whose email is refused would never reach the model, which alone can say that a corporation has no
// company name. What the model is handed of a member is what the boundary's decoder answered for
// it, and of a refused member nothing: the model never reads text the boundary refused, so what it
// sees does not depend on whether the rest of the request was valid. It then reports the refused
// member as missing, which is only that it was taken out, and that one issue is dropped: the one at
// the member's own path. Nothing inside the member is the model's to report, since it was not
// handed it; what is wrong inside it is its decoder's to say.
func members[T any](owned []member, model raoh.Decoder[any, T]) raoh.Decoder[any, T] {
	return raoh.NewDecoder(func(in any) (T, error) {
		var zero T
		given, ok := raoh.AsObject(in)
		if !ok {
			// What it is instead is the model's to say.
			return model.Decode(in)
		}
		written := make(map[string]any, given.Len())
		for name, value := range given.All() {
			written[name] = value
		}
		var found []raoh.Issue
		refused := map[string]bool{}
		for _, m := range owned {
			value, ok := given.Get(m.name)
			if !ok {
				continue
			}
			decoded, err := m.decoder.Decode(value)
			if err == nil {
				written[m.name] = decoded
				continue
			}
			issues, ok := errors.AsType[*raoh.Issues](err)
			if !ok {
				return zero, err
			}
			delete(written, m.name)
			at := raoh.Path{}.Key(m.name)
			refused[at.String()] = true
			for _, issue := range issues.All() {
				found = append(found, issue.At(under(at, issue.Path())))
			}
		}

		value, err := model.Decode(written)
		if err == nil {
			if len(found) > 0 {
				return zero, raoh.Invalid(found...)
			}
			return value, nil
		}
		issues, ok := errors.AsType[*raoh.Issues](err)
		if !ok {
			return zero, err
		}
		for _, issue := range issues.All() {
			if !refused[issue.Path().String()] {
				found = append(found, issue)
			}
		}
		return zero, raoh.Invalid(found...)
	})
}

// under is path read as relative to at.
func under(at, path raoh.Path) raoh.Path {
	for _, segment := range path.Segments() {
		at = at.Key(segment)
	}
	return at
}
