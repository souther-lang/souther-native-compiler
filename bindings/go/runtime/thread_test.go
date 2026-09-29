package souther

import (
	"os"
	"regexp"
	"testing"
)

// Threads are told apart by pthread_equal, the one comparison POSIX gives of two threads' ids, and
// by nothing that reads an id as a number. A speed a check gains by comparing ids is one a system
// whose pthread_t is no number does not have, so the contract is held here as text: no comparison
// of what a thread holds appears in the file that says what a thread is.
func TestThreadsAreComparedByPthreadEqualAndNothingElse(t *testing.T) {
	written, err := os.ReadFile("thread_unix.go")
	if err != nil {
		t.Fatal(err)
	}
	if regexp.MustCompile(`\.id\s*(==|!=)|(==|!=)\s*\w+\.id`).Match(written) {
		t.Fatal("thread_unix.go compares the ids of threads, which only pthread_equal may")
	}
}
