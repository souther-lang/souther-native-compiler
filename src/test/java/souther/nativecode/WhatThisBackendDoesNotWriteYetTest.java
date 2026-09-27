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

    private static final String OVER_A_SET = """
            module calculation

            behavior widen : (a: Set<Int>) -> Set<Int>

            let widen (a) = a
            """;

    /**
     * An arm binding a name where it tests that an optional holds nothing, which the language admits
     * with no type for the name (souther-lang/souther#1984): what this backend is behind on today.
     */
    private static final String BINDING_NOTHING = """
            module absent exposing ( counted, Held )

            data Held = { o: Int? }

            behavior counted : (h: Held) -> Int
            let counted (h) = match h.o with
                | Some x -> x
                | None as n -> 0
            """;

    /**
     * The type crosses. What a primitive is called is the language's and whether there is a
     * representation for it is the driver's, so a writer holding its own list of what the driver
     * supports would be a second copy of an answer that lives over there.
     *
     * <p>The operator half of this is checked where a document can be written by hand
     * (`native/crates/compiler/tests/refusals.rs`): every operator a program can currently get
     * past this writer has a lowering, so there is no program to write here that would show it.
     * That every member of either vocabulary is spelt the same way on both sides is a different
     * question, and {@link souther.nativecode.transport.ProgramWriter#vocabularies} is what the two
     * halves meet at for it.
     */
    @Test
    void aSetCrossesWhole() {
        String written = ProgramWriter.written(Checked.of(List.of(OVER_A_SET)));

        assertThat(written).contains("\"set\":");
    }

    /**
     * A row's entry calls what computes each of its inputs, so a row stating a temporal is a row
     * the object runs: its input is made from the text the checker read the literal as.
     */
    @Test
    void aRowStatingADateIsWrittenWithTheDayItStates() {
        CheckedProgram program = Checked.of(List.of("""
                module owing

                data Due = { on: Date, label: String }

                behavior labelled : (due: Due) -> String
                let labelled (due) = due.label

                example labelled
                    | "a date" : (Due { on = Date("2026-07-25"), label = "rent" }) -> "rent"
                """));

        assertThat(ProgramWriter.written(program))
                .contains("\"core\":\"temporal\",\"count\":"
                        + java.time.LocalDate.parse("2026-07-25").toEpochDay() + ",\"nano\":0");
    }

    /**
     * An arm binding a name where it tests that an optional holds nothing is admitted with no type
     * for the name (souther-lang/souther#1984), and there is nothing to write it as. Refused as not
     * lowered, naming that, rather than written with a type this side made up.
     */
    @Test
    void anArmBindingANameToNothingIsNotLoweredYet() {
        CheckedProgram program = Checked.of(List.of(BINDING_NOTHING));

        assertThatThrownBy(() -> ProgramWriter.written(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("souther-lang/souther#1984");
    }

    @Test
    void theDriverSaysItIsOneThisBackendHasNotGotRoundTo() {
        assertThatThrownBy(() -> NativeCompiler.compile(
                Checked.of(List.of(BINDING_NOTHING))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("souther-lang/souther#1984");
    }

    @Test
    void theCommandLineSaysTheBackendIsBehindAndNotThatTheCommandWasWrong() throws Exception {
        Path source = Files.createTempDirectory("souther-native-test").resolve("absent.sou");
        Files.writeString(source, BINDING_NOTHING, StandardCharsets.UTF_8);
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
