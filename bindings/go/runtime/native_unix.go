//go:build unix

package souther

/*
#cgo linux LDFLAGS: -ldl
#include <dlfcn.h>
#include <stdint.h>
#include <stdlib.h>

static int64_t call_mark(void *fn) { return ((int64_t (*)(void))fn)(); }
static void call_reset(void *fn, int64_t mark) { ((void (*)(int64_t))fn)(mark); }
*/
import "C"

import (
	"fmt"
	"runtime/cgo"
	"slices"
	"strings"
	"unsafe"
)

// Native is a library file opened by path.
//
// Every Souther library exports the same runtime functions, so a symbol is only looked up through
// the handle of the file it is wanted from, which is opened RTLD_LOCAL and never linked: two
// libraries in one program each keep their own arena.
type Native struct{ handle unsafe.Pointer }

// Open loads the library file at path.
func Open(path string) (*Native, error) {
	name := C.CString(path)
	defer C.free(unsafe.Pointer(name))
	handle := C.dlopen(name, C.RTLD_NOW|C.RTLD_LOCAL)
	if handle == nil {
		return nil, fmt.Errorf("souther: cannot load %s: %s", path, C.GoString(C.dlerror()))
	}
	return &Native{handle}, nil
}

// Symbol is where the library file has name, and whether it has.
func (n *Native) Symbol(name string) (unsafe.Pointer, bool) {
	c := C.CString(name)
	defer C.free(unsafe.Pointer(c))
	at := C.dlsym(n.handle, c)
	return at, at != nil
}

// Spec is what a generated binding needs of a library: the ABI generation its manifest was
// written for, the statuses the manifest numbers, and every function the binding calls.
type Spec struct {
	ABI      int
	Statuses map[string]Status
	Symbols  []string
}

// AbiMismatch is a library that is not of the ABI generation the binding was generated for.
type AbiMismatch struct {
	Path string
	Want int
}

func (e *AbiMismatch) Error() string {
	return fmt.Sprintf("souther: %s is not a library of ABI generation %d, which this binding was generated for", e.Path, e.Want)
}

// MissingSymbols are functions the binding calls that the library file does not have. It is not
// the library the binding was generated from.
type MissingSymbols struct {
	Path  string
	Names []string
}

func (e *MissingSymbols) Error() string {
	return fmt.Sprintf("souther: %s has no %s, and is not the library this binding was generated from",
		e.Path, strings.Join(e.Names, ", "))
}

// Load opens the library file at path as the library of a binding of the tag B.
//
// It checks the ABI generation the binding was generated for, and that every function it calls is
// there, so that a file of another library is refused here and not where a call reaches it. It
// cannot tell a library from another that has the same functions under the same names with other
// signatures, so the file is the library the binding was generated from: the caller holds that,
// as for any unsafe load.
func Load[B any](path string, spec Spec) (*Library[B], error) {
	statuses, err := NewStatuses(spec.Statuses)
	if err != nil {
		return nil, err
	}
	native, err := Open(path)
	if err != nil {
		return nil, err
	}
	if _, ok := native.Symbol(fmt.Sprintf("souther_runtime_abi_%d", spec.ABI)); !ok {
		return nil, &AbiMismatch{path, spec.ABI}
	}
	symbols := make(map[string]unsafe.Pointer, len(spec.Symbols)+2)
	var missing []string
	for _, name := range append([]string{"souther_mark", "souther_reset"}, spec.Symbols...) {
		if at, ok := native.Symbol(name); ok {
			symbols[name] = at
		} else if !slices.Contains(missing, name) {
			missing = append(missing, name)
		}
	}
	if len(missing) > 0 {
		return nil, &MissingSymbols{path, missing}
	}
	mark, reset := symbols["souther_mark"], symbols["souther_reset"]
	rt := NewRuntime(uintptr(mark),
		func() int64 { return int64(C.call_mark(mark)) },
		func(at int64) { C.call_reset(reset, C.int64_t(at)) },
		statuses)
	lib := NewLibrary[B](rt)
	lib.native, lib.symbols = native, symbols
	return lib, nil
}

// Room is memory of a host implementation for the library to keep, taken from C memory and never
// from Go's heap, since the library holds it for as long as it may call the implementation and
// cgo does not let C keep a Go pointer. It is zeroed, and freed once this run ends.
func (r *Run[B]) Room(size uintptr) unsafe.Pointer {
	r.checkReading()
	at := C.calloc(1, C.size_t(size))
	if at == nil {
		panic("souther: out of memory")
	}
	r.keep = append(r.keep, func() { C.free(at) })
	return at
}

// Userdata is what a host implementation is handed back by the library: a cell of C memory
// holding a [cgo.Handle] of v. The library keeps the cell and never a Go pointer. The cell is
// freed and the handle deleted once this run ends, after which the library does not call it.
func (r *Run[B]) Userdata(v any) unsafe.Pointer {
	r.checkReading()
	cell := (*cgo.Handle)(C.calloc(1, C.size_t(unsafe.Sizeof(cgo.Handle(0)))))
	if cell == nil {
		panic("souther: out of memory")
	}
	*cell = cgo.NewHandle(v)
	r.keep = append(r.keep, func() { cell.Delete(); C.free(unsafe.Pointer(cell)) })
	return unsafe.Pointer(cell)
}

// UserdataValue is the v that [Run.Userdata] made userdata of.
func UserdataValue(userdata unsafe.Pointer) any {
	return (*(*cgo.Handle)(userdata)).Value()
}

// Symbol is where the library has name, which [Load] resolved: a function the binding was
// generated to call. It panics for a name the binding did not say it calls.
func (l *Library[B]) Symbol(name string) unsafe.Pointer {
	at, ok := l.symbols[name]
	if !ok {
		panic(fmt.Sprintf("souther: the binding calls %s and did not say so to Load", name))
	}
	return at
}
