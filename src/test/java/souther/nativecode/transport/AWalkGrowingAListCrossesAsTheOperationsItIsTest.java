package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A fold that only grows a list, which the checker's compiler rewrote into a walk building the list
 * ({@code List.$build}) and the growth inside it ({@code List.$grow}), crosses as those two
 * operations, by the member each is; and the {@code []} it is seeded with crosses at its own type.
 *
 * <p>{@code grown} filters and maps the list {@code tenThousand} builds: the two walks are one
 * after the checker's compiler joined them, and it grows a list of however many elements are below
 * {@code n}. The driver's test runs it for two values of {@code n} and holds how much more room the
 * longer walk took to a bound linear in how many more elements it grew.
 */
class AWalkGrowingAListCrossesAsTheOperationsItIsTest {

    private static final String MODULE = """
            module growing exposing ( grown )

            behavior grown : (n: Int) -> Int

            let tenThousand: List<Int> = {
                let ten = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
                let hundred = List.flatMap((x) -> List.map((y) -> x * 10 + y, ten), ten)
                List.flatMap((x) -> List.map((y) -> x * 100 + y, hundred), hundred)
            }

            let grown (n) = {
                let all = tenThousand
                List.length(List.map((x) -> x + 1, List.filter((x) -> x < n, all)))
            }
            """;

    /** The document the driver's test of a growing walk is held to. */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "growing.transport.json");

    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(CheckedProgram.of(List.of(MODULE))));
    }

    @Test
    void theWalkAndItsGrowthCrossAsTheMembersTheyAre() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(MODULE)));

        assertThat(written)
                .contains("{\"is\":\"emitted\",\"operation\":\"BUILD_LIST\"}")
                .contains("{\"is\":\"emitted\",\"operation\":\"GROW_LIST\"}")
                .doesNotContain("List.$build")
                .doesNotContain("List.$grow");
    }

    @Test
    void theEmptyListAWalkIsSeededWithCrossesAtItsOwnType() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(MODULE)));

        assertThat(written).contains("{\"list\":{\"nothing\":{}}}");
    }
}
