// What a run does to a library's arena, over a stand-in for a library: a runtime whose arena is a
// count of what was made, and a native call that calls a host implementation back.
package souther

import (
	"errors"
	"sync"
	"sync/atomic"
	"testing"
	"unsafe"
)

const (
	answered       Status = 0
	divisionByZero Status = 4
	hostException  Status = 0x7fffffff
)

func statuses(t *testing.T) statusTable {
	t.Helper()
	s, err := newStatuses(map[string]Status{
		"ANSWERED":                     answered,
		"DIVISION_BY_ZERO":             divisionByZero,
		"INJECTION_UNBOUND":            0x7ffffffd,
		"INJECTION_PROTOCOL_VIOLATION": 0x7ffffffe,
		"HOST_EXCEPTION":               hostException,
	})
	if err != nil {
		t.Fatal(err)
	}
	return s
}

// tagA and tagB are two generated bindings, which are two types of run.
type tagA struct{}
type tagB struct{}

// arena is a library's arena: how much has been made in it, the scopes open on it, and where it
// was dropped back to each time one closed.
type arena struct {
	mu     sync.Mutex
	taken  int64
	tokens int64
	open   [][2]int64
	resets []int64
}

func (a *arena) scopeOpen() int64 {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.tokens++
	a.open = append(a.open, [2]int64{a.tokens, a.taken})
	return a.tokens
}

// scopeClose closes scope where it is open. One arena stands here for every thread's, so which
// scope is innermost is not asked: the library's runtime holds that of each thread, and its own
// tests and the bridge's hold it to that.
func (a *arena) scopeClose(scope int64) bool {
	a.mu.Lock()
	defer a.mu.Unlock()
	for at, open := range a.open {
		if open[0] == scope {
			a.open = append(a.open[:at], a.open[at+1:]...)
			a.taken = open[1]
			a.resets = append(a.resets, open[1])
			return true
		}
	}
	return false
}

func (a *arena) make() unsafe.Pointer {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.taken++
	return unsafe.Pointer(new(int))
}

func (a *arena) size() int64 { a.mu.Lock(); defer a.mu.Unlock(); return a.taken }

func libraryOf[B any](t *testing.T, identity uintptr) (*Library[B], *arena) {
	t.Helper()
	a := &arena{}
	rt := newRuntime(identity, a.scopeOpen, a.scopeClose, statuses(t))
	return newLibrary[B](rt), a
}

func misused(t *testing.T, want error, f func()) {
	t.Helper()
	defer func() {
		t.Helper()
		got := recover()
		if got == nil {
			t.Fatalf("no panic, want %v", want)
		}
		var m *Misuse
		err, _ := got.(error)
		if !errors.As(err, &m) || !errors.Is(m, want) {
			t.Fatalf("panicked with %v, want a misuse: %v", got, want)
		}
	}()
	f()
}

func TestARunDropsTheArenaBackToItsMarkWhenItReturns(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	err := lib.Run(func(r *Run[tagA]) error {
		a.make()
		a.make()
		return r.Scope(func(in *Run[tagA]) error {
			a.make()
			return nil
		})
	})
	if err != nil {
		t.Fatal(err)
	}
	if got := a.size(); got != 0 {
		t.Fatalf("the arena stands at %d after the run", got)
	}
	if got := a.resets; len(got) != 2 || got[0] != 2 || got[1] != 0 {
		t.Fatalf("dropped back to %v, the inner run first and then the root", got)
	}
}

func TestARunDropsTheArenaBackWhenItPanics(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	func() {
		defer func() { _ = recover() }()
		_ = lib.Run(func(r *Run[tagA]) error {
			a.make()
			panic("in the run")
		})
	}()
	if got := a.size(); got != 0 {
		t.Fatalf("the arena stands at %d after a run that panicked", got)
	}
	// The thread is free for a root run again: the panic left nothing open.
	if err := lib.Run(func(*Run[tagA]) error { return nil }); err != nil {
		t.Fatalf("a run after a run that panicked: %v", err)
	}
}

func TestARunAnswersWhatItsFunctionReturns(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	want := errors.New("from the function")
	if got := lib.Run(func(*Run[tagA]) error { return want }); got != want {
		t.Fatalf("got %v", got)
	}
}

func TestAValueUsedAfterItsRunEndedIsExpired(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	var kept Ref[tagA]
	var run *Run[tagA]
	_ = lib.Run(func(r *Run[tagA]) error {
		kept = NewRef(r, a.make())
		run = r
		return nil
	})
	misused(t, ErrExpired, func() { kept.Read() })
	misused(t, ErrExpired, func() { _ = run.Scope(func(*Run[tagA]) error { return nil }) })
}

func TestAValueMadeInsideIsExpiredOnceTheScopeEnded(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		var inner Ref[tagA]
		_ = r.Scope(func(in *Run[tagA]) error {
			inner = NewRef(in, a.make())
			return nil
		})
		misused(t, ErrExpired, func() { inner.Read() })
		misused(t, ErrExpired, func() { _, _ = inner.In(r) })
		return nil
	})
}

func TestAnOuterValueIsReadAndHandedToAComputationInsideAScope(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		outer := NewRef(r, a.make())
		return r.Scope(func(in *Run[tagA]) error {
			if outer.Read() == nil {
				t.Error("an outer value is read inside a scope")
			}
			at, err := outer.In(in)
			if err != nil || at == nil {
				t.Errorf("an outer value is handed to a computation inside: %v", err)
			}
			return nil
		})
	})
}

func TestAComputationThroughARunWithARunInsideItIsNotTheInnermost(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		outer := NewRef(r, a.make())
		return r.Scope(func(*Run[tagA]) error {
			misused(t, ErrNotTheInnermostRun, func() {
				_ = Call(r, func() Status { return answered })
			})
			misused(t, ErrNotTheInnermostRun, func() { _, _ = outer.In(r) })
			misused(t, ErrNotTheInnermostRun, func() { _ = r.Scope(func(*Run[tagA]) error { return nil }) })
			// Reading is not making, and is good from the run outside.
			if outer.Read() == nil {
				t.Error("reading an outer value is not a computation")
			}
			return nil
		})
	})
}

func TestARunAndItsValuesBelongToTheGoroutineThatOpenedThem(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		v := NewRef(r, a.make())
		var wg sync.WaitGroup
		wg.Add(1)
		go func() {
			defer wg.Done()
			misused(t, ErrRunOnAnotherGoroutine, func() { v.Read() })
			misused(t, ErrRunOnAnotherGoroutine, func() { _, _ = v.In(r) })
			misused(t, ErrRunOnAnotherGoroutine, func() {
				_ = Call(r, func() Status { return answered })
			})
			misused(t, ErrRunOnAnotherGoroutine, func() { _ = r.Scope(func(*Run[tagA]) error { return nil }) })
		}()
		wg.Wait()
		return nil
	})
}

func TestAZeroValueHoldsNothing(t *testing.T) {
	var v Ref[tagA]
	misused(t, ErrNoValue, func() { v.Read() })
}

func TestASecondRootRunOfOneRuntimeOnOneThreadIsRefused(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	// Another handle on the same arena is the same runtime.
	other := newLibrary[tagA](newRuntime(1, func() int64 { return 0 }, func(int64) bool { return true }, statuses(t)))
	_ = lib.Run(func(*Run[tagA]) error {
		if err := lib.Run(func(*Run[tagA]) error { return nil }); !errors.Is(err, ErrAlreadyRunning) {
			t.Errorf("a second root run through the same handle: %v", err)
		}
		if err := other.Run(func(*Run[tagA]) error { return nil }); !errors.Is(err, ErrAlreadyRunning) {
			t.Errorf("a second root run through another handle on the arena: %v", err)
		}
		return nil
	})
}

func TestARootRunOfAnotherRuntimeIsOpenedInsideOne(t *testing.T) {
	a, _ := libraryOf[tagA](t, 1)
	b, _ := libraryOf[tagB](t, 2)
	err := a.Run(func(*Run[tagA]) error {
		return b.Run(func(*Run[tagB]) error { return nil })
	})
	if err != nil {
		t.Fatalf("a run of one library inside a run of another: %v", err)
	}
}

func TestOneRuntimeHasARootRunOnEachThread(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	var running, most atomic.Int32
	var wg sync.WaitGroup
	release := make(chan struct{})
	opened := make(chan struct{}, 4)
	for range 4 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			err := lib.Run(func(*Run[tagA]) error {
				now := running.Add(1)
				for {
					seen := most.Load()
					if now <= seen || most.CompareAndSwap(seen, now) {
						break
					}
				}
				opened <- struct{}{}
				<-release
				running.Add(-1)
				return nil
			})
			if err != nil {
				t.Errorf("a root run on a thread of its own: %v", err)
			}
		}()
	}
	for range 4 {
		<-opened
	}
	close(release)
	wg.Wait()
	if most.Load() != 4 {
		t.Fatalf("at most %d runs were open together", most.Load())
	}
}

func TestAValueOfAnotherRuntimeIsRefusedBeforeTheCall(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	other := newLibrary[tagA](newRuntime(2, func() int64 { return 0 }, func(int64) bool { return true }, statuses(t)))
	_ = lib.Run(func(r *Run[tagA]) error {
		mine := NewRef(r, a.make())
		return other.Run(func(o *Run[tagA]) error {
			if _, err := mine.In(o); !errors.Is(err, ErrForeignHandle) {
				t.Errorf("a value another runtime made: %v", err)
			}
			return nil
		})
	})
}

func TestAValueOfAnotherHandleOnTheSameArenaIsGoodInThisOne(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	sameArena := newLibrary[tagA](newRuntime(1, a.scopeOpen, a.scopeClose, statuses(t)))
	_ = lib.Run(func(r *Run[tagA]) error {
		mine := NewRef(r, a.make())
		// Same identity, so same arena: it cannot be opened as a root through the other handle
		// while this one is, and a scope of this run is the way in.
		if err := sameArena.Run(func(*Run[tagA]) error { return nil }); !errors.Is(err, ErrAlreadyRunning) {
			t.Errorf("another handle on the arena: %v", err)
		}
		return r.Scope(func(in *Run[tagA]) error {
			if _, err := mine.In(in); err != nil {
				t.Errorf("a value of the same runtime: %v", err)
			}
			return nil
		})
	})
}

func TestACallAnswersWhatItsStatusSays(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		if err := Call(r, func() Status { return answered }); err != nil {
			t.Errorf("answered: %v", err)
		}
		var abort *Abort
		err := Call(r, func() Status { return divisionByZero })
		if !errors.As(err, &abort) || abort.Name != "DIVISION_BY_ZERO" || abort.Status != divisionByZero {
			t.Errorf("an abort names the status: %v", err)
		}
		if err := Call(r, func() Status { return 0x7ffffffd }); !errors.Is(err, ErrUnbound) {
			t.Errorf("unbound: %v", err)
		}
		if err := Call(r, func() Status { return 0x7ffffffe }); !errors.Is(err, ErrProtocolViolation) {
			t.Errorf("protocol violation: %v", err)
		}
		// A host that said it failed with nothing kept broke the protocol.
		if err := Call(r, func() Status { return hostException }); !errors.Is(err, ErrProtocolViolation) {
			t.Errorf("a failure with nothing kept: %v", err)
		}
		return nil
	})
}

func TestAStatusAManifestDoesNotNameIsAnAbortWithoutAName(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		var abort *Abort
		err := Call(r, func() Status { return 99 })
		if !errors.As(err, &abort) || abort.Name != "" || abort.Status != 99 {
			t.Errorf("got %v", err)
		}
		return nil
	})
}

func TestStatusesNeedTheOnesAHostTellsApart(t *testing.T) {
	_, err := newStatuses(map[string]Status{"ANSWERED": 0})
	var unnamed *UnnamedStatus
	if !errors.As(err, &unnamed) {
		t.Fatalf("got %v", err)
	}
}

// callback is the library calling a host implementation back in the middle of a call.
func callback(origin *Run[tagA], f func(*Run[tagA]) error) Status {
	return Host(origin, f)
}

func TestAHostImplementationIsLentTheRunTheCallWasMadeIn(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		return r.Scope(func(inner *Run[tagA]) error {
			var lent *Run[tagA]
			err := Call(inner, func() Status {
				// The implementation was made in the outer run, and is lent the innermost.
				return callback(r, func(in *Run[tagA]) error {
					lent = in
					NewRef(in, a.make())
					return nil
				})
			})
			if err != nil {
				t.Errorf("the call: %v", err)
			}
			if lent != inner {
				t.Error("an implementation is lent the innermost run, not the one it was made in")
			}
			return nil
		})
	})
}

func TestAHostImplementationsErrorIsAnsweredWhereTheLibraryReturns(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	failed := errors.New("no such product")
	_ = lib.Run(func(r *Run[tagA]) error {
		err := Call(r, func() Status {
			return callback(r, func(*Run[tagA]) error { return failed })
		})
		var host *HostError
		if !errors.As(err, &host) || !errors.Is(err, failed) {
			t.Errorf("got %v", err)
		}
		return nil
	})
}

func TestAHostImplementationsPanicIsRaisedAgainWhereTheLibraryReturns(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		defer func() {
			if got := recover(); got != "boom" {
				t.Errorf("raised %v", got)
			}
		}()
		_ = Call(r, func() Status {
			status := callback(r, func(*Run[tagA]) error { panic("boom") })
			if status != hostException {
				t.Errorf("the library was told %d, not that the implementation failed", status)
			}
			return status
		})
		t.Error("the call returned")
		return nil
	})
}

func TestWhatAnImplementationFailsToCrossIsWhatTheCallComesTo(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	noPlace := &Abort{Status: 5, Name: "REQUIRED_FORM_HAS_NO_PLACE"}
	_ = lib.Run(func(r *Run[tagA]) error {
		err := Call(r, func() Status {
			return callback(r, func(*Run[tagA]) error { return Crossing(noPlace) })
		})
		var host *HostError
		if errors.As(err, &host) || err != error(noPlace) {
			t.Errorf("a crossing failure comes to itself, not a host's: %v", err)
		}
		return nil
	})
}

func TestACallbackThatCallsBackInKeepsEachFailureWithItsOwnCall(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	outerFailure := errors.New("the outer implementation failed")
	innerFailure := errors.New("the inner implementation failed")
	_ = lib.Run(func(r *Run[tagA]) error {
		var inner error
		outer := Call(r, func() Status {
			return callback(r, func(in *Run[tagA]) error {
				// The implementation calls the library, which calls another implementation back.
				inner = Call(in, func() Status {
					return callback(r, func(*Run[tagA]) error { return innerFailure })
				})
				return outerFailure
			})
		})
		if !errors.Is(inner, innerFailure) || errors.Is(inner, outerFailure) {
			t.Errorf("the inner call comes to %v", inner)
		}
		if !errors.Is(outer, outerFailure) || errors.Is(outer, innerFailure) {
			t.Errorf("the outer call comes to %v", outer)
		}
		return nil
	})
}

func TestTheFirstFailureOfACallIsTheOneItIsAnsweredWith(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	first, second := errors.New("first"), errors.New("second")
	_ = lib.Run(func(r *Run[tagA]) error {
		err := Call(r, func() Status {
			callback(r, func(*Run[tagA]) error { return first })
			return callback(r, func(*Run[tagA]) error { return second })
		})
		if !errors.Is(err, first) || errors.Is(err, second) {
			t.Errorf("got %v", err)
		}
		return nil
	})
}

func TestAKeptFailureIsWhatACallComesToWhateverStatusTheLibraryAnswered(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	failed := errors.New("failed")
	_ = lib.Run(func(r *Run[tagA]) error {
		err := Call(r, func() Status {
			callback(r, func(*Run[tagA]) error { return failed })
			return divisionByZero
		})
		if !errors.Is(err, failed) {
			t.Errorf("the host's failure was lost to the status: %v", err)
		}
		return nil
	})
}

func TestACallbackOutsideACallFails(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		if got := callback(r, func(*Run[tagA]) error { return nil }); got != hostException {
			t.Errorf("no call is open, and the library was told %d", got)
		}
		return nil
	})
}

func TestWhatARunKeptIsReleasedAfterTheArenaIsDroppedBack(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	var order []string
	_ = lib.Run(func(r *Run[tagA]) error {
		a.make()
		r.hold(func() { order = append(order, "first kept") })
		r.hold(func() { order = append(order, "second kept") })
		return nil
	})
	if len(order) != 2 || order[0] != "second kept" || order[1] != "first kept" {
		t.Fatalf("released %v", order)
	}
	if a.size() != 0 {
		t.Fatal("the arena was dropped back before what the run kept was released")
	}
}
