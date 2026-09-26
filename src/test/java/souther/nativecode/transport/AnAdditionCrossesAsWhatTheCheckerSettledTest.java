package souther.nativecode.transport;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NotLowered;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.ZoneOffset;
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
     * A temporal literal crosses as the count the checker's own parse read it as, and not as the
     * text it was written as: {@code java.time} admits spellings it does not write back
     * ({@code DateTime("2026-04-01t09:30")}, {@code Time("09:30:00.")}, {@code Date("+010000-01-01")}),
     * and text handed over would be read again on the other side by a grammar of its own, whose
     * refusals would be programs the checker passed. Two spellings of one value cross as one
     * document.
     */
    @Test
    void aTemporalLiteralCrossesAsTheCountTheCheckerReadItAs() {
        CheckedProgram program = Checked.of(List.of("""
                module calculation

                behavior opening : (a: Int) -> Date

                let opening (a) = Date("2026-04-01")

                behavior closing : (a: Int) -> Instant

                let closing (a) = Instant("2026-04-01T09:30:00.5Z")

                behavior clock : (a: Int) -> Time

                let clock (a) = Time("09:30:15")

                behavior meeting : (a: Int) -> DateTime

                let meeting (a) = DateTime("2026-04-01T09:30")
                """));

        String written = ProgramWriter.written(program);

        long day = LocalDate.parse("2026-04-01").toEpochDay();
        long second = LocalDateTime.parse("2026-04-01T09:30").toEpochSecond(ZoneOffset.UTC);
        assertThat(written).contains("{\"core\":\"temporal\",\"count\":" + day
                + ",\"nano\":0,\"type\":{\"prim\":\"DATE\"},");
        assertThat(written).contains("{\"core\":\"temporal\",\"count\":"
                + Instant.parse("2026-04-01T09:30:00.5Z").getEpochSecond()
                + ",\"nano\":500000000,\"type\":{\"prim\":\"INSTANT\"},");
        assertThat(written).contains("{\"core\":\"temporal\",\"count\":" + (9 * 3600 + 30 * 60 + 15)
                + ",\"nano\":0,\"type\":{\"prim\":\"TIME\"},");
        assertThat(written).contains("{\"core\":\"temporal\",\"count\":" + second
                + ",\"nano\":0,\"type\":{\"prim\":\"DATETIME\"},");
    }

    /** What the checker admits of a spelling is the checker's, so a spelling and the one it names
     * are one literal by the time they cross. */
    @Test
    void twoSpellingsOfOneTemporalCrossAsOne() {
        String canonical = literalsOver("""
                let a (n) = Date("+10000-01-01")
                let b (n) = Time("09:30")
                let c (n) = DateTime("2026-07-01T09:30")
                let d (n) = Instant("2026-07-01T00:00:00Z")
                """);
        String spelt = literalsOver("""
                let a (n) = Date("+010000-01-01")
                let b (n) = Time("09:30:00.")
                let c (n) = DateTime("2026-07-01t09:30")
                let d (n) = Instant("2026-07-01T00:00:00.Z")
                """);

        assertThat(spelt).isEqualTo(canonical);
    }

    private static String literalsOver(String definitions) {
        return ProgramWriter.written(Checked.of(List.of("""
                module spelling

                behavior a : (n: Int) -> Date
                behavior b : (n: Int) -> Time
                behavior c : (n: Int) -> DateTime
                behavior d : (n: Int) -> Instant

                """ + definitions)));
    }
}
