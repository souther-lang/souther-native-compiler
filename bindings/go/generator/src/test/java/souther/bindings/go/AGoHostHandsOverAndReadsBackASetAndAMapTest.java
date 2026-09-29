package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A set crosses as a slice of its members and a map as a slice of its entries, each a tuple of the
 * key and the value, in the order the library has them, which the language says nothing of.
 */
class AGoHostHandsOverAndReadsBackASetAndAMapTest {

    private static final String TALLY = """
            module tally exposing ( Tally, sized, counted, total, tallied )

            data Tally = { tags: Set<String>, counts: Map<String, Int> }

            behavior sized : (s: Set<Int>) -> Int
            let sized (s) = Set.size(s)

            behavior counted : (words: List<String>) -> Map<String, Int>
            let counted (words) =
                List.fold((acc, w) -> Map.updateOrInsert(w, 1, n -> n + 1, acc), Map.empty, words)

            behavior total : (m: Map<String, Int>) -> Int
            let total (m) = Map.fold((acc, k, v) -> acc + v, 0, m)

            behavior tallied : (t: Tally) -> Int
            let tallied (t) = Set.size(t.tags) * 100 + Map.size(t.counts)
            """;

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"
            	"slices"

            	"example.com/tallybinding"
            	"example.com/tallybinding/tally"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func entry(k string, v int64) souther.Tuple2[string, int64] {
            	return souther.Tuple2[string, int64]{V0: k, V1: v}
            }

            func main() {
            	library, err := tallybinding.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *tallybinding.Run) error {
            		fmt.Printf("sized: %d\\n", made(tally.Sized(r, []int64{3, 1, 3, 2})))
            		counted := made(tally.Counted(r, []string{"b", "a", "b"}))
            		slices.SortFunc(counted, func(a, b souther.Tuple2[string, int64]) int {
            			return compare(a.V0, b.V0)
            		})
            		fmt.Printf("counted: %v\\n", counted)
            		fmt.Printf("total: %d\\n", made(tally.Total(r, []souther.Tuple2[string, int64]{entry("a", 1), entry("b", 2), entry("a", 5)})))

            		held := made(tally.NewTally(r, []string{"y", "x", "y"}, []souther.Tuple2[string, int64]{entry("k", 1), entry("j", 2)}))
            		tags := held.Tags()
            		slices.Sort(tags)
            		fmt.Printf("tags: %v\\n", tags)
            		fmt.Printf("tallied: %d\\n", made(tally.Tallied(r, held)))
            		fmt.Printf("written: %s\\n", held.Encode())
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }

            func compare(a, b string) int {
            	switch {
            	case a < b:
            		return -1
            	case a > b:
            		return 1
            	}
            	return 0
            }
            """;

    @Test
    void aSetAndAMapCrossAsSlicesOfWhatTheyHold(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, TALLY, "example.com/tallybinding", HOST);

        assertThat(said).isEqualTo("""
                sized: 3
                counted: [{a 1} {b 2}]
                total: 7
                tags: [x y]
                tallied: 202
                written: {"tags":["x","y"],"counts":{"j":2,"k":1}}
                """);
    }
}
