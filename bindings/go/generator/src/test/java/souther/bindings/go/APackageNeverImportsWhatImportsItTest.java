package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Souther's modules depend on one another without a cycle, and Go's packages may not. A type that
 * no declaration names, a union or a function, belongs to the module that says it, so two modules
 * that say one alike each have their own and the package of the one that depends on the other is
 * the only import between them. What a type of a module is written as is never a reason for a
 * package to import another that is not one of the module's dependencies.
 */
class APackageNeverImportsWhatImportsItTest {

    private static final String Z = """
            module z exposing ( Item, numberOrText )

            data Item = { n: Int }

            behavior numberOrText : (n: Int) -> Int | String
            let numberOrText (n) = if n > 0 then n else "none"
            """;

    private static final String A = """
            module a exposing ( wrap, either )

            import z ( Item )

            behavior wrap : (n: Int) -> Item
            let wrap (n) = Item { n = n }

            behavior either : (n: Int) -> Int | String
            let either (n) = if n > 0 then n else "nothing"
            """;

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"

            	"example.com/cycle"
            	"example.com/cycle/a"
            	"example.com/cycle/z"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func main() {
            	library := made(cycle.Load(os.Args[1]))
            	made(0, library.Run(func(r *cycle.Run) error {
            		for _, n := range []int64{1, 0} {
            			switch it := made(z.NumberOrText(r, n)).(type) {
            			case z.IntOrStringInt:
            				fmt.Printf("z: int %d\\n", it.Value)
            			case z.IntOrStringString:
            				fmt.Printf("z: text %s\\n", it.Value)
            			}
            			switch it := made(a.Either(r, n)).(type) {
            			case a.IntOrStringInt:
            				fmt.Printf("a: int %d\\n", it.Value)
            			case a.IntOrStringString:
            				fmt.Printf("a: text %s\\n", it.Value)
            			}
            		}
            		fmt.Printf("wrapped: %d\\n", made(a.Wrap(r, 5)).N())
            		return nil
            	}))
            }
            """;

    @Test
    void twoModulesSayingOneUnionAlikeEachHaveTheirOwn(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, java.util.List.of(Z, A), "example.com/cycle", HOST);

        assertThat(said).isEqualTo("""
                z: int 1
                a: int 1
                z: text none
                a: text nothing
                wrapped: 5
                """);
    }
}
