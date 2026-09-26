package souther.nativecode.transport;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatCode;

/**
 * A helper crosses under the reference a call in its module reaches it by, and the type variables
 * its body leaves open cross as numbers of that helper's own.
 *
 * <p>The standard library declares {@code foldFrom} and a module reaches it as
 * {@code List.foldFrom}. What a module holds and what a call reaches are both the second, so a
 * reader holding a call finds the helper by what the call says, and never by a name made up out of
 * where the helper was declared.
 */
class AHelperCrossesUnderTheReferenceItIsReachedByTest {

    private static final String FOLDING = """
            module folding exposing ( summed, again, spelt )

            behavior summed : (a: Int) -> Int
            let summed (a) = List.fold((acc, x) -> acc + x, 0, [a, 2])

            behavior again : (a: Int) -> Int
            let again (a) = List.fold((acc, x) -> acc + x, a, [1])

            behavior spelt : (a: Int) -> Int
            let spelt (a) = String.length(List.fold((acc, x) -> acc ++ "ab", "", [a]))
            """;

    /**
     * The document the driver's specialization is tested against is this one, for the reason every
     * fixture of the driver's is one the writer wrote.
     */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "folding.transport.json");

    private static String written() {
        return ProgramWriter.written(Checked.of(List.of(FOLDING)));
    }

    private static final String FOLD_FROM =
            "{\"is\":\"library\",\"alias\":\"List\",\"name\":\"foldFrom\"}";

    /**
     * The helper and the call carry one reference, the route and what it reaches, and not a
     * spelling of it: the library's operation under the alias it publishes it as.
     */
    @Test
    void aHelperTheLibraryDeclaresCrossesAsTheReferenceItIsReachedBy() {
        assertThat(written())
                .contains("\"helpers\":[{\"reached\":" + FOLD_FROM + ",")
                .contains("\"reaches\":{\"is\":\"helper\",\"reached\":" + FOLD_FROM + "}")
                .doesNotContain("souther.list");
    }

    /**
     * A module reaches a helper it declares as its own and one another module declares under that
     * module's name, and holds a copy of each under the reference it reaches it by.
     */
    @Test
    void aHelperOfAnotherModuleCrossesUnderThatModulesName() {
        CheckedProgram program = Checked.of(List.of("""
                module lib.walk exposing ( count )

                partial let count (n: Int, acc: Int): Int = if n == 0 then acc else count(n - 1, acc + 1)
                """, """
                module app.use exposing ( counted )
                import lib.walk ( count )

                behavior counted : (n: Int) -> Int
                let counted (n) = count(n, 0)
                """));

        assertThat(ProgramWriter.written(program))
                .contains("{\"reached\":{\"is\":\"ofmodule\",\"module\":\"lib.walk\","
                        + "\"name\":\"count\"},");
        // And the driver takes the route as the checker's from the module holding the copy.
        assertThatCode(() -> NativeCompiler.compile(program)).doesNotThrowAnyException();
    }

    /**
     * {@code 'acc} and {@code 'a} are numbered where the helper first names them: its step takes an
     * {@code 'acc} and then an {@code 'a}.
     */
    @Test
    void aTypeVariableCrossesAsANumberOfItsHelper() {
        assertThat(written()).contains("""
                "parameters":[{"name":"step","type":{"fn":{"takes":[{"var":0},{"var":1}],\
                "answers":{"var":0}}}},{"name":"seed","type":{"var":0}},\
                {"name":"xs","type":{"list":{"var":1}}},{"name":"i","type":{"prim":"INT"}}]""");
    }

    /** A call of it carries the types it was settled at, which is what binds each variable. */
    @Test
    void aCallOfItCarriesTheTypesItWasSettledAt() {
        assertThat(written()).contains("""
                "reaches":{"is":"helper","reached":{"is":"library","alias":"List","name":"foldFrom"}},"arguments":[{"core":"block",""")
                .contains("""
                "type":{"fn":{"takes":[{"prim":"STRING"},{"prim":"INT"}],"answers":{"prim":"STRING"}}}""");
    }

    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip()).isEqualTo(written());
    }
}
