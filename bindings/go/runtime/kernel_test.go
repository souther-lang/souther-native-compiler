// What a run does to a library's arena, over a stand-in for a library: a runtime whose arena is a
// count of what was made, and a native call that calls a host implementation back.
package souther_test

import (
	"errors"
	"sync"
	"sync/atomic"
	"testing"
	"unsafe"

	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
)

const (
	answered       souther.Status = 0
	divisionByZero souther.Status = 4
	hostException  souther.Status = 0x7fffffff
)

func statuses(t *testing.T) souther.Statuses {
	t.Helper()
	s, err := souther.NewStatuses(map[string]souther.Status{
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

// arena is a library's arena: how much has been made in it, and how often it was dropped back.
type arena struct {
	mu     sync.Mutex
	taken  int64
	resets []int64
}

func (a *arena) mark() int64 { a.mu.Lock(); defer a.mu.Unlock(); return a.taken }

func (a *arena) reset(mark int64) {
	a.mu.Lock()
	defer a.mu.Unlock()
	if mark > a.taken {
		panic("a mark is never above where the arena stands")
	}
	a.taken = mark
	a.resets = append(a.resets, mark)
}

func (a *arena) make() unsafe.Pointer {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.taken++
	return unsafe.Pointer(new(int))
}

func (a *arena) size() int64 { return a.mark() }

func libraryOf[B any](t *testing.T, identity uintptr) (*souther.Library[B], *arena) {
	t.Helper()
	a := &arena{}
	rt := souther.NewRuntime(identity, a.mark, a.reset, statuses(t))
	return souther.NewLibrary[B](rt), a
}

func misused(t *testing.T, want error, f func()) {
	t.Helper()
	defer func() {
		t.Helper()
		got := recover()
		if got == nil {
			t.Fatalf("no panic, want %v", want)
		}
		var m *souther.Misuse
		err, _ := got.(error)
		if !errors.As(err, &m) || !errors.Is(m, want) {
			t.Fatalf("panicked with %v, want a misuse: %v", got, want)
		}
	}()
	f()
}

func TestARunDropsTheArenaBackToItsMarkWhenItReturns(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	err := lib.Run(func(r *souther.Run[tagA]) error {
		a.make()
		a.make()
		return r.Scope(func(in *souther.Run[tagA]) error {
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
		_ = lib.Run(func(r *souther.Run[tagA]) error {
			a.make()
			panic("in the run")
		})
	}()
	if got := a.size(); got != 0 {
		t.Fatalf("the arena stands at %d after a run that panicked", got)
	}
	// The thread is free for a root run again: the panic left nothing open.
	if err := lib.Run(func(*souther.Run[tagA]) error { return nil }); err != nil {
		t.Fatalf("a run after a run that panicked: %v", err)
	}
}

func TestARunAnswersWhatItsFunctionReturns(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	want := errors.New("from the function")
	if got := lib.Run(func(*souther.Run[tagA]) error { return want }); got != want {
		t.Fatalf("got %v", got)
	}
}

func TestAValueUsedAfterItsRunEndedIsExpired(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	var kept souther.Ref[tagA]
	var run *souther.Run[tagA]
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		kept = souther.NewRef(r, a.make())
		run = r
		return nil
	})
	misused(t, souther.ErrExpired, func() { kept.Read() })
	misused(t, souther.ErrExpired, func() { _ = run.Scope(func(*souther.Run[tagA]) error { return nil }) })
}

func TestAValueMadeInsideIsExpiredOnceTheScopeEnded(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		var inner souther.Ref[tagA]
		_ = r.Scope(func(in *souther.Run[tagA]) error {
			inner = souther.NewRef(in, a.make())
			return nil
		})
		misused(t, souther.ErrExpired, func() { inner.Read() })
		misused(t, souther.ErrExpired, func() { _, _ = inner.In(r) })
		return nil
	})
}

func TestAnOuterValueIsReadAndHandedToAComputationInsideAScope(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		outer := souther.NewRef(r, a.make())
		return r.Scope(func(in *souther.Run[tagA]) error {
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
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		outer := souther.NewRef(r, a.make())
		return r.Scope(func(*souther.Run[tagA]) error {
			misused(t, souther.ErrNotTheInnermostRun, func() {
				_ = souther.Call(r, func() souther.Status { return answered })
			})
			misused(t, souther.ErrNotTheInnermostRun, func() { _, _ = outer.In(r) })
			misused(t, souther.ErrNotTheInnermostRun, func() { _ = r.Scope(func(*souther.Run[tagA]) error { return nil }) })
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
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		v := souther.NewRef(r, a.make())
		var wg sync.WaitGroup
		wg.Add(1)
		go func() {
			defer wg.Done()
			misused(t, souther.ErrRunOnAnotherGoroutine, func() { v.Read() })
			misused(t, souther.ErrRunOnAnotherGoroutine, func() { _, _ = v.In(r) })
			misused(t, souther.ErrRunOnAnotherGoroutine, func() {
				_ = souther.Call(r, func() souther.Status { return answered })
			})
			misused(t, souther.ErrRunOnAnotherGoroutine, func() { _ = r.Scope(func(*souther.Run[tagA]) error { return nil }) })
		}()
		wg.Wait()
		return nil
	})
}

func TestAZeroValueHoldsNothing(t *testing.T) {
	var v souther.Ref[tagA]
	misused(t, souther.ErrNoValue, func() { v.Read() })
}

func TestASecondRootRunOfOneRuntimeOnOneThreadIsRefused(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	// Another handle on the same arena is the same runtime.
	other := souther.NewLibrary[tagA](souther.NewRuntime(1, func() int64 { return 0 }, func(int64) {}, statuses(t)))
	_ = lib.Run(func(*souther.Run[tagA]) error {
		if err := lib.Run(func(*souther.Run[tagA]) error { return nil }); !errors.Is(err, souther.ErrAlreadyRunning) {
			t.Errorf("a second root run through the same handle: %v", err)
		}
		if err := other.Run(func(*souther.Run[tagA]) error { return nil }); !errors.Is(err, souther.ErrAlreadyRunning) {
			t.Errorf("a second root run through another handle on the arena: %v", err)
		}
		return nil
	})
}

func TestARootRunOfAnotherRuntimeIsOpenedInsideOne(t *testing.T) {
	a, _ := libraryOf[tagA](t, 1)
	b, _ := libraryOf[tagB](t, 2)
	err := a.Run(func(*souther.Run[tagA]) error {
		return b.Run(func(*souther.Run[tagB]) error { return nil })
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
	for range 4 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			err := lib.Run(func(*souther.Run[tagA]) error {
				now := running.Add(1)
				for {
					seen := most.Load()
					if now <= seen || most.CompareAndSwap(seen, now) {
						break
					}
				}
				<-release
				running.Add(-1)
				return nil
			})
			if err != nil {
				t.Errorf("a root run on a thread of its own: %v", err)
			}
		}()
	}
	for running.Load() < 4 {
	}
	close(release)
	wg.Wait()
	if most.Load() != 4 {
		t.Fatalf("at most %d runs were open together", most.Load())
	}
}

func TestAValueOfAnotherRuntimeIsRefusedBeforeTheCall(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	other := souther.NewLibrary[tagA](souther.NewRuntime(2, func() int64 { return 0 }, func(int64) {}, statuses(t)))
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		mine := souther.NewRef(r, a.make())
		return other.Run(func(o *souther.Run[tagA]) error {
			if _, err := mine.In(o); !errors.Is(err, souther.ErrForeignHandle) {
				t.Errorf("a value another runtime made: %v", err)
			}
			return nil
		})
	})
}

func TestAValueOfAnotherHandleOnTheSameArenaIsGoodInThisOne(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	sameArena := souther.NewLibrary[tagA](souther.NewRuntime(1, a.mark, a.reset, statuses(t)))
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		mine := souther.NewRef(r, a.make())
		// Same identity, so same arena: it cannot be opened as a root through the other handle
		// while this one is, and a scope of this run is the way in.
		if err := sameArena.Run(func(*souther.Run[tagA]) error { return nil }); !errors.Is(err, souther.ErrAlreadyRunning) {
			t.Errorf("another handle on the arena: %v", err)
		}
		return r.Scope(func(in *souther.Run[tagA]) error {
			if _, err := mine.In(in); err != nil {
				t.Errorf("a value of the same runtime: %v", err)
			}
			return nil
		})
	})
}

func TestACallAnswersWhatItsStatusSays(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		if err := souther.Call(r, func() souther.Status { return answered }); err != nil {
			t.Errorf("answered: %v", err)
		}
		var abort *souther.Abort
		err := souther.Call(r, func() souther.Status { return divisionByZero })
		if !errors.As(err, &abort) || abort.Name != "DIVISION_BY_ZERO" || abort.Status != divisionByZero {
			t.Errorf("an abort names the status: %v", err)
		}
		if err := souther.Call(r, func() souther.Status { return 0x7ffffffd }); !errors.Is(err, souther.ErrUnbound) {
			t.Errorf("unbound: %v", err)
		}
		if err := souther.Call(r, func() souther.Status { return 0x7ffffffe }); !errors.Is(err, souther.ErrProtocolViolation) {
			t.Errorf("protocol violation: %v", err)
		}
		// A host that said it failed with nothing kept broke the protocol.
		if err := souther.Call(r, func() souther.Status { return hostException }); !errors.Is(err, souther.ErrProtocolViolation) {
			t.Errorf("a failure with nothing kept: %v", err)
		}
		return nil
	})
}

func TestAStatusAManifestDoesNotNameIsAnAbortWithoutAName(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		var abort *souther.Abort
		err := souther.Call(r, func() souther.Status { return 99 })
		if !errors.As(err, &abort) || abort.Name != "" || abort.Status != 99 {
			t.Errorf("got %v", err)
		}
		return nil
	})
}

func TestStatusesNeedTheOnesAHostTellsApart(t *testing.T) {
	_, err := souther.NewStatuses(map[string]souther.Status{"ANSWERED": 0})
	var unnamed *souther.UnnamedStatus
	if !errors.As(err, &unnamed) {
		t.Fatalf("got %v", err)
	}
}

// callback is the library calling a host implementation back in the middle of a call.
func callback(origin *souther.Run[tagA], f func(*souther.Run[tagA]) error) souther.Status {
	return souther.Host(origin, f)
}

func TestAHostImplementationIsLentTheRunTheCallWasMadeIn(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		return r.Scope(func(inner *souther.Run[tagA]) error {
			var lent *souther.Run[tagA]
			err := souther.Call(inner, func() souther.Status {
				// The implementation was made in the outer run, and is lent the innermost.
				return callback(r, func(in *souther.Run[tagA]) error {
					lent = in
					souther.NewRef(in, a.make())
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
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		err := souther.Call(r, func() souther.Status {
			return callback(r, func(*souther.Run[tagA]) error { return failed })
		})
		var host *souther.HostError
		if !errors.As(err, &host) || !errors.Is(err, failed) {
			t.Errorf("got %v", err)
		}
		return nil
	})
}

func TestAHostImplementationsPanicIsRaisedAgainWhereTheLibraryReturns(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		defer func() {
			if got := recover(); got != "boom" {
				t.Errorf("raised %v", got)
			}
		}()
		_ = souther.Call(r, func() souther.Status {
			status := callback(r, func(*souther.Run[tagA]) error { panic("boom") })
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
	noPlace := &souther.Abort{Status: 5, Name: "REQUIRED_FORM_HAS_NO_PLACE"}
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		err := souther.Call(r, func() souther.Status {
			return callback(r, func(*souther.Run[tagA]) error { return souther.Crossing(noPlace) })
		})
		var host *souther.HostError
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
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		var inner error
		outer := souther.Call(r, func() souther.Status {
			return callback(r, func(in *souther.Run[tagA]) error {
				// The implementation calls the library, which calls another implementation back.
				inner = souther.Call(in, func() souther.Status {
					return callback(r, func(*souther.Run[tagA]) error { return innerFailure })
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
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		err := souther.Call(r, func() souther.Status {
			callback(r, func(*souther.Run[tagA]) error { return first })
			return callback(r, func(*souther.Run[tagA]) error { return second })
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
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		err := souther.Call(r, func() souther.Status {
			callback(r, func(*souther.Run[tagA]) error { return failed })
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
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		if got := callback(r, func(*souther.Run[tagA]) error { return nil }); got != hostException {
			t.Errorf("no call is open, and the library was told %d", got)
		}
		return nil
	})
}

func TestWhatARunKeptIsReleasedAfterTheArenaIsDroppedBack(t *testing.T) {
	lib, a := libraryOf[tagA](t, 1)
	var order []string
	_ = lib.Run(func(r *souther.Run[tagA]) error {
		a.make()
		r.Keep(func() { order = append(order, "first kept") })
		r.Keep(func() { order = append(order, "second kept") })
		return nil
	})
	if len(order) != 2 || order[0] != "second kept" || order[1] != "first kept" {
		t.Fatalf("released %v", order)
	}
	if a.size() != 0 {
		t.Fatal("the arena was dropped back before what the run kept was released")
	}
}
