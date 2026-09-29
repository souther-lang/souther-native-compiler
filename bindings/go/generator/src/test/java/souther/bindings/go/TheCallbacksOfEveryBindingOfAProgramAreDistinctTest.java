package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A function the library calls back is exported from Go, and what is exported is one name in the
 * whole program: a Go binary that holds two bindings, or one binding with two modules whose packages
 * end alike, links one set of them. So no two behaviors of any two bindings may be exported under
 * one name, however their paths and names are spelled.
 */
class TheCallbacksOfEveryBindingOfAProgramAreDistinctTest {

    private static final String AX = """
            module a.x exposing ( choose )

            behavior pick : (n: Int) -> Int

            behavior choose : (n: Int) -> Int
                depends on pick
            let choose (n, pick) = pick(n) + 1
            """;

    private static final String BX = """
            module b.x exposing ( choose )

            behavior pick : (n: Int) -> Int

            behavior choose : (n: Int) -> Int
                depends on pick
            let choose (n, pick) = pick(n) + 2
            """;

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"

            	first "example.com/a-b"
            	fax "example.com/a-b/a/x"
            	fbx "example.com/a-b/b/x"
            	second "example.com/a_b"
            	sax "example.com/a_b/a/x"
            	sbx "example.com/a_b/b/x"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            type tenfold struct{}

            func (tenfold) Apply(r *first.Run, n int64) (int64, error) { return n * 10, nil }

            type hundredfold struct{}

            func (hundredfold) Apply(r *second.Run, n int64) (int64, error) { return n * 100, nil }

            func main() {
            	f := made(first.Load(os.Args[1]))
            	s := made(second.Load(os.Args[2]))
            	made(0, f.Run(func(r *first.Run) error {
            		fmt.Printf("first a.x: %d\\n", made(fax.BindChoose(r, fax.ImplementPick(r, tenfold{})).Call(r, 3)))
            		fmt.Printf("first b.x: %d\\n", made(fbx.BindChoose(r, fbx.ImplementPick(r, tenfold{})).Call(r, 3)))
            		return nil
            	}))
            	made(0, s.Run(func(r *second.Run) error {
            		fmt.Printf("second a.x: %d\\n", made(sax.BindChoose(r, sax.ImplementPick(r, hundredfold{})).Call(r, 3)))
            		fmt.Printf("second b.x: %d\\n", made(sbx.BindChoose(r, sbx.ImplementPick(r, hundredfold{})).Call(r, 3)))
            		return nil
            	}))
            }
            """;

    @Test
    void twoBindingsAndTwoModulesEndingAlikeAreOneProgram(@TempDir Path into) throws Exception {
        NativeCompiler.Library one = NativeCompiler.library(Checked.of(List.of(AX, BX)),
                into.resolve("native1"));
        NativeCompiler.Library two = NativeCompiler.library(Checked.of(List.of(AX, BX)),
                into.resolve("native2"));
        // Two import paths that spell alike once what Go does not take in a name is set aside.
        var first = GoHost.generated(one, into.resolve("binding1"), "example.com/a-b");
        var second = GoHost.generated(two, into.resolve("binding2"), "example.com/a_b");

        String said = GoHost.ran(into, List.of(new GoHost.Binding(first, "example.com/a-b"),
                        new GoHost.Binding(second, "example.com/a_b")), HOST,
                List.of(one.library().toString(), two.library().toString()));

        assertThat(said).isEqualTo("""
                first a.x: 31
                first b.x: 32
                second a.x: 301
                second b.x: 302
                """);
    }
}
