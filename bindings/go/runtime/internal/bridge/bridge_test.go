package bridge_test

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"testing"

	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
	"github.com/souther-lang/souther-native-compiler/bindings/go/runtime/internal/bridge"
)

var libraries string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "souther-bridge")
	if err != nil {
		panic(err)
	}
	libraries = dir
	build := func(name string, flags ...string) {
		out := filepath.Join(dir, name+sharedExtension())
		args := append([]string{"-shared", "-fPIC", "-o", out}, flags...)
		args = append(args, "testdata/fake.c")
		if output, err := exec.Command("cc", args...).CombinedOutput(); err != nil {
			panic(string(output))
		}
	}
	build("fake")
	build("second")
	build("nodouble", "-DNO_DOUBLE")
	code := m.Run()
	_ = os.RemoveAll(dir)
	os.Exit(code)
}

func sharedExtension() string {
	if runtime.GOOS == "darwin" {
		return ".dylib"
	}
	return ".so"
}

func path(name string) string { return filepath.Join(libraries, name+sharedExtension()) }

func load(t *testing.T, name string) *bridge.Library {
	t.Helper()
	lib, err := bridge.Load(path(name))
	if err != nil {
		t.Fatal(err)
	}
	return lib
}

func TestALibraryIsCalledThroughItsAddress(t *testing.T) {
	lib := load(t, "fake")
	err := lib.Run(func(r *bridge.Run) error {
		got, err := bridge.Double(r, 21)
		if err != nil || got != 42 {
			t.Errorf("Double(21) = %d, %v", got, err)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
}

func TestAFileThatIsNoLibraryIsNotLoaded(t *testing.T) {
	notLibrary := filepath.Join(libraries, "text.dylib")
	if err := os.WriteFile(notLibrary, []byte("not a library"), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := bridge.Load(notLibrary); err == nil {
		t.Fatal("loaded a text file")
	}
	if _, err := bridge.Load(filepath.Join(libraries, "absent.dylib")); err == nil {
		t.Fatal("loaded a file that is not there")
	}
}

func TestALibraryWithoutAFunctionTheBindingCallsIsRefused(t *testing.T) {
	_, err := bridge.Load(path("nodouble"))
	var missing *souther.MissingSymbols
	if !errors.As(err, &missing) || len(missing.Names) != 1 || missing.Names[0] != "fake_double" {
		t.Fatalf("got %v", err)
	}
}

func TestTwoLibrariesInOneProgramEachKeepTheirOwnRuntime(t *testing.T) {
	first, second := load(t, "fake"), load(t, "second")
	sameFile := load(t, "fake")
	if first.Runtime().Identity() == second.Runtime().Identity() {
		t.Error("two files are one runtime")
	}
	if first.Runtime().Identity() != sameFile.Runtime().Identity() {
		t.Error("one file, loaded twice, is two runtimes")
	}
	// Each answers through its own functions, and a run of one is open inside a run of the other.
	err := first.Run(func(a *bridge.Run) error {
		return second.Run(func(b *bridge.Run) error {
			x, errA := bridge.Double(a, 1)
			y, errB := bridge.Double(b, 2)
			if errA != nil || errB != nil || x != 2 || y != 4 {
				t.Errorf("got %d %v, %d %v", x, errA, y, errB)
			}
			return nil
		})
	})
	if err != nil {
		t.Fatal(err)
	}
	// One file loaded twice has one arena: a root run of it while the other is open is refused.
	err = first.Run(func(*bridge.Run) error {
		return sameFile.Run(func(*bridge.Run) error { return nil })
	})
	if !errors.Is(err, souther.ErrAlreadyRunning) {
		t.Errorf("got %v", err)
	}
}

// double is an implementation that answers twice what it is asked.
type double struct{}

func (double) Apply(r *bridge.Run, x int64) (int64, error) { return x * 2, nil }

type failing struct{ err error }

func (f failing) Apply(*bridge.Run, int64) (int64, error) { return 0, f.err }

type panicking struct{}

func (panicking) Apply(*bridge.Run, int64) (int64, error) { panic("in the host") }

type function func(r *bridge.Run, x int64) (int64, error)

func (f function) Apply(r *bridge.Run, x int64) (int64, error) { return f(r, x) }

func TestALibraryCallsAHostImplementationBack(t *testing.T) {
	lib := load(t, "fake")
	_ = lib.Run(func(r *bridge.Run) error {
		capability := bridge.Implement(r, double{})
		got, err := bridge.Call(r, capability, 5)
		if err != nil || got != 10 {
			t.Errorf("Call = %d, %v", got, err)
		}
		return nil
	})
}

func TestAHostImplementationsErrorComesBackWhereTheLibraryReturns(t *testing.T) {
	lib := load(t, "fake")
	no := errors.New("no such product")
	_ = lib.Run(func(r *bridge.Run) error {
		_, err := bridge.Call(r, bridge.Implement(r, failing{no}), 1)
		var host *souther.HostError
		if !errors.As(err, &host) || !errors.Is(err, no) {
			t.Errorf("got %v", err)
		}
		return nil
	})
}

func TestAHostImplementationsPanicIsRaisedAgainWhereTheLibraryReturns(t *testing.T) {
	lib := load(t, "fake")
	_ = lib.Run(func(r *bridge.Run) error {
		defer func() {
			if got := recover(); got != "in the host" {
				t.Errorf("raised %v", got)
			}
		}()
		capability := bridge.Implement(r, panicking{})
		_, _ = bridge.Call(r, capability, 1)
		t.Error("the call returned")
		return nil
	})
}

func TestAHostImplementationThatCallsTheLibraryBackHasEachFailureKeptWithItsOwnCall(t *testing.T) {
	lib := load(t, "fake")
	outer, inner := errors.New("outer"), errors.New("inner")
	_ = lib.Run(func(r *bridge.Run) error {
		var innerErr error
		var again bridge.Capability
		again = bridge.Implement(r, failing{inner})
		outerImpl := bridge.Implement(r, function(func(in *bridge.Run, x int64) (int64, error) {
			// Go called into C, which called Go, which calls into C, which calls Go again.
			_, innerErr = bridge.Call(in, again, x)
			got, err := bridge.Double(in, x)
			if err != nil || got != 2*x {
				t.Errorf("the library answers inside a callback: %d, %v", got, err)
			}
			return 0, outer
		}))
		_, outerErr := bridge.Call(r, outerImpl, 3)
		if !errors.Is(innerErr, inner) || errors.Is(innerErr, outer) {
			t.Errorf("inner: %v", innerErr)
		}
		if !errors.Is(outerErr, outer) || errors.Is(outerErr, inner) {
			t.Errorf("outer: %v", outerErr)
		}
		return nil
	})
}

func TestAReentrantCallbackFiveDeep(t *testing.T) {
	lib := load(t, "fake")
	_ = lib.Run(func(r *bridge.Run) error {
		var capability bridge.Capability
		depth := 0
		capability = bridge.Implement(r, function(func(in *bridge.Run, x int64) (int64, error) {
			depth++
			if x == 0 {
				return 100, nil
			}
			below, err := bridge.Call(in, capability, x-1)
			return below + 1, err
		}))
		got, err := bridge.Call(r, capability, 4)
		if err != nil || got != 104 || depth != 5 {
			t.Errorf("got %d, %v after %d calls", got, err, depth)
		}
		return nil
	})
}

func TestWhatAnImplementationMadeIsHeldUntilTheRunEnds(t *testing.T) {
	lib := load(t, "fake")
	var capability bridge.Capability
	_ = lib.Run(func(r *bridge.Run) error {
		capability = bridge.Implement(r, double{})
		return nil
	})
	// The run ended, and its rooms were freed: a second run works and holds nothing over.
	_ = lib.Run(func(r *bridge.Run) error {
		got, err := bridge.Call(r, bridge.Implement(r, double{}), 7)
		if err != nil || got != 14 {
			t.Errorf("got %d, %v", got, err)
		}
		return nil
	})
	_ = capability
}
