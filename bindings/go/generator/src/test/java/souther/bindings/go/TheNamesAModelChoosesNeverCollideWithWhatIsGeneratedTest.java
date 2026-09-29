package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A model names a parameter or a field whatever it likes, and a generated function writes names of
 * its own beside them: what holds an address, what counts, what a signature calls the run and a
 * receiver, what a file calls what it imports, and the identifiers Go declares. The two are one scope
 * in Go, so this model names its parameters and fields after every one of them, in every place a
 * generated function takes one (a function, a constructor, a method of what is bound, a host's
 * implementation and what the library calls back), and the packages have to build and answer.
 */
class TheNamesAModelChoosesNeverCollideWithWhatIsGeneratedTest {

    private static final String KMOD = """
            module kmod exposing ( Thing )

            data Thing = { n: Int }
            """;

    private static final String NAMES = """
            module names exposing ( Box, probe, wrap, useIt )

            import kmod ( Thing )

            data Box = { count0: List<Int>, column1: List<String>, element2: String?,
                         m0: Int, answer: Int, err: Int, fn: Int, failed: Int, len: Int, nil: Int,
                         run: Int, value: Int, g0: Int, a0: Int, text0: String, at0: Int }

            behavior probe : (count0: List<Int>, column1: List<String>, element2: List<List<Int>>,
                              m0: Int, answer: Int, err: Int, fn: Int, failed: Int, len: Int,
                              nil: Int, run: Int, value: Int, g0: Int, a0: Int, text0: String,
                              at0: Int, b: Int, v: Int, f: Int, r: Int, souther: Int, lib: Int,
                              raoh: Int, unsafe: Int) -> Int
            let probe (count0, column1, element2, m0, answer, err, fn, failed, len, nil, run, value,
                       g0, a0, text0, at0, b, v, f, r, souther, lib, raoh, unsafe) =
                List.length(count0) + List.length(column1) + m0 + answer + err + fn + failed + len
                    + nil + run + value + g0 + a0 + at0 + b + v + f + r + souther + lib + raoh + unsafe

            behavior wrap : (m0: Int) -> Thing
            let wrap (m0) = Thing { n = m0 }

            behavior pickIt : (count0: List<Int>, answer: Int, err: Int, run: Int, hosted: Int,
                               userdata: Int, handed0: Int, input0: Int) -> Int

            behavior useIt : (n: Int) -> Int
                depends on pickIt
            let useIt (n, pickIt) = pickIt([n], n, n, n, n, n, n, n)
            """;

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"

            	"example.com/hygiene"
            	"example.com/hygiene/names"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            var _ = souther.Some[int]

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            type sum struct{}

            func (sum) Apply(r *hygiene.Run, count0 []int64, answer, err, run, hosted, userdata, handed0, input0 int64) (int64, error) {
            	total := answer + err + run + hosted + userdata + handed0 + input0
            	for _, it := range count0 {
            		total += it
            	}
            	return total, nil
            }

            func main() {
            	library := made(hygiene.Load(os.Args[1]))
            	made(0, library.Run(func(r *hygiene.Run) error {
            		fmt.Printf("probe: %d\\n", made(names.Probe(r, []int64{1, 2}, []string{"a"}, [][]int64{{1}, {2, 3}},
            			1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, "t", 12, 13, 14, 15, 16, 17, 18, 19, 20)))
            		box := made(names.NewBox(r, []int64{1}, []string{"a", "b"}, souther.None[string](),
            			1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, "t", 12))
            		fmt.Printf("box: %d %d %d\\n", len(box.Count0()), len(box.Column1()), box.At0())
            		fmt.Printf("wrapped: %d\\n", made(names.Wrap(r, 5)).N())
            		fmt.Printf("hosted: %d\\n", made(names.BindUseIt(r, names.ImplementPickIt(r, sum{})).Call(r, 3)))
            		return nil
            	}))
            }
            """;

    @Test
    void everyNameTheGeneratorWritesYieldsToTheOnesAModelChose(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, List.of(KMOD, NAMES), "example.com/hygiene", HOST);

        assertThat(said).isEqualTo("""
                probe: 213
                box: 1 2 12
                wrapped: 5
                hosted: 24
                """);
    }
}
