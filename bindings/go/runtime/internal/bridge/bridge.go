// Package bridge is what a generated binding does to reach a library and to be reached by it,
// written out by hand over a stand-in library: a typed C shim to call a function through its
// address, a Go function the library calls back through a trampoline, and the cell of C memory
// that holds the Go value it is handed.
//
// It is the shape the generator writes, and the runtime's tests hold it to what cgo allows.
package bridge

/*
#include "testdata/fake.h"

extern souther_status bridgeImplementation(void *, int64_t, int64_t *);

// A function is called through its address: cgo cannot call one. Its type is what the
// declarations say it is, so a shim that does not match them does not compile.
static souther_status call_fake_double(void *fn, int64_t x, int64_t *out) {
	__typeof__(&fake_double) f = fn;
	return f(x, out);
}
static souther_status call_fake_call(void *fn, void *c, int64_t x, int64_t *out) {
	__typeof__(&fake_call) f = fn;
	return f((const souther_capability *)c, x, out);
}
static void call_fake_implement(void *fn, void *into, void *hosted, void *userdata) {
	__typeof__(&fake_implement) f = fn;
	f((souther_capability *)into, (souther_hosted *)hosted, bridgeImplementation, userdata);
}
static void call_fake_bind(void *fn, void *into, void *requirements) {
	__typeof__(&fake_bind) f = fn;
	f((souther_capability *)into, (const souther_capability *const *)requirements);
}
static souther_status call_fake_run(void *fn, void *requirements, int64_t x, int64_t *out) {
	__typeof__(&fake_run) f = fn;
	return f((const souther_capability *const *)requirements, x, out);
}
*/
import "C"

import (
	"unsafe"

	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
)

// Tag is this binding's: its runs and values are of no other's.
type Tag struct{}

type (
	Library = souther.Library[Tag]
	Run     = souther.Run[Tag]
)

// Spec is what this binding needs of a library.
var Spec = souther.Spec{
	Statuses: map[string]souther.Status{
		"ANSWERED":                     0,
		"INJECTION_UNBOUND":            0x7ffffffd,
		"INJECTION_PROTOCOL_VIOLATION": 0x7ffffffe,
		"HOST_EXCEPTION":               0x7fffffff,
	},
	Symbols: []string{"fake_bind", "fake_call", "fake_double", "fake_implement", "fake_run"},
}

// Load opens the library file at path.
func Load(path string) (*Library, error) { return souther.Load[Tag](path, Spec) }

// Double is a behavior of the library.
func Double(r *Run, x int64) (int64, error) {
	fn := r.Library().Symbol("fake_double")
	var out C.int64_t
	err := souther.Call(r, func() souther.Status {
		return souther.Status(C.call_fake_double(fn, C.int64_t(x), &out))
	})
	return int64(out), err
}

// Implementation is a behavior the host implements.
type Implementation interface {
	Apply(r *Run, x int64) (int64, error)
}

// hostedImplementation is what the library's userdata stands for: the implementation, and the run
// its capability was made in, which the library may call for as long as it is open.
type hostedImplementation struct {
	origin *Run
	impl   Implementation
}

// Capability stands for a behavior to hand to a computation that requires it.
type Capability = souther.Capability[Tag]

// Implement makes a capability of impl, held until r ends.
func Implement(r *Run, impl Implementation) Capability {
	fn := r.Library().Symbol("fake_implement")
	return souther.Implemented(r, &hostedImplementation{r, impl}, func(capability, hosted, userdata unsafe.Pointer) {
		C.call_fake_implement(fn, capability, hosted, userdata)
	})
}

// Bind makes a capability of the behavior that asks its first requirement, bound to requires and
// held until r ends.
func Bind(r *Run, requires ...Capability) Capability {
	fn := r.Library().Symbol("fake_bind")
	return souther.Bound(r, requires, func(capability, requirements unsafe.Pointer) {
		C.call_fake_bind(fn, capability, requirements)
	})
}

// Call is the library calling the implementation through the capability.
func Call(r *Run, c Capability, x int64) (int64, error) {
	fn := r.Library().Symbol("fake_call")
	var out C.int64_t
	err := souther.Call(r, func() souther.Status {
		return souther.Status(C.call_fake_call(fn, c.Address(), C.int64_t(x), &out))
	})
	return int64(out), err
}

// RunBound calls the behavior bound to what it requires, handing it what it requires.
func RunBound(r *Run, bound Capability, x int64) (int64, error) {
	fn := r.Library().Symbol("fake_run")
	requirements, err := bound.Requirements(r)
	if err != nil {
		return 0, err
	}
	var out C.int64_t
	err = souther.Call(r, func() souther.Status {
		return souther.Status(C.call_fake_run(fn, requirements, C.int64_t(x), &out))
	})
	return int64(out), err
}

var _ = unsafe.Pointer(nil)
