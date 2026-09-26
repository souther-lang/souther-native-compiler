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
    void aTypeWithNoRepresentationStillCrosses() {
        String written = ProgramWriter.written(Checked.of(List.of(OVER_A_SET)));

        assertThat(written).contains("\"set\":");
    }

    /**
     * A set crosses whole: the program is read, and what is refused is laying one out, which
     * nothing here does yet. How a set holds its members waits on the language saying how every
     * carrier orders and spells them.
     */
    @Test
    void anAnswerThatIsASetIsReadAndNotLaidOut() {
        assertThatThrownBy(() -> NativeCompiler.compile(Checked.of(List.of("""
                module listed exposing ( many )

                behavior many : (n: Int) -> Set<Int>
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Set");
    }

    /**
     * The checker lets a set of a case stand where a set of its sum is answered, and this backend
     * lays out no set. So the program is not lowered — and it is not the two halves disagreeing,
     * which is what it would be read as if this side answered the checker's question about a
     * collection without the checker's rules.
     */
    @Test
    void aSetAnsweredCovariantlyIsNotLoweredRatherThanADisagreement() {
        assertThatThrownBy(() -> NativeCompiler.compile(Checked.of(List.of("""
                module demo exposing ( f, Box, A, B, S )

                data A = { v: Int }
                data B = { v: Int }
                data S = A | B

                data Box = { xs: Set<A> }

                behavior f : (b: Box) -> Set<S>
                let f (b) = b.xs
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Set");
    }

    /**
     * A fold accumulating a map is rewritten by the checker's compiler into a walk that builds the
     * map, which crosses as the operations it is. No map is laid out here, so the walk is not
     * lowered, and that is what is said: not the two halves disagreeing about an operation one of
     * them could not read.
     */
    @Test
    void aWalkBuildingAMapIsReadAndNotLowered() {
        assertThatThrownBy(() -> NativeCompiler.compile(Checked.of(List.of("""
                module grouping exposing ( groups )

                behavior groups : (a: Int) -> Int
                let groups (a) = Map.size(List.groupBy((x) -> x > a, [1, 2, 3]))
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageStartingWith("the operation Map.$");
    }

    /**
     * A fold over an empty list literal that is not rewritten into a walk hands its helper a
     * function over what has no value, which it never applies. The checker's backend hands
     * {@code Fn.NEVER} in its place; a copy here would have to take a function over a type nothing
     * lays out, so it is refused as not lowered, and nothing of the function is.
     */
    @Test
    void aFunctionAHelperNeverAppliesIsNotLoweredYet() {
        assertThatThrownBy(() -> NativeCompiler.compile(Checked.of(List.of("""
                module folding exposing ( kept )

                behavior kept : (a: Int) -> Int
                let kept (a) = List.fold((acc, x) -> acc, a, [])
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("it never applies");
    }

    /**
     * A value only passing through is not written, so a behavior handing one back compiles; what
     * is refused is the boundary that would have to write a {@code Set} out, which is where its
     * external form would be decided.
     */
    @Test
    void anAnswerWithASetFieldIsRefusedWhereItWouldBeWrittenOut() {
        assertThatThrownBy(() -> NativeCompiler.compile(Checked.of(List.of("""
                module listed exposing ( same, Listed )

                data Listed = { on: Set<Int> }

                behavior same : (p: Listed) -> Listed
                let same (p) = p
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Set");
    }

    /**
     * A row's entry calls what computes each of its inputs, so a row stating a temporal is a row
     * the object runs: its input is made from the text the checker read the literal as.
     */
    @Test
    void aRowStatingADateIsWrittenWithTheTextItStatesItAs() {
        CheckedProgram program = Checked.of(List.of("""
                module owing

                data Due = { on: Date, label: String }

                behavior labelled : (due: Due) -> String
                let labelled (due) = due.label

                example labelled
                    | "a date" : (Due { on = Date("2026-07-25"), label = "rent" }) -> "rent"
                """));

        assertThat(ProgramWriter.written(program))
                .contains("\"core\":\"temporal\",\"text\":\"2026-07-25\"");
    }

    /**
     * An arm binding a name where it tests that an optional holds nothing is admitted with no type
     * for the name (souther-lang/souther#1984), and there is nothing to write it as. Refused as not
     * lowered, naming that, rather than written with a type this side made up.
     */
    @Test
    void anArmBindingANameToNothingIsNotLoweredYet() {
        CheckedProgram program = Checked.of(List.of("""
                module absent exposing ( counted, Held )

                data Held = { o: Int? }

                behavior counted : (h: Held) -> Int
                let counted (h) = match h.o with
                    | Some x -> x
                    | None as n -> 0
                """));

        assertThatThrownBy(() -> ProgramWriter.written(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("souther-lang/souther#1984");
    }

    @Test
    void theDriverSaysItIsOneThisBackendHasNotGotRoundTo() {
        assertThatThrownBy(() -> NativeCompiler.compile(Checked.of(List.of(OVER_A_SET))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Set");
    }

    @Test
    void theCommandLineSaysTheBackendIsBehindAndNotThatTheCommandWasWrong() throws Exception {
        Path source = Files.createTempDirectory("souther-native-test").resolve("calculation.sou");
        Files.writeString(source, OVER_A_SET, StandardCharsets.UTF_8);
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
