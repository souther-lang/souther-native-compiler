package souther

import (
	"runtime"
	"sync"
	"sync/atomic"
	"unsafe"
)

// Runtime is one library's runtime: the functions that mark its arena and drop it back, and what
// its statuses are numbered.
type Runtime struct {
	identity uintptr
	mark     func() int64
	reset    func(int64)
	statuses Statuses
}

// NewRuntime is the runtime of a library whose souther_mark is at identity.
//
// mark and reset are that library's souther_mark and souther_reset, and stay callable for as long
// as the runtime is used. Two runtimes with one identity are two handles on one arena.
func NewRuntime(identity uintptr, mark func() int64, reset func(int64), statuses Statuses) *Runtime {
	return &Runtime{identity, mark, reset, statuses}
}

// Statuses is what each status the library answers is.
func (r *Runtime) Statuses() Statuses { return r.statuses }

// Identity is the address of the library's souther_mark.
func (r *Runtime) Identity() uintptr { return r.identity }

// Library is a loaded library, of the binding B was made for.
type Library[B any] struct {
	rt       *Runtime
	native   *Native
	symbols  map[string]unsafe.Pointer
	outcomes map[string]int32
}

// NewLibrary is the library whose runtime is rt.
func NewLibrary[B any](rt *Runtime) *Library[B] { return &Library[B]{rt: rt} }

// Runtime is the runtime the library was built with.
func (l *Library[B]) Runtime() *Runtime { return l.rt }

// rootKey is a root run open: which runtime, and the thread it holds.
type rootKey struct {
	identity uintptr
	thread   thread
}

var roots struct {
	sync.Mutex
	open []rootKey
}

func enterRoot(key rootKey) bool {
	roots.Lock()
	defer roots.Unlock()
	for _, it := range roots.open {
		if it.identity == key.identity && it.thread.equal(key.thread) {
			return false
		}
	}
	roots.open = append(roots.open, key)
	return true
}

func leaveRoot(key rootKey) {
	roots.Lock()
	defer roots.Unlock()
	for i, it := range roots.open {
		if it.identity == key.identity && it.thread.equal(key.thread) {
			roots.open = append(roots.open[:i], roots.open[i+1:]...)
			return
		}
	}
}

// Run is a run: what a computation is made in.
//
// It belongs to the goroutine that opened it, and ends when the function it was handed to
// returns, or panics.
type Run[B any] struct {
	lib    *Library[B]
	thread thread
	live   atomic.Bool
	child  *Run[B]
	frames []*frame
	keep   []func()
	// functions is the function value each function of the host's own was made into in this run,
	// so that one handed over again is the value it was made into before.
	functions map[any]unsafe.Pointer
}

// frame is a call into the library that is going on in a run, and what a host implementation it
// called back left for it.
type frame struct{ caught *caught }

// Run opens a root run of the library on this goroutine, hands it to f, and drops everything made
// in it once f has returned, or panicked. It holds the goroutine on its OS thread until then.
//
// It returns f's error, and [ErrAlreadyRunning] where a root run of this runtime is open on this
// thread, through this handle on it or another one.
func (l *Library[B]) Run(f func(*Run[B]) error) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	key := rootKey{l.rt.identity, currentThread()}
	if !enterRoot(key) {
		return ErrAlreadyRunning
	}
	defer leaveRoot(key)
	return within(&Run[B]{lib: l, thread: key.thread}, f)
}

// Scope opens a run inside this one, hands it to f, and drops what was made in it once f has
// returned, or panicked.
//
// Until then this run is not the innermost, and making anything through it panics. A value made
// in this run can still be read in the run inside, and handed to a computation it starts; one
// made inside cannot be used once it has ended.
func (r *Run[B]) Scope(f func(*Run[B]) error) error {
	r.checkMaking()
	inner := &Run[B]{lib: r.lib, thread: r.thread}
	r.child = inner
	defer func() { r.child = nil }()
	return within(inner, f)
}

// within takes a mark, runs f over run, and ends the run when f returns or panics: the run
// expires first, then the arena is dropped back to the mark, and what the run kept is released
// once nothing in the arena reads it.
func within[B any](run *Run[B], f func(*Run[B]) error) error {
	rt := run.lib.rt
	mark := rt.mark()
	run.live.Store(true)
	defer func() {
		run.live.Store(false)
		rt.reset(mark)
		for i := len(run.keep) - 1; i >= 0; i-- {
			run.keep[i]()
		}
		run.keep = nil
	}()
	return f(run)
}

// Keep has release called once this run has ended and the arena has been dropped back, and
// nothing in it reads what is released. It is what holds a host implementation's room and handle
// for as long as the library may call it.
func (r *Run[B]) Keep(release func()) {
	r.checkReading()
	r.keep = append(r.keep, release)
}

// Library is the library this is a run of.
func (r *Run[B]) Library() *Library[B] { return r.lib }

// checkReading panics unless this run is live and on this goroutine's thread.
//
// Expiry is asked first: a thread that has ended may be numbered again, and a run that has ended
// is no longer on any.
func (r *Run[B]) checkReading() {
	if !r.live.Load() {
		misuse(ErrExpired)
	}
	if !r.thread.equal(currentThread()) {
		misuse(ErrRunOnAnotherGoroutine)
	}
}

// checkMaking panics unless this run may have something made through it: it is checked as for
// reading, and is the innermost.
func (r *Run[B]) checkMaking() {
	r.checkReading()
	if r.child != nil {
		misuse(ErrNotTheInnermostRun)
	}
}

// innermost is the run open innermost of the ones inside this one, or this one.
func (r *Run[B]) innermost() *Run[B] {
	for r.child != nil {
		r = r.child
	}
	return r
}

// Making panics with a [*Misuse] unless something may be made through r: it is live, is on this
// goroutine, and is the innermost run. A generated function that hands the library a list or a
// union it builds itself asks first, before anything is made.
func Making[B any](r *Run[B]) { r.checkMaking() }

// NoValue panics with a [*Misuse] for a value that was never made: a nil interface where the
// model has a union, which has no zero value.
func NoValue() { misuse(ErrNoValue) }

// Ref is a value in a library's arena: where it stands, and the run it was made in. It is good
// until that run ends.
type Ref[B any] struct {
	ptr unsafe.Pointer
	run *Run[B]
}

// NewRef is the value at ptr, which the library answered in run. ptr is memory of the library's
// arena and never Go's.
func NewRef[B any](run *Run[B], ptr unsafe.Pointer) Ref[B] {
	run.checkReading()
	if ptr == nil {
		panic("souther: the library answers a value's address")
	}
	return Ref[B]{ptr, run}
}

// Run is the run the value was made in.
func (v Ref[B]) Run() *Run[B] { return v.run }

// Read is where the value stands, to read it through the library that made it: a field, a case,
// its external form. It panics with a [*Misuse] where the run it was made in has ended or is on
// another goroutine.
func (v Ref[B]) Read() unsafe.Pointer {
	v.checkUsable()
	return v.ptr
}

// In is where the value stands, to hand to a computation started in run.
//
// It panics with a [*Misuse] where either run has ended, is on another goroutine, or where run is
// not the innermost. A value made in a run outside run is good here; one made inside has ended.
// It returns [ErrForeignHandle] where another runtime made the value: its address is one of
// another arena, which the computation would read as its own. Two handles on one library are one
// runtime, and a value one made is the other's.
func (v Ref[B]) In(run *Run[B]) (unsafe.Pointer, error) {
	v.checkUsable()
	run.checkMaking()
	if v.run.lib.rt.identity != run.lib.rt.identity {
		return nil, ErrForeignHandle
	}
	return v.ptr, nil
}

func (v Ref[B]) checkUsable() {
	if v.run == nil {
		misuse(ErrNoValue)
	}
	v.run.checkReading()
}

// Call calls into the library: native is the call, made while r is the innermost run, and what it
// answers is the status the call answered.
//
// A host implementation the library calls back meanwhile is lent the innermost run through [Host],
// and a panic it raised is raised again here, once the library has returned.
//
// It returns the failure the status says, where it is not the one saying the call answered: a
// [*HostError] where a host implementation answered one.
func Call[B any](r *Run[B], native func() Status) error {
	r.checkMaking()
	call := &frame{}
	r.frames = append(r.frames, call)
	var status Status
	func() {
		defer func() { r.frames = r.frames[:len(r.frames)-1] }()
		status = native()
	}()
	return r.lib.rt.statuses.outcome(status, call.caught)
}

// Host runs a host implementation of a behavior, which the library has just called back, in the
// run the call it was called back from was made in. origin is the run the implementation was made
// in.
//
// No run is opened: what f makes is answered to the library, which is still in the middle of the
// call, and is dropped with the run that call was made in.
//
// Neither a panic in f nor an error it returns unwinds into the library. Either is kept, and the
// call raises the panic again or answers the error once the library has returned. It answers the
// status the library is to be told: that f answered, or that it failed. A callback with no call
// open into this library on this thread has nowhere to keep what went wrong, and fails.
func Host[B any](origin *Run[B], f func(*Run[B]) error) Status {
	statuses := origin.lib.rt.statuses
	if !origin.live.Load() || !origin.thread.equal(currentThread()) {
		return statuses.hostException
	}
	inner := origin.innermost()
	if len(inner.frames) == 0 {
		return statuses.hostException
	}
	call := inner.frames[len(inner.frames)-1]
	keptOf := func(c caught) {
		// The first is what the library was told of, and what it stopped for.
		if call.caught == nil {
			call.caught = &c
		}
	}
	answered := func() (ok bool) {
		defer func() {
			if payload := recover(); payload != nil {
				keptOf(caught{panicked: true, payload: payload})
				ok = false
			}
		}()
		if err := f(inner); err != nil {
			keptOf(caught{err: err})
			return false
		}
		return true
	}()
	if answered {
		return statuses.answered
	}
	return statuses.hostException
}
