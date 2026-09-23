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
 * A function value — {@code Type.FnOf}, {@code Core.Block}, {@code Core.Apply} — crosses for
 * Issue #11 (souther-lang/souther-native-compiler#11).
 *
 * <p>None of the five behaviors below binds a lambda and calls it straight away: the checker
 * inlines that case away before {@link ProgramWriter} ever sees a {@link
 * souther.compiler.core.Core.Block} at all — {@code let f = (x) -> x + 1 in f(10)} crosses as the
 * arithmetic alone, with no block and no apply anywhere on the wire. What forces a function value
 * to stay a value, rather than being resolved back into the code that built it, is not knowing
 * which of two bodies is meant until the run decides — an {@code if} choosing between two lambdas,
 * which is the one shape every behavior here is built from.
 *
 * <p>What crosses for one of these is a {@code Core.Block} written whole — its own parameters and
 * its body, {@code site} the only thing minted for it — and a {@code Core.Apply} naming a {@link
 * souther.compiler.core.Core.Read} the same way any other operand is named. Neither carries a
 * capture list: what a block reaches outside its own boundary is this backend's own question,
 * answered in {@code souther_native_driver::closures}, not the checker's to state and not this
 * writer's to project.
 *
 * <p>Every behavior but the last builds and applies its own closure in the one generated function
 * its own body lowers to. {@code handed_over} is the exception: a function type cannot cross a
 * behavior's own declared boundary (E1311), so the way this language actually hands one to a
 * different generated function is a helper's own parameter — a helper survives as a method of its
 * own, rather than being inlined at its call site, exactly where it recurses (see
 * {@code CheckedHelper}'s own javadoc), so {@code apply_n} is written {@code partial} and calls
 * itself. That makes {@code f} cross as an ordinary machine parameter of {@code apply_n}'s own
 * generated function, applied inside a body that never built it.
 */
class AFunctionValueCrossesWholeWithNoCaptureListTest {

    private static final String MODULE = """
            module closures exposing (
                no_capture, with_struct, aborting, adder, nested, handed_over, Box
            )

            data Box = { n: Int }

            behavior no_capture : (c: Bool, x: Int) -> Int
            behavior with_struct : (c: Bool, box: Box, x: Int) -> Int
            behavior aborting : (c: Bool, x: Int) -> Int
            behavior adder : (a: Int, c: Bool, x: Int) -> Int
            behavior nested : (a: Int, c1: Bool, c2: Bool) -> Int
            behavior handed_over : (c: Bool, x: Int) -> Int

            let no_capture (c, x) = {
                let f = if c then (y) -> y + 1 else (y) -> y - 1
                f(x)
            }

            let with_struct (c, box, x) = {
                let f = if c then (y) -> y + box.n else (y) -> y - box.n
                f(x)
            }

            let aborting (c, x) = {
                let f = if c then (y) -> y - 1 else (y) -> y + x
                f(9223372036854775807)
            }

            let adder (a, c, x) = {
                let f = if c then (y) -> y + a else (y) -> y * a
                f(x)
            }

            let nested (a, c1, c2) = {
                let outer = if c1
                    then (x) -> {
                        let b = x + 1
                        let inner = if c2 then (y) -> y + a + b else (y) -> y * a * b
                        inner(100)
                    }
                    else (x) -> x
                outer(5)
            }

            partial let apply_n (f: (Int) -> Int, n: Int, x: Int): Int =
                if n <= 0 then x else apply_n(f, n - 1, f(x))

            let handed_over (c, x) = {
                let f: (Int) -> Int = if c then (y) -> y + 1 else (y) -> y * 2
                apply_n(f, 3, x)
            }
            """;

    /**
     * The document the driver's own closure tests are held to. Written to a file the Rust half
     * reads as a fixture rather than described there again, so the two halves meet at something
     * one of them produced instead of at two readings of the same prose (the same arrangement
     * {@code adding.transport.json} already keeps).
     */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "closures.transport.json");

    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(CheckedProgram.of(List.of(MODULE))));
    }

    @Test
    void aFunctionTypeCrossesAsTakesAndAnswersNestedUnderFn() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(MODULE)));

        assertThat(written).contains(
                "\"type\":{\"fn\":{\"takes\":[{\"prim\":\"INT\"}],\"answers\":{\"prim\":\"INT\"}}}");
    }

    @Test
    void aBlockCarriesASiteAndItsOwnParametersAndNothingAboutWhatItReaches() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(MODULE)));

        assertThat(written).contains("\"core\":\"block\",\"site\":0,\"parameters\":[{\"binding\":");
        // Never a capture list: closure conversion is this backend's own question, not written
        // here at all. (`no_capture` is one of this fixture's own behavior names, so the check is
        // for the field a leaking writer would add, not the bare word.)
        assertThat(written).doesNotContain("\"captures\"");
    }

    @Test
    void anApplyNamesItsFunctionTheSameWayAnyOtherOperandIsNamed() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(MODULE)));

        assertThat(written).contains("\"core\":\"apply\",\"function\":{\"core\":\"read\",\"binding\":");
    }

    /**
     * A lambda bound and applied where nothing else could have been meant is not a function value
     * at all by the time it reaches this writer — the checker already resolved it into the code it
     * names, and there is nothing here to write a block for.
     */
    @Test
    void aLambdaAppliedWhereNothingElseCouldBeMeantNeverBecomesABlock() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of("""
                module inlined

                behavior straight : (a: Int) -> Int

                let straight (a) = {
                    let f = (x) -> x + a
                    f(10)
                }
                """)));

        assertThat(written).doesNotContain("\"core\":\"block\"");
        assertThat(written).doesNotContain("\"core\":\"apply\"");
    }
}
