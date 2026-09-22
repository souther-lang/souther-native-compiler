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
            module calculation exposing ( add )

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
                {"transport":3,"declarations":[],\
                "behaviors":[{"module":"calculation","name":"add","is":"body",\
                "takes":[{"prim":"INT"},{"prim":"INT"}],"answers":{"prim":"INT"}}],\
                "modules":[{"name":"calculation","helpers":[],\
                "bodies":[{"declared":"calculation.add","parameters":["a","b"],\
                "publication":"published",\
                "body":{"core":"binary","op":"ADD",\
                "left":{"core":"read","binding":0,"type":{"prim":"INT"}},\
                "right":{"core":"read","binding":1,"type":{"prim":"INT"}},\
                "type":{"prim":"INT"}}}],"examples":[]}]}""");
    }

    /** What the driver compiles is what this writer wrote, and not a second thing like it. */
    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(CheckedProgram.of(List.of(ADDING))));
    }

    /**
     * A type nothing in a body names still has to be in the document.
     *
     * <p>It is named by a signature and by nothing else, so it is found while the behaviors are
     * being written — after the declarations would be finished, if the two were finished one at a
     * time. The document would then name a type it says nothing about.
     */
    @Test
    void aTypeOnlyASignatureNamesIsStillDeclared() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of("""
                module demo

                data Inner = Int
                data Token = { held: Inner }

                behavior ignore : (token: Token) -> Int
                let ignore (token) = 42
                """)));

        assertThat(written).contains("\"takes\":[{\"declared\":\"demo.Token\"}]");
        assertThat(written).contains(
                "{\"module\":\"demo\",\"name\":\"Token\",\"by\":\"amodule\",\"is\":\"product\"");
        // And what that one holds, which nothing but its declaration names: found while the
        // declarations were being written, after the behaviors were finished.
        assertThat(written).contains(
                "{\"module\":\"demo\",\"name\":\"Inner\",\"by\":\"amodule\",\"is\":\"newtype\"");
    }

    /**
     * A body this backend does not write yet says so. It is not a refusal of the program: the
     * language admits this one and will compile it on another backend today.
     */
    @Test
    void aBodyThisBackendDoesNotWriteYetSaysWhichItWas() {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module calculation

                behavior rate : (a: Int) -> Decimal

                let rate (a) = 1.5m
                """));

        assertThatThrownBy(() -> ProgramWriter.written(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("a decimal literal");
    }
}
