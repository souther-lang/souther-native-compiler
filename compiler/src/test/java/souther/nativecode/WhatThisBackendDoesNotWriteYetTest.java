package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * What this backend does not write yet, and that it is told apart from a document the two halves
 * disagree about.
 *
 * <p>Three answers are possible at the end of this path and only one of them is right: the program
 * is refused by the language, this backend has not got round to it, or the command was wrong.
 * Which one a reader is told decides whether they go and change their program. No program the
 * checker accepts reaches a refusal of the writer's today (the last, an arm naming an absent
 * optional, went once the checker said what such a name stands for), so the driver's half is asked
 * of a document.
 */
class WhatThisBackendDoesNotWriteYetTest {

    private static final String OVER_A_SET = """
            module calculation

            behavior widen : (a: Set<Int>) -> Set<Int>

            let widen (a) = a
            """;

    /**
     * The smallest document that reaches a lowering this driver does not have: two values of the
     * type of what has no value, compared. No program the checker accepts is refused as not lowered
     * by the writer today, so the driver's half of the path is asked with a document written by
     * hand, the one `native/crates/compiler/tests/refusals.rs` refuses the same way.
     */
    private static final String COMPARING_NOTHING = ("{\"transport\":" + ProgramWriter.TRANSPORT_VERSION
            + ",\"declarations\":[],\"behaviors\":[],\"modules\":[{\"name\":\"calculation\","
            + "\"publishes\":[],\"helpers\":[{\"reached\":{\"is\":\"own\",\"module\":\"calculation\","
            + "\"name\":\"f\"},\"parameters\":[{\"name\":\"a\",\"type\":{\"nothing\":{}}},"
            + "{\"name\":\"b\",\"type\":{\"nothing\":{}}}],\"body\":{\"core\":\"binary\",\"op\":\"EQ\","
            + "\"reading\":{\"is\":\"astheystand\"},"
            + "\"left\":{\"core\":\"read\",\"binding\":0,\"type\":{\"nothing\":{}},\"aborts\":[]},"
            + "\"right\":{\"core\":\"read\",\"binding\":1,\"type\":{\"nothing\":{}},\"aborts\":[]},"
            + "\"type\":{\"prim\":\"BOOL\"},\"aborts\":[]}}],\"values\":[],\"entries\":[],"
            + "\"definitions\":[],\"examples\":[]}]}");

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
     * Raoh's {@code unique} takes only elements an issue can write, which Raoh gives a message form
     * to (spec decoder-language.md, Types), and Souther states it of any list
     * (souther-lang/souther#2149). A list of what has none, an optional, a product, a unit, is not
     * written until Souther decides what such a clause is, rather than reported with elements no
     * host's Raoh holds as a value of the type they are.
     */
    @Test
    void aUniqueOfElementsAnIssueCannotWriteIsNotWrittenYet() {
        for (String elements : List.of("Option<Int>", "Point", "Marker")) {
            CheckedProgram program = Checked.of(List.of("""
                    module shelf exposing ( Point, Marker, Held )

                    data Point = { x: Int, y: Int }
                    data Marker

                    data Held = List<%s>
                        invariant List.allDistinctBy(x -> x, value)
                    """.formatted(elements)));

            assertThatThrownBy(() -> NativeCompiler.compile(program))
                    .as("a list of %s", elements)
                    .isInstanceOf(NotLowered.class)
                    .hasMessageContaining("souther-lang/souther#2149");
        }
    }

    /** What the driver has no lowering for arrives as that, and not as a document it could not read. */
    @Test
    void theDriverSaysItIsOneThisBackendHasNotGotRoundTo() {
        assertThatThrownBy(() -> NativeCompiler.driven(COMPARING_NOTHING))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Nothing");
    }
}
