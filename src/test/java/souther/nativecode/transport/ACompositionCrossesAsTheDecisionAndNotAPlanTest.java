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
 * A behavior written as {@code >->} crosses as its stages and its routing, not as the word
 * {@code "composed"} and nothing else (Issue #13).
 *
 * <p>The document here is what the fixture the Rust driver runs is written from — the same
 * relationship the addition fixture has to {@link AnAdditionCrossesAsWhatTheCheckerSettledTest}.
 * That fixture is what proves the routing crosses whole: a stage that left the main line at one
 * point in the pipeline must not be tested against a later stage's cases, which is exactly the
 * ambiguity {@code composing.rs} runs to observe rather than only asserts of this JSON.
 */
class ACompositionCrossesAsTheDecisionAndNotAPlanTest {

    private static final String ROUTED = """
            module routing exposing ( pipeline : B | C )

            data A = { n: Int }
            data B = { n: Int }
            data C = { n: Int }

            behavior f : (n: Int) -> A | B
                constructs A, B

            let f (n) = {
                guard n <= 100 else B { n = n }
                A { n = n }
            }

            behavior g : (a: A) -> B
                constructs B

            let g (a) = B { n = a.n * 2 }

            behavior h : (b: B) -> C
                constructs C

            let h (b) = C { n = b.n + 1 }

            behavior pipeline = f >-> g >-> h
            """;

    /**
     * The document the Rust driver is tested against. Written to a file rather than described
     * there again, for the reason the addition fixture is.
     */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "composing.transport.json");

    @Test
    void aStageIsCalledAndItsRoutingTestsWhatTheCheckerResolved() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(ROUTED)));

        assertThat(written).contains("\"is\":\"composed\",\"declared\":\"routing.pipeline\"");
        assertThat(written).contains("\"stages\":[");
        // The first stage takes the composition's own arguments, so nothing is routed into it.
        assertThat(written).contains(
                "{\"behavior\":\"routing.f\",\"answers\":{\"union\":[{\"is\":\"declared\",\"declared\":\"routing.A\"},"
                        + "{\"is\":\"declared\",\"declared\":\"routing.B\"}]},"
                        + "\"routing\":{\"is\":\"always\"}}");
        // A stage after the first is offered only the cases it accepts, and which cases those are
        // is the checker's answer, read off the stage rather than worked out again here.
        assertThat(written).contains(
                "{\"behavior\":\"routing.g\",\"answers\":{\"declared\":\"routing.B\"},"
                        + "\"routing\":{\"is\":\"oncases\",\"accepted\":[{\"is\":\"declared\",\"declared\":\"routing.A\"}]}}");
        assertThat(written).contains(
                "{\"behavior\":\"routing.h\",\"answers\":{\"declared\":\"routing.C\"},"
                        + "\"routing\":{\"is\":\"oncases\",\"accepted\":[{\"is\":\"declared\",\"declared\":\"routing.B\"}]}}");
        // What a stage's behavior is closes the document, the same as a body's call does: `g` and
        // `h` are named by no body a `Core.Call` ever reaches, only by a stage.
        assertThat(written).contains("\"module\":\"routing\",\"name\":\"g\",\"is\":\"body\"");
        assertThat(written).contains("\"module\":\"routing\",\"name\":\"h\",\"is\":\"body\"");
    }

    /** What the driver compiles is what this writer wrote, and not a second thing like it. */
    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(CheckedProgram.of(List.of(ROUTED))));
    }
}
