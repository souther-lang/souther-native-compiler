package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NotLowered;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

class AnAdditionCrossesAsWhatTheCheckerSettledTest {

    private static final String ADDING = """
            module calculation

            behavior add : (a: Int, b: Int) -> Int

            let add (a, b) = a + b
            """;

    /**
     * The document the driver is tested against is this one. Written to a file the Rust half reads
     * as a fixture rather than described there again, so the two halves meet at something one of
     * them produced instead of at two readings of the same prose.
     */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "adding.transport.json");

    @Test
    void anAdditionOfTwoParametersCrossesAsABinaryOverTwoReads() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(ADDING)));

        assertThat(written).isEqualTo("""
                {"transport":1,"modules":[{"name":"calculation","behaviors":[\
                {"name":"add","parameters":["a","b"],\
                "takes":[{"prim":"INT"},{"prim":"INT"}],"answers":{"prim":"INT"},\
                "body":{"core":"binary","op":"ADD",\
                "left":{"core":"read","binding":0,"type":{"prim":"INT"}},\
                "right":{"core":"read","binding":1,"type":{"prim":"INT"}},\
                "type":{"prim":"INT"}}}]}]}""");
    }

    /** What the driver compiles is what this writer wrote, and not a second thing like it. */
    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(CheckedProgram.of(List.of(ADDING))));
    }

    /**
     * A body this backend does not write yet says so. It is not a refusal of the program: the
     * language admits this one and will compile it on another backend today.
     */
    @Test
    void aBodyThisBackendDoesNotWriteYetSaysWhichItWas() {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module calculation

                behavior pick : (a: Int) -> Int

                let pick (a) = if a > 0 then a else 0
                """));

        assertThatThrownBy(() -> ProgramWriter.written(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("a condition");
    }
}
