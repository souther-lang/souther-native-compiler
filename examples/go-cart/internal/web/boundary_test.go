package web

import (
	"errors"
	"reflect"
	"testing"

	"github.com/raoh-project/raoh-go"
)

// recording is a model that records what it was handed, as JSON text, and reports each of required
// it is missing.
func recording(read *[]string, required ...string) raoh.Decoder[any, struct{}] {
	return raoh.NewDecoder(func(in any) (struct{}, error) {
		text, err := jsonText(in)
		if err != nil {
			return struct{}{}, err
		}
		*read = append(*read, string(text))
		given, _ := raoh.AsObject(in)
		var missing []raoh.Issue
		for _, name := range required {
			if _, ok := given.Get(name); !ok {
				missing = append(missing, raoh.NewIssue("missing_field").At(raoh.Path{}.Key(name)))
			}
		}
		if len(missing) > 0 {
			return struct{}{}, raoh.Invalid(missing...)
		}
		return struct{}{}, nil
	})
}

func email() member { return text("email", raoh.String().Trim().ToLower().Email()) }

func decode(t *testing.T, d raoh.Decoder[any, struct{}], text string) []string {
	t.Helper()
	_, err := raoh.DecodeJSON([]byte(text), d)
	if err == nil {
		return nil
	}
	issues, ok := errors.AsType[*raoh.Issues](err)
	if !ok {
		t.Fatal(err)
	}
	var paths []string
	for _, issue := range issues.All() {
		paths = append(paths, issue.Path().String())
	}
	return paths
}

func TestTheModelReadsWhatTheBoundaryWrote(t *testing.T) {
	var read []string

	decode(t, members([]member{email()}, recording(&read, "email")), `{"email":" A@Example.COM "}`)

	if want := []string{`{"email":"a@example.com"}`}; !reflect.DeepEqual(read, want) {
		t.Fatalf("the model read %v", read)
	}
}

func TestTheModelIsNeverHandedWhatTheBoundaryRefused(t *testing.T) {
	var read []string

	paths := decode(t, members([]member{email()}, recording(&read, "email", "city")),
		`{"email":" X ","city":"Tokyo"}`)

	if want := []string{`{"city":"Tokyo"}`}; !reflect.DeepEqual(read, want) {
		t.Fatalf("the model read %v", read)
	}
	if want := []string{"/email"}; !reflect.DeepEqual(paths, want) {
		t.Fatalf("issues at %v", paths)
	}
}

func TestTheModelReadsTheRestWhicheverMemberWasRefused(t *testing.T) {
	var read []string

	paths := decode(t, members([]member{email()}, recording(&read, "email", "city")),
		`{"email":"not-an-email"}`)

	if want := []string{"/email", "/city"}; !reflect.DeepEqual(paths, want) {
		t.Fatalf("issues at %v", paths)
	}
}

func TestOnlyTheRefusedMembersOwnPathIsTakenForTheBoundarys(t *testing.T) {
	// The boundary's decoder of an address refuses its postcode, and the address is taken out. The
	// model reports it missing, which is dropped, and the model's own issue beside it is kept:
	// nothing at a path merely under or beside the refused member is dropped.
	address := member{"address", raoh.NewDecoder(func(any) (any, error) {
		return nil, raoh.Invalid(raoh.NewIssue("invalid_format").At(raoh.Path{}.Key("postcode")))
	})}
	var read []string

	paths := decode(t, members([]member{address}, recording(&read, "address", "addressee")),
		`{"address":{"postcode":1}}`)

	if want := []string{"/address/postcode", "/addressee"}; !reflect.DeepEqual(paths, want) {
		t.Fatalf("issues at %v", paths)
	}
}

func TestAMemberIsReadWhereTheValueIs(t *testing.T) {
	// The issues of a value read whole are at its path in the request, the boundary's and the model's.
	var read []string
	whole := raoh.Object(raoh.Fields().
		Field("orderer", members([]member{email()}, recording(&read, "email", "name"))),
	).Map(func(struct{}) struct{} { return struct{}{} })

	paths := decode(t, whole, `{"orderer":{"email":"x"}}`)

	if want := []string{"/orderer/email", "/orderer/name"}; !reflect.DeepEqual(paths, want) {
		t.Fatalf("issues at %v", paths)
	}
}
