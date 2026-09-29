package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.Generated;
import souther.nativecode.Documents;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A function value is a type of its own for its function type: one the library made, which is
 * called through the library in the run it is handed, or a function of the host's own, which the
 * library calls in the run of the call that reached it. One function of the host's handed over
 * again and again in a run is one function value.
 */
class AGoHostCallsAndHandsOverAFunctionValueTest {

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

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

            func shown[T any](o souther.Option[T]) string {
            	if it, ok := o.Get(); ok {
            		return fmt.Sprintf("Some(%v)", it)
            	}
            	return "None"
            }

            func deep(o souther.Option[souther.Option[int64]]) string {
            	if inner, ok := o.Get(); ok {
            		return "Some(" + shown(inner) + ")"
            	}
            	return "None"
            }

            func main() {
            	library, err := calling.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *calling.Run) error {
            		bump := made(m.Bump(r))
            		fmt.Printf("bump: %d\\n", made(bump.Call(r, 3)))
            		twice := made(m.Twice(r))
            		fmt.Printf("twice bump: %d\\n", made(twice.Call(r, bump, 1)))
            		tripled := m.HostFnIntToInt(func(r *calling.Run, x int64) (int64, error) { return x * 3, nil })
            		fmt.Printf("twice hosted: %d\\n", made(twice.Call(r, tripled, 2)))
            		fmt.Printf("hosted alone: %d\\n", made(tripled.Call(r, 4)))
            		refusing := m.HostFnIntToInt(func(r *calling.Run, x int64) (int64, error) {
            			return 0, fmt.Errorf("refused %d", x)
            		})
            		_, err := twice.Call(r, refusing, 2)
            		var host *souther.HostError
            		if errors.As(err, &host) {
            			fmt.Printf("failed: %v\\n", host.Err)
            		}
            		_, err = made(m.Overflow(r)).Call(r, 1)
            		var abort *souther.Abort
            		if errors.As(err, &abort) {
            			fmt.Printf("overflow: %s\\n", abort.Name)
            		}
            		pairing := made(made(m.Pairing(r)).Call(r, -4))
            		fmt.Printf("pairing: (%d, %v)\\n", pairing.V0, pairing.V1)
            		deepest := made(m.Deep(r))
            		for _, x := range []int64{1, 0, -1} {
            			fmt.Printf("deep %d: %s\\n", x, deep(made(deepest.Call(r, x))))
            		}
            		meet := made(m.Meet(r))
            		fmt.Printf("meet: %s %s\\n",
            			shown(made(meet.Call(r, souther.Tuple2[int64, souther.Option[int64]]{V0: 4, V1: souther.Some(int64(7))}))),
            			shown(made(meet.Call(r, souther.Tuple2[int64, souther.Option[int64]]{V0: 4, V1: souther.None[int64]()}))))
            		lifted := made(made(m.Lifted(r)).Call(r, 10))
            		fmt.Printf("lifted: %d\\n", made(lifted.Call(r, 1)))

            		// One function of the host's handed over again and again in a run is one
            		// function value, which the library keeps calling.
            		for x := int64(0); x < 1000; x++ {
            			made(twice.Call(r, tripled, x))
            		}

            		// What was made in this run is called in a run inside it.
            		var inside int64
            		err = r.Scope(func(inner *calling.Run) error {
            			inside = made(twice.Call(inner, tripled, 5))
            			return nil
            		})
            		fmt.Printf("inside: %d\\n", inside)
            		return err
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aFunctionValueIsATypeOfTheLibrarysAndTheHostsOwn(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                Documents.library(Documents.FUNCTIONS, into.resolve("native"));
        Generated binding = GoHost.generated(library, into.resolve("binding"), "example.com/calling");

        String said = GoHost.ran(into, binding, "example.com/calling", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                bump: 8
                twice bump: 11
                twice hosted: 18
                hosted alone: 12
                failed: refused 2
                overflow: REQUIRED_FORM_HAS_NO_PLACE
                pairing: (-4, false)
                deep 1: Some(Some(1))
                deep 0: Some(None)
                deep -1: None
                meet: Some(7) None
                lifted: 11
                inside: 45
                """);
    }
}
