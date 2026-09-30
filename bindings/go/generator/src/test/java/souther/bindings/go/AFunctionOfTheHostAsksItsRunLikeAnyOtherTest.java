package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;
import souther.nativecode.Documents;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Calling a function value makes a computation, so the run it is called in is asked whether one may
 * be made through it, whichever way the function is made: one of the host's that touches no run
 * and hands over no list or union is no reason to skip what a function of the library's is asked.
 */
class AFunctionOfTheHostAsksItsRunLikeAnyOtherTest {

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"
            	"sync"

            	"example.com/calling"
            	"example.com/calling/m"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            // refused is whether f panics as a misuse of a run, of the kind said.
            func refused(f func(), kind error) bool {
            	refused := false
            	func() {
            		defer func() {
            			var misuse *souther.Misuse
            			if err, ok := recover().(error); ok && errors.As(err, &misuse) && errors.Is(misuse, kind) {
            				refused = true
            			}
            		}()
            		f()
            	}()
            	return refused
            }

            func main() {
            	library := made(calling.Load(os.Args[1]))
            	// One that asks nothing of the run it is called in, and makes nothing in it.
            	plain := m.HostFnIntToInt(func(r *calling.Run, x int64) (int64, error) { return x + 1, nil })

            	var ended *calling.Run
            	made(0, library.Run(func(r *calling.Run) error {
            		ended = r
            		return nil
            	}))
            	fmt.Printf("expired: %v\\n", refused(func() { _, _ = plain.Call(ended, 1) }, souther.ErrExpired))

            	made(0, library.Run(func(r *calling.Run) error {
            		var wg sync.WaitGroup
            		wg.Add(1)
            		go func() {
            			defer wg.Done()
            			fmt.Printf("another goroutine: %v\\n",
            				refused(func() { _, _ = plain.Call(r, 1) }, souther.ErrRunOnAnotherGoroutine))
            		}()
            		wg.Wait()
            		made(0, r.Scope(func(inner *calling.Run) error {
            			fmt.Printf("not the innermost: %v\\n",
            				refused(func() { _, _ = plain.Call(r, 1) }, souther.ErrNotTheInnermostRun))
            			return nil
            		}))
            		fmt.Printf("in its own run: %d\\n", made(plain.Call(r, 1)))
            		return nil
            	}))
            }
            """;

    @Test
    void aFunctionOfTheHostIsCalledOnlyWhereAFunctionOfTheLibraryIs(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = Documents.library(Documents.FUNCTIONS, into.resolve("native"));
        Generated binding = GoHost.generated(library, into.resolve("binding"), "example.com/calling");

        String said = GoHost.ran(into, binding, "example.com/calling", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                expired: true
                another goroutine: true
                not the innermost: true
                in its own run: 2
                """);
    }
}
