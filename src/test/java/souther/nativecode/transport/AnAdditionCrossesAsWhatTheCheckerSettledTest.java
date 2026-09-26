package souther.nativecode.transport;

import souther.nativecode.Checked;
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
        String written = ProgramWriter.written(Checked.of(List.of(ADDING)));

        assertThat(written).isEqualTo("""
                {"transport":26,"declarations":[],\
                "behaviors":[{"module":"calculation","name":"add","is":"body",\
                "parameters":{"named":[{"name":"a","input":{"is":"scalar","scalar":"INT"}},\
                {"name":"b","input":{"is":"scalar","scalar":"INT"}}]},\
                "output":{"is":"scalar","scalar":"INT"},"ensures":{"at":"none"},"requirements":[]}],\
                "modules":[{"name":"calculation","publishes":[],"helpers":[],"values":[],"entries":[],\
                "definitions":[{"is":"body","declared":"calculation.add","parameters":["a","b"],\
                "publication":"published",\
                "body":{"core":"binary","op":"ADD","reading":{"is":"astheystand"},\
                "left":{"core":"read","binding":0,"type":{"prim":"INT"},"aborts":[]},\
                "right":{"core":"read","binding":1,"type":{"prim":"INT"},"aborts":[]},\
                "type":{"prim":"INT"},"aborts":["REQUIRED_FORM_HAS_NO_PLACE"]}}],"examples":[]}]}""");
    }

    /**
     * A parameter crosses under the name the signature gives it, and not under the one the
     * {@code let} binds it to: the two correspond by place and may differ, and the signature's is
     * the one a host is told. The body still says its own.
     */
    @Test
    void aParameterCrossesUnderTheNameItsSignatureGivesIt() {
        String written = ProgramWriter.written(Checked.of(List.of("""
                module calculation exposing ( add )

                behavior add : (augend: Int, addend: Int) -> Int

                let add (a, b) = a + b
                """)));

        assertThat(written).contains("""
                "parameters":{"named":[{"name":"augend","input":{"is":"scalar","scalar":"INT"}},\
                {"name":"addend","input":{"is":"scalar","scalar":"INT"}}]}""");
        assertThat(written).contains("\"parameters\":[\"a\",\"b\"]");
    }

    /** A composition declares no parameters, and takes what it takes in order and unnamed. */
    @Test
    void aCompositionCrossesWithItsInputsInOrderAndUnnamed() {
        String written = ProgramWriter.written(Checked.of(List.of("""
                module calculation exposing ( add, doubledSum : Int )

                behavior add : (a: Int, b: Int) -> Int
                let add (a, b) = a + b

                behavior double : (n: Int) -> Int
                let double (n) = n * 2

                behavior doubledSum = add >-> double
                """)));

        assertThat(written).contains("""
                "name":"doubledSum","is":"composed",\
                "parameters":{"positional":[{"is":"scalar","scalar":"INT"},{"is":"scalar","scalar":"INT"}]}""");
    }

    /** What the driver compiles is what this writer wrote, and not a second thing like it. */
    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(Checked.of(List.of(ADDING))));
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
        String written = ProgramWriter.written(Checked.of(List.of("""
                module demo

                data Inner = Int
                data Token = { held: Inner }

                behavior ignore : (token: Token) -> Int
                let ignore (token) = 42
                """)));

        assertThat(written).contains("{\"name\":\"token\",\"input\":{\"is\":\"nominal\",\"named\":{\"is\":\"declared\",\"declared\":\"demo.Token\"}}}");
        assertThat(written).contains(
                "{\"module\":\"demo\",\"name\":\"Token\",\"by\":\"amodule\",\"is\":\"product\"");
        // And what that one holds, which nothing but its declaration names: found while the
        // declarations were being written, after the behaviors were finished.
        assertThat(written).contains(
                "{\"module\":\"demo\",\"name\":\"Inner\",\"by\":\"amodule\",\"is\":\"newtype\"");
    }

    /**
     * A temporal literal crosses as the ISO text the checker read it as, under the type it is:
     * {@code kind} and {@code type} of a {@code Core.Temporal} are one value, so it is written once.
     */
    @Test
    void aTemporalLiteralCrossesAsTheTextTheCheckerReadItAs() {
        CheckedProgram program = Checked.of(List.of("""
                module calculation

                behavior opening : (a: Int) -> Date

                let opening (a) = Date("2026-04-01")

                behavior closing : (a: Int) -> Instant

                let closing (a) = Instant("2026-04-01T09:30:00.5Z")
                """));

        String written = ProgramWriter.written(program);

        assertThat(written).contains(
                "{\"core\":\"temporal\",\"text\":\"2026-04-01\",\"type\":{\"prim\":\"DATE\"},");
        assertThat(written).contains(
                "{\"core\":\"temporal\",\"text\":\"2026-04-01T09:30:00.5Z\",\"type\":{\"prim\":\"INSTANT\"},");
    }
}
