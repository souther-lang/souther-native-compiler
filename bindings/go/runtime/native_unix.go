//go:build unix

package souther

/*
#cgo linux LDFLAGS: -ldl
#include <dlfcn.h>
#include <stdint.h>
#include <stdlib.h>

static uint32_t call_abi_generation(void *fn) { return ((uint32_t (*)(void))fn)(); }
static int64_t call_scope_open(void *fn) { return ((int64_t (*)(void))fn)(); }
static uint8_t call_scope_close(void *fn, int64_t scope) { return ((uint8_t (*)(int64_t))fn)(scope); }
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime/cgo"
	"slices"
	"strings"
	"unsafe"
)

// A cgo.Handle is a uintptr, and a cell of C memory holds one where the library keeps it; and
// what a C pointer takes is what a Go pointer does, since both are written into C memory here.
var (
	_ = [1]struct{}{}[int(unsafe.Sizeof(cgo.Handle(0)))-int(C.sizeof_uintptr_t)]
	_ = [1]struct{}{}[int(unsafe.Sizeof(unsafe.Pointer(nil)))-int(C.sizeof_uintptr_t)]
)

// nativeFile is a library file opened by path.
//
// Every Souther library exports the same runtime functions, so a symbol is only looked up through
// the handle of the file it is wanted from, which is opened RTLD_LOCAL and never linked: two
// libraries in one program each keep their own arena.
type nativeFile struct{ handle unsafe.Pointer }

// open loads the library file at path.
func open(path string) (*nativeFile, error) {
	name := C.CString(path)
	defer C.free(unsafe.Pointer(name))
	handle := C.dlopen(name, C.RTLD_NOW|C.RTLD_LOCAL)
	if handle == nil {
		return nil, fmt.Errorf("souther: cannot load %s: %s", path, C.GoString(C.dlerror()))
	}
	return &nativeFile{handle}, nil
}

// close unloads the file. It is called only where nothing was taken from it.
func (n *nativeFile) close() { C.dlclose(n.handle) }

// symbol is where the library file has name, and whether it has.
func (n *nativeFile) symbol(name string) (unsafe.Pointer, bool) {
	c := C.CString(name)
	defer C.free(unsafe.Pointer(c))
	at := C.dlsym(n.handle, c)
	return at, at != nil
}

// Spec is what a generated binding needs of a library: the statuses the manifest numbers, what a
// reading comes to, and every function the binding calls.
type Spec struct {
	Layout   Layout
	Statuses map[string]Status
	// Outcomes is what a reading comes to, by the names the manifest gives them.
	Outcomes map[string]int32
	Symbols  []string
}

// MissingSymbols are functions the binding calls that the library file does not have. It is not
// the library the binding was generated from, or is one of another ABI generation: a function
// generated for a behavior or a type has its generation in its name, so a library of another one
// has none of them.
type MissingSymbols struct {
	Path  string
	Names []string
}

func (e *MissingSymbols) Error() string {
	return fmt.Sprintf("souther: %s has no %s, and is not the library this binding was generated from",
		e.Path, strings.Join(e.Names, ", "))
}

// runtimeFunctions are the library's runtime functions this package calls, which are the ABI
// generation's: the same in every library of [ABIGeneration], and in no manifest.
var runtimeFunctions = []string{
	"souther_scope_open",
	"souther_scope_close",
	"souther_string_of_utf8",
	"souther_string_length",
	"souther_string_bytes",
	"souther_decimal_of_parts",
	"souther_decimal_unscaled",
	"souther_decimal_scale",
	"souther_date_of_parts",
	"souther_date_parts",
	"souther_time_of_parts",
	"souther_time_parts",
	"souther_datetime_of_parts",
	"souther_datetime_parts",
	"souther_instant_of_parts",
	"souther_instant_parts",
	"souther_decoded_outcome",
	"souther_decoded_value",
	"souther_decoded_malformed_at",
	"souther_decoded_issue_count",
	"souther_decoded_issue",
	"souther_issue_code",
	"souther_issue_message_key",
	"souther_issue_path",
	"souther_issue_meta",
}

// ABIGeneration is the ABI generation this package calls a library as, which [Load] refuses any
// other of.
const ABIGeneration uint32 = 9

// UnsupportedGeneration is a library file of another ABI generation than [ABIGeneration]: Found is
// the one it answers to, and nought where it has no query for one, which a library of generation 8
// or earlier does not.
type UnsupportedGeneration struct {
	Path  string
	Found uint32
}

func (e *UnsupportedGeneration) Error() string {
	if e.Found == 0 {
		return fmt.Sprintf("souther: %s says no ABI generation, so it is of generation 8 or earlier, "+
			"and this runtime calls a library of generation %d", e.Path, ABIGeneration)
	}
	return fmt.Sprintf("souther: %s answers to ABI generation %d, and this runtime calls a library "+
		"of generation %d", e.Path, e.Found, ABIGeneration)
}

// Load opens the library file at path as the library of a binding of the tag B.
//
// It asks the file which ABI generation it answers to before anything else, by the one query every
// generation has (souther_abi_generation), and returns an [*UnsupportedGeneration] where it is not
// [ABIGeneration]. It then checks that every function the binding calls is there, so that a file
// of another library is refused here and not where a call reaches it. It cannot tell a library
// from another of the same generation that has the same functions under the same names with other
// signatures, so the file is the library the binding was generated from: the caller holds that, as
// for any unsafe load.
func Load[B any](path string, spec Spec) (*Library[B], error) {
	statuses, err := newStatuses(spec.Statuses)
	if err != nil {
		return nil, err
	}
	// What a reading came to is told by these two, and any other is that the text was not JSON.
	for _, name := range []string{"VALUE", "ISSUES"} {
		if _, ok := spec.Outcomes[name]; !ok {
			return nil, &UnnamedOutcome{name}
		}
	}
	if l := spec.Layout; l.Pointer == 0 || l.Capability == 0 || l.Hosted == 0 || l.HostedFunction == 0 {
		return nil, errors.New("souther: the binding says nothing of the size of what a host lays out room for")
	}
	native, err := open(path)
	if err != nil {
		return nil, err
	}
	// The file is unloaded where nothing comes of loading it: nothing has been handed out of it yet.
	loaded := false
	defer func() {
		if !loaded {
			native.close()
		}
	}()
	query, ok := native.symbol("souther_abi_generation")
	if !ok {
		return nil, &UnsupportedGeneration{Path: path}
	}
	if found := uint32(C.call_abi_generation(query)); found != ABIGeneration {
		return nil, &UnsupportedGeneration{Path: path, Found: found}
	}
	symbols := make(map[string]unsafe.Pointer, len(spec.Symbols)+2)
	var missing []string
	for _, name := range append(slices.Clone(runtimeFunctions), spec.Symbols...) {
		if at, ok := native.symbol(name); ok {
			symbols[name] = at
		} else if !slices.Contains(missing, name) {
			missing = append(missing, name)
		}
	}
	if len(missing) > 0 {
		return nil, &MissingSymbols{path, missing}
	}
	open, close := symbols["souther_scope_open"], symbols["souther_scope_close"]
	rt := newRuntime(uintptr(open),
		func() int64 { return int64(C.call_scope_open(open)) },
		func(scope int64) bool { return C.call_scope_close(close, C.int64_t(scope)) != 0 },
		statuses)
	lib := newLibrary[B](rt)
	lib.native, lib.symbols, lib.outcomes, lib.layout = native, symbols, spec.Outcomes, spec.Layout
	// From here the library's functions are in use for as long as the program is: nothing says when
	// the last value or function pointer taken from it is gone, so it is kept loaded.
	loaded = true
	return lib, nil
}

// room is memory of a host implementation for the library to keep, of size bytes, taken from C
// memory and never from Go's heap, since the library holds it for as long as it may call the
// implementation and cgo does not let C keep a Go pointer. It is zeroed, and freed once this run
// ends. The size is the declarations' ([Layout]).
func (r *Run[B]) room(size uintptr) unsafe.Pointer {
	r.checkReading()
	at := C.calloc(1, C.size_t(size))
	if at == nil {
		panic("souther: out of memory")
	}
	r.hold(func() { C.free(at) })
	return at
}

// userdata is what a host implementation is handed back by the library: a cell of C memory
// holding a [cgo.Handle] of v. The library keeps the cell and never a Go pointer. The cell is
// freed and the handle deleted once this run ends, after which the library does not call it.
func (r *Run[B]) userdata(v any) unsafe.Pointer {
	r.checkReading()
	cell := (*cgo.Handle)(C.calloc(1, C.sizeof_uintptr_t))
	if cell == nil {
		panic("souther: out of memory")
	}
	*cell = cgo.NewHandle(v)
	r.hold(func() { cell.Delete(); C.free(unsafe.Pointer(cell)) })
	return unsafe.Pointer(cell)
}

// UserdataValue is the v that [Run.userdata] made userdata of.
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
