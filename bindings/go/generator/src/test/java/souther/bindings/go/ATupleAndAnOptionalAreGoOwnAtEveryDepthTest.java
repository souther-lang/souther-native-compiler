package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A tuple is a {@code souther.Tuple2} and its like and an optional a {@code souther.Option}, at
 * every depth: an optional of an optional is one Go's pointer could not tell from an optional of
 * nothing.
 */
class ATupleAndAnOptionalAreGoOwnAtEveryDepthTest {

    private static final String SHAPE = """
            module shape exposing ( Line, pair, noted, blank, beyond, pairs, triple )

            data Line = { quantity: Int, note: String? }

            let pair: (Int, Bool) = (3, true)

            let noted = List.get(0, [Line { quantity = 1, note = "gift" }.note])

            let blank = List.get(0, [Line { quantity = 1, note = None }.note])

            let beyond = List.get(1, [Line { quantity = 1, note = None }.note])

            let pairs = [(1, "a"), (2, "b")]

            let triple = (1, "x", (2, true))
            """;

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"

            	"example.com/shapes"
            	"example.com/shapes/shape"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func main() {
            	library, err := shapes.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *shapes.Run) error {
            		pair := made(shape.Pair(r))
            		fmt.Printf("pair: %v %v\\n", pair.V0, pair.V1)
            		noted := made(shape.Noted(r))
            		outer, some := noted.Get()
            		inner, present := outer.Get()
            		fmt.Printf("noted: %v %v %q\\n", some, present, inner)
            		blank := made(shape.Blank(r))
            		outer, some = blank.Get()
            		_, present = outer.Get()
            		fmt.Printf("blank: %v %v\\n", some, present)
            		fmt.Printf("beyond: %v\\n", made(shape.Beyond(r)).IsSome())
            		for _, it := range made(shape.Pairs(r)) {
            			fmt.Printf("pairs: %d %s\\n", it.V0, it.V1)
            		}
            		triple := made(shape.Triple(r))
            		fmt.Printf("triple: %d %s %d %v\\n", triple.V0, triple.V1, triple.V2.V0, triple.V2.V1)
            		line := made(shape.NewLine(r, 2, souther.Some("gift")))
            		note, _ := line.Note().Get()
            		fmt.Printf("line: %d %q\\n", line.Quantity(), note)
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aTupleAndAnOptionalAreGoOwnAtEveryDepth(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, SHAPE, "example.com/shapes", HOST);

        assertThat(said).isEqualTo("""
                pair: 3 true
                noted: true true "gift"
                blank: true false
                beyond: false
                pairs: 1 a
                pairs: 2 b
                triple: 1 x 2 true
                line: 2 "gift"
                """);
    }
}
