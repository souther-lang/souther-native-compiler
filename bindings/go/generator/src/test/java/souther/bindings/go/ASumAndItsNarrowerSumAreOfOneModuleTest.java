package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A sum's cases are declared with it, so a sum and a sum of some of its cases are of one module, and
 * what takes one for the other is written in that module's package and names nothing else.
 *
 * <p>That is a fact of the language and not of the binding, and the binding is written on it: the
 * helper that takes a narrower sum for a wider one imports nothing the model does not refer to. A
 * language that let a module make a sum of another module's cases would relate two packages by a
 * helper that no dependency asks for, and the first test fails then, to say the binding has to be
 * looked at again.
 */
class ASumAndItsNarrowerSumAreOfOneModuleTest {

    private static final String A = """
            module a exposing ( X, Y, Z, Wide, Narrow, wideOf, narrowOf )

            data X
            data Y
            data Z
            data Wide = X | Y | Z
            data Narrow = X | Y

            behavior wideOf : (n: Int) -> Wide
            let wideOf (n) = if n > 1 then Z else if n > 0 then Y else X

            behavior narrowOf : (n: Int) -> Narrow
            let narrowOf (n) = if n > 0 then Y else X
            """;

    private static final String OTHER = """
            module z exposing ( Narrow )

            import a ( X, Y )

            data Narrow = X | Y
            """;

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"

            	"example.com/narrower"
            	"example.com/narrower/a"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func said(wide a.Wide) string {
            	switch wide.Case().(type) {
            	case a.X:
            		return "X"
            	case a.Y:
            		return "Y"
            	case a.Z:
            		return "Z"
            	}
            	return "?"
            }

            func main() {
            	library := made(narrower.Load(os.Args[1]))
            	made(0, library.Run(func(r *narrower.Run) error {
            		for _, n := range []int64{0, 1, 2} {
            			fmt.Printf("wide %d: %s\\n", n, said(made(a.WideOf(r, n))))
            		}
            		for _, n := range []int64{0, 1} {
            			fmt.Printf("narrow %d as wide: %s\\n", n, said(a.WideFromNarrow(made(a.NarrowOf(r, n)))))
            		}
            		return nil
            	}))
            }
            """;

    @Test
    void aModuleCannotMakeASumOfAnotherModulesCases() {
        assertThatThrownBy(() -> Checked.of(List.of(A, OTHER))).hasMessageContaining("E1606");
    }

    @Test
    void aNarrowerSumIsTakenForTheWiderOneWhereTheyAreWritten(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, List.of(A), "example.com/narrower", HOST);

        assertThat(said).isEqualTo("""
                wide 0: X
                wide 1: Y
                wide 2: Z
                narrow 0 as wide: X
                narrow 1 as wide: Y
                """);
    }
}
