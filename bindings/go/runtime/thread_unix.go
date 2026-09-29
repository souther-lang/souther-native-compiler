//go:build unix

package souther

/*
#include <pthread.h>
*/
import "C"

// thread is an OS thread. A run holds its goroutine on one, so it stands for the goroutine too.
//
// Threads are told apart by comparing the ids in Go: POSIX does not promise they are numbers, but
// on every system this is written for (Linux, macOS, the BSDs) one is an integer or a pointer, and
// a call to pthread_equal is a second crossing into C for each check, which would double what a
// check costs. The id is a Go value that compiles to a comparison only where that holds.
type thread struct{ id C.pthread_t }

func currentThread() thread { return thread{C.pthread_self()} }

func (t thread) equal(other thread) bool { return t.id == other.id }
