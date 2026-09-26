package souther.nativecode.transport;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value that names another value at its root crosses with the handover that names it — the case
 * every other transport test so far left at {@code "values":[]} (review of #20).
 *
 * <p>{@code ks} is kept and takes nothing; {@code ys} is published, names {@code ks} and so takes
 * one handover carrying it. The published entry's own body is written the same way any other
 * reference to a value is: two nested calls, each an ordinary {@code Reaches::Value} reach, the
 * second handed the first's answer as an argument — not a special "call a value" shape of its own.
 * That is what proves handovers do not need a machinery beside {@code Node::Call}'s own {@code
 * arguments}: a call to a local value already carries what it needs to run there.
 */
class AValueThatHandsOverAnotherValueCrossesTest {

    private static final String MODULE = """
            module m exposing ( P, ys )

            data P = { n: Int }

            let ks = P { n = 42 }

            let ys = ks
            """;

    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "values.transport.json");

    @Test
    void aValueNamingAnotherCrossesWithTheHandoverThatCarriesIt() {
        String written = ProgramWriter.written(Checked.of(List.of(MODULE)));

        assertThat(written).isEqualTo("""
                {"transport":23,"declarations":[{"module":"m","name":"P","by":"amodule","is":"product",\
                "fields":[{"name":"n","binding":0,"codec":{"is":"scalar","scalar":"INT"}}],"invariants":[]}],"behaviors":[],\
                "modules":[{"name":"m","publishes":["m.P"],"helpers":[],\
                "values":[{"module":"m","name":"ks","handovers":[],\
                "body":{"core":"construct","declared":"m.P",\
                "values":[{"core":"int","value":42,"type":{"prim":"INT"},"aborts":[]}],\
                "type":{"ref":{"is":"declared","declared":"m.P"}},"aborts":[]}},\
                {"module":"m","name":"ys",\
                "handovers":[{"parameter":"$dep_ks","type":{"ref":{"is":"declared","declared":"m.P"}},\
                "carries":{"module":"m","name":"ks"}}],\
                "body":{"core":"read","binding":0,"type":{"ref":{"is":"declared","declared":"m.P"}},"aborts":[]}}],\
                "entries":[{"value":{"module":"m","name":"ys"},\
                "body":{"core":"let","binding":0,"binds":{"ref":{"is":"declared","declared":"m.P"}},\
                "value":{"core":"call","reaches":{"is":"value","module":"m","name":"ks"},\
                "arguments":[],"type":{"ref":{"is":"declared","declared":"m.P"}},"aborts":[]},\
                "body":{"core":"let","binding":1,"binds":{"ref":{"is":"declared","declared":"m.P"}},\
                "value":{"core":"call","reaches":{"is":"value","module":"m","name":"ys"},\
                "arguments":[{"core":"read","binding":0,"type":{"ref":{"is":"declared","declared":"m.P"}},"aborts":[]}],\
                "type":{"ref":{"is":"declared","declared":"m.P"}},"aborts":[]},\
                "body":{"core":"read","binding":1,"type":{"ref":{"is":"declared","declared":"m.P"}},"aborts":[]},\
                "type":{"ref":{"is":"declared","declared":"m.P"}},"aborts":[]},\
                "type":{"ref":{"is":"declared","declared":"m.P"}},"aborts":[]}}],\
                "definitions":[],"examples":[]}]}""");
    }

    /** What the driver is tested against is this, written to a file rather than described there
     *  again — the same split every other transport fixture keeps. */
    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(Checked.of(List.of(MODULE))));
    }
}
