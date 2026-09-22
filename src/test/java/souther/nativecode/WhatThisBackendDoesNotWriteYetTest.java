package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.io.ByteArrayOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A program the language admits that this backend does not write yet, followed all the way out.
 *
 * <p>Three answers are possible at the end of this path and only one of them is right: the program
 * is refused by the language, this backend has not got round to it, or the command was wrong.
 * Which one a reader is told decides whether they go and change their program. So the test is the
 * whole way through rather than at any one of the places the answer could be lost.
 */
class WhatThisBackendDoesNotWriteYetTest {

    private static final String OVER_A_STRING = """
            module calculation

            behavior widen : (a: String) -> String

            let widen (a) = a
            """;

    /**
     * The type crosses. What a primitive is called is the language's and whether there is a
     * representation for it is the driver's, so a writer holding its own list of what the driver
     * supports would be a second copy of an answer that lives over there.
     *
     * <p>The operator half of this is checked where a document can be written by hand
     * (`native/crates/compiler/tests/refusals.rs`): every operator a program can currently get
     * past this writer has a lowering, so there is no program to write here that would show it.
     */
    @Test
    void aTypeWithNoRepresentationStillCrosses() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(OVER_A_STRING)));

        assertThat(written).contains("\"prim\":\"STRING\"");
    }

    /**
     * A construction runs the type's clauses and stops at the first that does not hold. Nothing
     * here runs one, and building the value anyway would make the type's invariant true of what
     * this emits by leaving it out.
     */
    @Test
    void aTypeThatSaysWhatItsValuesOweIsOneNothingIsBuiltOfYet() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module owing

                data Amount = { value: Int }
                    invariant value >= 0

                behavior of : (a: Int) -> Int
                let of (a) = {
                    let held = Amount { value = a }
                    held.value
                }
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("owing.Amount");
    }

    /**
     * A row states its values and the object carries an entry that runs them, so a value with no
     * expression to make it is a row the object cannot run.
     *
     * <p>Refused rather than left out. An object missing an entry would still link and still
     * answer every row it did carry, so what a check of the rows compared would shrink by however
     * many rows had values like this one — and it would go on being green over the ones that were
     * left.
     */
    @Test
    void aRowStatingAValueWithNoExpressionToMakeItIsRefusedRatherThanLeftOut() {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module owing

                data Amount = { value: Int }

                behavior tally : (a: Amount) -> Int
                let tally (a) = a.value

                example tally
                    | "a value of it" : (Amount { value = 7 }) -> 7
                """));

        assertThatThrownBy(() -> ProgramWriter.written(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("a row stating")
                .hasMessageContaining("owing.Amount");
    }

    @Test
    void theDriverSaysItIsOneThisBackendHasNotGotRoundTo() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of(OVER_A_STRING))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("String");
    }

    @Test
    void theCommandLineSaysTheBackendIsBehindAndNotThatTheCommandWasWrong() throws Exception {
        Path source = Files.createTempDirectory("souther-native-test").resolve("calculation.sou");
        Files.writeString(source, OVER_A_STRING, StandardCharsets.UTF_8);
        ByteArrayOutputStream problems = new ByteArrayOutputStream();

        int ended = Main.run(
                new String[]{"-o", source.resolveSibling("out.o").toString(), source.toString()},
                new PrintStream(OutputStream.nullOutputStream(), true, StandardCharsets.UTF_8),
                new PrintStream(problems, true, StandardCharsets.UTF_8));

        assertThat(ended).isEqualTo(1);
        assertThat(problems.toString(StandardCharsets.UTF_8))
                .contains("this backend does not write that yet");
    }
}
