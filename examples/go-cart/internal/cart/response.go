package cart

import (
	"encoding/json"
	"errors"
	"net/http"

	"github.com/raoh-project/raoh-go"
	"github.com/raoh-project/raoh-go/encode"
)

// Outcome is what a request came to, and whether what was written while answering it is kept.
//
// A route says which, where it knows what the domain answered. That the domain answered is not that
// its answer is to be kept: an injected behavior may write before the command is refused (loadCart
// makes the user's cart row), and a refused command keeps nothing it wrote on the way. A route
// answers an Outcome and not a [Response], so no route commits without saying so.
type Outcome struct {
	keep     bool
	response Response
}

// Commit is a command that succeeded: what it wrote is kept.
func Commit(r Response) Outcome { return Outcome{keep: true, response: r} }

// Rollback is a command that was refused, or wrote nothing worth keeping: what it wrote is dropped.
func Rollback(r Response) Outcome { return Outcome{keep: false, response: r} }

// ClientError is a request the client got wrong: a body that is not JSON, not what the route reads,
// or longer than a body may be. Err holds the *raoh.Issues found in it, or raoh.ErrInputTooLarge.
type ClientError struct{ Err error }

func (e *ClientError) Error() string { return e.Err.Error() }

func (e *ClientError) Unwrap() error { return e.Err }

func (e *ClientError) response() Response {
	if issues, ok := errors.AsType[*raoh.Issues](e.Err); ok {
		return BadRequest(issues)
	}
	return TooLarge()
}

// Response is a status and, where there is one, a body that is JSON.
type Response struct {
	Status int
	Body   []byte
}

// Write writes the response to w.
func (r Response) Write(w http.ResponseWriter) {
	if r.Body == nil {
		w.WriteHeader(r.Status)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(r.Status)
	_, _ = w.Write(r.Body)
}

// OK is a 200 with body, which is what a value of the model encodes to, or any other JSON.
func OK(body []byte) Response { return Response{http.StatusOK, body} }

// Created is a 201, with body where there is one.
func Created(body []byte) Response { return Response{http.StatusCreated, body} }

// BadRequest is a 400 with raoh's issues, each with its path, and the messages by path.
func BadRequest(issues *raoh.Issues) Response {
	return Response{http.StatusBadRequest, written(issuesBody.Encode(issues))}
}

// Unprocessable is a 422 naming a business case the model answered.
func Unprocessable(name string) Response {
	return Response{http.StatusUnprocessableEntity, written(errorBody.Encode(name))}
}

// TooLarge is a 413, for a body longer than a request may be.
func TooLarge() Response {
	return Response{http.StatusRequestEntityTooLarge, written(errorBody.Encode("too_large"))}
}

// Internal is a request that came to no answer: the run ended, an implementation failed, or the
// database did.
func Internal() Response {
	return Response{http.StatusInternalServerError, written(errorBody.Encode("internal"))}
}

var (
	issuesBody = encode.Object(
		encode.Property("issues", func(is *raoh.Issues) []raoh.RenderedIssue { return is.Render(raoh.English) },
			encode.Value[[]raoh.RenderedIssue]()),
		encode.Property("errors", func(is *raoh.Issues) map[string][]string { return is.Flatten(raoh.English) },
			encode.Value[map[string][]string]()),
	)
	errorBody = encode.Object(
		encode.Property("error", func(name string) string { return name }, encode.String()),
	)
)

// written is JSON text of what an encoder gave, which is maps, slices and scalars only and always
// has one.
func written(encoded any) []byte {
	text, err := json.Marshal(encoded)
	if err != nil {
		panic(err)
	}
	return text
}
