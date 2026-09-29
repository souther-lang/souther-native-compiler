//go:build unix

package souther

/*
#include <pthread.h>
*/
import "C"

// thread is an OS thread. A run holds its goroutine on one, so it stands for the goroutine too.
//
// Threads are told apart by pthread_equal and never by comparing the ids, which POSIX does not
// promise are integers.
type thread struct{ id C.pthread_t }

func currentThread() thread { return thread{C.pthread_self()} }

func (t thread) equal(other thread) bool { return C.pthread_equal(t.id, other.id) != 0 }
