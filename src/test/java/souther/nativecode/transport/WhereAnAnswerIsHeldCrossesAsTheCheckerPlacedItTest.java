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
 * What a behavior declares of its answer crosses on the behavior's target, saying where the checker
 * placed the check and carrying the rules as the checker elaborated them to run.
 *
 * <p>Where it is held is the checker's answer and each of its answers crosses as itself: at the
 * callee for a behavior whose body is here, at each crossing for one whose answer arrives from
 * outside, and nowhere for one that declares nothing. A rule crosses with what it is about, what
 * the answer is read as there, and what has to hold, over the parameters' bindings and the
 * answer's; what the checker decided about the declaration beside that crosses too, whether or not
 * anything reads it yet.
 */
class WhereAnAnswerIsHeldCrossesAsTheCheckerPlacedItTest {

    private static final String DECLARING = """
            module m exposing ( find, twice, looked : Int )

            data Found = { id: Int }
            data Missing

            behavior find : (id: Int) -> Found | Missing
                ensures sameId = Found -> value.id == id
                ensures Missing -> id <= 0

            let find (id) = if id > 0 then Found { id = id } else Missing

            behavior lookUp : (a: Int) -> Int
                ensures notBelow = value >= a

            behavior twice : (a: Int) -> Int
                depends on lookUp
            let twice (a, lookUp) = lookUp(a) * 2

            behavior doubled : (a: Int) -> Int
            let doubled (a) = a * 2

            behavior looked = lookUp >-> doubled

            example find
                | "found under itself" : (3) -> Found { id = 3 }
            """;

    /**
     * The document the Rust driver is tested against: rules held at the callee, reached from a row,
     * and rules held where an answer crosses in, from a body and from a stage.
     */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "ensures.transport.json");

    private static String written() {
        return ProgramWriter.written(Checked.of(List.of(DECLARING)));
    }

    /**
     * A rule over a case says which case it tests for and what the answer is read as there, and
     * every rule is written, in the order the checker keeps them: the second applies to an answer
     * the first does not, and neither is chosen over the other.
     */
    @Test
    void aBodysRulesCrossAsHeldAtTheCallee() {
        assertThat(written()).contains("""
                "ensures":{"at":"callee","contract":{"parameters":["id"],"rules":[\
                {"guard":{"is":"case","selects":{"tests":"which","atoms":[\
                {"is":"declared","declared":"m.Found"}]},"binds":{"declared":"m.Found"}},\
                "value":1,"condition":{"core":"binary","op":"EQ","reading":{"is":"astheystand"},\
                "left":{"core":"field","target":{"core":"read","binding":1,\
                "type":{"declared":"m.Found"},"aborts":[]},"field":"id","type":{"prim":"INT"},\
                "aborts":[]},"right":{"core":"read","binding":0,"type":{"prim":"INT"},"aborts":[]},\
                "type":{"prim":"BOOL"},"aborts":[]},"readsanswer":true,"clause":"sameId"},\
                {"guard":{"is":"case","selects":{"tests":"which","atoms":[\
                {"is":"declared","declared":"m.Missing"}]},"binds":{"declared":"m.Missing"}},\
                "value":2,""");
        // A rule that names no value reads no answer, which is the checker's to say and is said.
        assertThat(written()).contains("\"readsanswer\":false,\"clause\":null}]}}");
    }

    /** A rule over every answer tests for nothing and reads the answer as what the behavior answers. */
    @Test
    void anAnswerFromOutsideCrossesAsHeldWhereItCrossesIn() {
        assertThat(written()).contains("""
                "name":"lookUp","is":"injected",\
                "parameters":{"named":[{"name":"a","input":{"is":"scalar","scalar":"INT"}}]},\
                "output":{"is":"scalar","scalar":"INT"},\
                "ensures":{"at":"crossing","contract":{"parameters":["a"],"rules":[\
                {"guard":{"is":"always"},"value":1,""");
    }

    /** What the driver compiles is what this writer wrote, and not a second thing like it. */
    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip()).isEqualTo(written());
    }

    /** A behavior read and found to declare nothing says so, which is not saying nothing. */
    @Test
    void aBehaviorThatDeclaresNothingCrossesAsHeldNowhere() {
        assertThat(written()).contains("""
                "name":"twice","is":"body",\
                "parameters":{"named":[{"name":"a","input":{"is":"scalar","scalar":"INT"}}]},\
                "output":{"is":"scalar","scalar":"INT"},"ensures":{"at":"none"},\
                "requirements":[{"module":"m","name":"lookUp"}]}""");
    }
}
