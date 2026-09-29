package souther

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
	"testing"
)

// surface is what go doc says of the package without what says it in words: the declarations, and
// not the comments on them, which a change of wording is free to move.
func surface(t *testing.T) string {
	t.Helper()
	said, err := exec.Command("go", "doc", "-all", ".").Output()
	if err != nil {
		t.Fatal(err)
	}
	var kept []string
	for _, line := range strings.Split(string(said), "\n") {
		if strings.HasPrefix(line, "    ") || strings.HasPrefix(strings.TrimSpace(line), "//") {
			continue
		}
		kept = append(kept, line)
	}
	return strings.Join(kept, "\n")
}

// The public surface of the package is what `go doc` says of it, and each protocol is what it said
// when the protocol was made. A change to it that does not move the protocol is a change to what a
// generated package calls without a package that says so.
func TestTheSurfaceOfTheRuntimeMovesWithItsProtocol(t *testing.T) {
	said := surface(t)
	record := fmt.Sprintf("protocol/%d.txt", Protocol)
	recorded, err := os.ReadFile(record)
	if err != nil {
		t.Fatalf("%v: the surface of protocol %d is not recorded, and is:\n%s", err, Protocol, said)
	}
	if string(recorded) != said {
		t.Fatalf("the surface is not what %s records: a change to it is a new protocol, with a record of its own.\n"+
			"It is now:\n%s", record, said)
	}
}
