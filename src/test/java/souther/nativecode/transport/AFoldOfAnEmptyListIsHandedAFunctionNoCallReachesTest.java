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
 * A fold over an empty list literal that is not rewritten into a walk hands its helper a function
 * over what has no value, which it never applies. The checker's backend hands {@code Fn.NEVER} in
 * its place; the driver hands the helper a function no call reaches, and does not lower the arm of
 * the helper that would read an element of what has none.
 *
 * <p>The driver's test of it (`native/crates/compiler/tests/never_applied.rs`) runs the document
 * this writes, so what is held here is that the document it runs is what this half says.
 */
class AFoldOfAnEmptyListIsHandedAFunctionNoCallReachesTest {

    private static final String MODULE = """
            module folding exposing ( kept )

            behavior kept : (a: Int) -> Int
            let kept (a) = List.fold((acc, x) -> acc, a, [])
            """;

    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "folding_empty.transport.json");

    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(Checked.of(List.of(MODULE))));
    }

    /** The function crosses as the block it is, taking what has no value. */
    @Test
    void theFunctionCrossesAsABlockOverWhatHasNoValue() {
        assertThat(ProgramWriter.written(Checked.of(List.of(MODULE))))
                .contains("{\"fn\":{\"takes\":[{\"prim\":\"INT\"},{\"nothing\":{}}]");
    }
}
