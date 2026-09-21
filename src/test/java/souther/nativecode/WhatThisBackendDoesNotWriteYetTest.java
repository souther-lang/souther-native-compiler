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

    private static final String SUBTRACTING = """
            module calculation

            behavior less : (a: Int, b: Int) -> Int

            let less (a, b) = a - b
            """;

    private static final String OVER_A_STRING = """
            module calculation

            behavior widen : (a: String) -> String

            let widen (a) = a
            """;

    /**
     * The operator crosses. What it means is the language's and whether it can be written is the
     * driver's, so a writer holding its own list of what the driver supports would be a second
     * copy of an answer that lives over there.
     */
    @Test
    void anOperatorWithNoLoweringStillCrosses() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(SUBTRACTING)));

        assertThat(written).contains("\"op\":\"SUB\"");
    }

    @Test
    void theDriverSaysItIsOneThisBackendHasNotGotRoundTo() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of(SUBTRACTING))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("-");
    }

    /** The same for a type with no representation yet, which crosses for the same reason. */
    @Test
    void aTypeWithNoRepresentationIsOneThisBackendHasNotGotRoundToEither() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of(OVER_A_STRING))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("String");
    }

    @Test
    void theCommandLineSaysTheBackendIsBehindAndNotThatTheCommandWasWrong() throws Exception {
        Path source = Files.createTempDirectory("souther-native-test").resolve("calculation.sou");
        Files.writeString(source, SUBTRACTING, StandardCharsets.UTF_8);
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
