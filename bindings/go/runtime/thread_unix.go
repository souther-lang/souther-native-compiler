//go:build unix

package souther

/*
#include <pthread.h>

// same_thread is whether this is the thread a token was taken on, by pthread_equal, which is the
// only comparison POSIX gives of two threads' ids, in one crossing into C: asking pthread_self and
// then pthread_equal from Go is two.
static int same_thread(pthread_t token) { return pthread_equal(pthread_self(), token); }
*/
import "C"

// thread is an OS thread. A run holds its goroutine on one, so it stands for the goroutine too.
//
// Two threads are told apart by pthread_equal and never by comparing their ids, which POSIX does not
// promise are numbers. Whether one is the thread this is called on is one call into C.
type thread struct{ id C.pthread_t }

func currentThread() thread { return thread{C.pthread_self()} }

// isCurrent is whether this is the thread the caller is on.
func (t thread) isCurrent() bool { return C.same_thread(t.id) != 0 }

// equal is whether two threads taken earlier are one.
func (t thread) equal(other thread) bool { return C.pthread_equal(t.id, other.id) != 0 }
