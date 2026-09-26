package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.nativecode.Checked;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A function handed to a kernel beside an empty list or an absent value takes what has no value,
 * so it never runs; the kernel is still handed it (upstream's {@code Core.Call.functionArgument}
 * answers {@code HANDED_OVER} for every kernel). The driver's test of such a call, over this
 * document, makes the function the kernel is handed one computed around its block and holds the run
 * to working that computation out, as the JVM does before it makes the lambda.
 */
class AFunctionAKernelNeverRunsIsStillHandedOverTest {

    private static final String MODULE = """
            module unran exposing ( sorted, mapped, byFold )

            behavior sorted : (a: Int) -> Int
            let sorted (a) = List.length(List.sortBy((x) -> a, [])) + a

            behavior byFold : (a: Int) -> Int
            let byFold (a) = List.length(List.sortBy((x) -> List.fold((acc, y) -> acc, 0, []), [])) + a

            behavior mapped : (a: Int) -> Int
            let mapped (a) = match Option.map((x) -> a, List.get(0, [])) with
                | Some x -> x
                | None -> a

            example sorted
                | "nothing to sort" : (5) -> 5

            example mapped
                | "nothing to map" : (5) -> 5

            example byFold
                | "a key no call runs" : (5) -> 5
            """;

    /** The document the driver's test of a function a kernel never runs is held to. */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "unran.transport.json");

    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(Checked.of(List.of(MODULE))));
    }

    /** The function crosses as the block it is, taking what has no value. */
    @Test
    void theFunctionCrossesAsABlockOverWhatHasNoValue() {
        String written = ProgramWriter.written(Checked.of(List.of(MODULE)));

        assertThat(written)
                .contains("\"kernel\":\"list.sortBy\"")
                .contains("\"kernel\":\"option.map\"")
                .contains("{\"fn\":{\"takes\":[{\"nothing\":{}}]");
    }
}
