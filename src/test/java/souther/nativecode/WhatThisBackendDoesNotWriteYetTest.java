package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A program the language admits that this backend does not write yet, followed all the way out.
 *
 * <p>Three answers are possible at the end of this path and only one of them is right: the program
 * is refused by the language, this backend has not got round to it, or the command was wrong.
 * Which one a reader is told decides whether they go and change their program.
 *
 * <p>The middle answer has no example here today: `Set` and `Map` (`ASetIsHeldByItsMembersTest`)
 * closed the gaps this class held open for them, and the arm this class once refused a binder over
 * crosses now too. What a reader is told when this backend really is behind — {@code NotLowered},
 * and {@code Main}'s "this backend does not write that yet" over it — is still
 * {@code NativeCompiler}'s and {@code Main}'s own contract, and the next gap this class finds
 * should hold that path to it again.
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
     * An arm binding a name where it tests that an optional holds nothing is read as the optional
     * itself ({@link souther.compiler.core.Core.Case#bindType()}), which is the type
     * {@code n} stands as. The checker never admits such a binder without settling its type, so
     * this crosses like any other arm.
     */
    @Test
    void anArmBindingANameToNothingCrossesAsTheOptionalItself() {
        CheckedProgram program = Checked.of(List.of("""
                module absent exposing ( counted, Held )

                data Held = { o: Int? }

                behavior counted : (h: Held) -> Int
                let counted (h) = match h.o with
                    | Some x -> x
                    | None as n -> 0
                """));

        assertThat(ProgramWriter.written(program))
                .contains("\"selects\":[{\"tests\":\"nothing\"}],\"binding\":2,"
                        + "\"binds\":{\"option\":{\"prim\":\"INT\"}}");
    }

    /**
     * The writer's shape crosses `Coherent` and runs: {@code n}, bound to the optional itself, is
     * read back out and answers what it was bound to, the way any other binder does. Checked here
     * and not only above, because a shape that crosses is not yet a shape the driver accepts — the
     * two are different questions this backend has answered wrongly apart before.
     */
    @Test
    void anArmBindingANameToNothingIsRunWithTheOptionalItBoundIt() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds("""
                module absent exposing ( counted, Held )

                data Held = { o: Int? }

                behavior counted : (h: Held) -> Int
                let counted (h) = match h.o with
                    | Some x -> x
                    | None as n -> if n == h.o then 0 else 1

                example counted
                    | "held" : (Held { o = 5 }) -> 5
                    | "absent" : (Held { o = None }) -> 0
                """);
    }
}
