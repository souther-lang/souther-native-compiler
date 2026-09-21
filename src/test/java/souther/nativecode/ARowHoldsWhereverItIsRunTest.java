package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.Verdict;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedRow;
import souther.compiler.diag.CompileException;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * What a native run answers, held against what the program's own rows say.
 *
 * <p>The rows are the oracle and not this project's idea of one, and what that establishes is worth
 * stating exactly. A row arrives as {@link CheckedRow.SelfContained} only where the compile ran it,
 * and a row that ran whose answer did not keep it refuses the program — so for such a row the JVM
 * answered and the answer kept the row. Putting the native run to the same row therefore holds both
 * carriers to one statement without this backend writing down what either of them should say.
 *
 * <p>What it says nothing about is a row the compile did not run. Acceptance says so itself: a row
 * whose classes will not link observes nothing and does not refuse the program. Such a row arrives
 * as {@link CheckedRow.NotReproducible} carrying why, so the checked program does say which rows
 * those are — and a reader that filtered them out would quietly compare fewer rows than the program
 * states and stay green. Every arm is answered below for that reason.
 *
 * <p>Nor is this carriers compared against each other. Holding two of them to one statement is not
 * running both and comparing what came back.
 *
 * <p>Whether an answer is the one a row states is asked of the row. A test deciding that for itself
 * would be a second reading of what a row means, and the two carriers would then agree only as far
 * as this file agreed with the language.
 */
class ARowHoldsWhereverItIsRunTest {

    private static final String ARITHMETIC = """
            module calculation

            behavior add : (a: Int, b: Int) -> Int
            let add (a, b) = a + b

            example add
                | "two and forty" : (2, 40) -> 42
                | "one of them below nought" : (-5, 3) -> -2
                | "nothing and nothing" : (0, 0) -> 0
            """;

    private static final String COMPARING = """
            module comparing

            behavior less : (a: Int, b: Int) -> Int
            let less (a, b) = a - b

            behavior times : (a: Int, b: Int) -> Int
            let times (a, b) = a * b

            behavior atLeast : (a: Int, b: Int) -> Bool
            let atLeast (a, b) = a >= b

            behavior same : (a: Bool, b: Bool) -> Bool
            let same (a, b) = a == b

            behavior larger : (a: Int, b: Int) -> Int
            let larger (a, b) = if a > b then a else b

            example less
                | "what is left of it" : (40, 2) -> 38
                | "past nought" : (2, 40) -> -38

            example times
                | "twice" : (21, 2) -> 42
                | "by nothing" : (21, 0) -> 0
                | "signs" : (-6, 7) -> -42

            example atLeast
                | "above" : (2, 1) -> true
                | "level" : (1, 1) -> true
                | "below" : (0, 1) -> false

            example same
                | "both" : (true, true) -> true
                | "one of them" : (true, false) -> false

            example larger
                | "the left one" : (40, 2) -> 40
                | "the right one" : (2, 40) -> 40
                | "neither" : (7, 7) -> 7
            """;

    /**
     * A condition whose right side would abort at the values its left side exists to exclude.
     *
     * <p>Which operands run is part of what `&&` and `||` mean, so a row here is not about a
     * backend's arrangement: lowered eagerly, the multiplication leaves the range an `Int` holds
     * and the run ends where the language says it answers.
     */
    private static final String STOPPING = """
            module stopping

            behavior small : (a: Int) -> Bool
            let small (a) = a < 1000 && a * a < 1000000

            behavior anyAtAll : (a: Int) -> Bool
            let anyAtAll (a) = a > 0 || a * a > 0

            example small
                | "small enough to square" : (10) -> true
                | "too large to square at all" : (4000000000) -> false

            example anyAtAll
                | "settled by the left" : (4000000000) -> true
                | "the right one decides" : (-3) -> true
            """;

    private static final String NAMING = """
            module naming

            behavior flip : (a: Int) -> Int
            let flip (a) = -a

            behavior spread : (a: Int, b: Int) -> Int
            let spread (a, b) = {
                let low = if a < b then a else b
                let high = if a > b then a else b
                high - low
            }

            example flip
                | "above nought" : (7) -> -7
                | "below it" : (-7) -> 7
                | "nought itself" : (0) -> 0

            example spread
                | "the left one is larger" : (40, 2) -> 38
                | "the right one is" : (2, 40) -> 38
                | "neither" : (7, 7) -> 0
            """;

    @Test
    void everyRowOfEveryBehaviorHoldsWhenTheNativeObjectAnswersIt() throws Exception {
        assertEveryRowHolds(ARITHMETIC);
        assertEveryRowHolds(COMPARING);
        assertEveryRowHolds(STOPPING);
        assertEveryRowHolds(NAMING);
    }

    /**
     * What makes the oracle an oracle. Were a row's answer merely what its author typed, a row
     * would say nothing about what the JVM does, and agreeing with one would be agreeing with the
     * person who wrote it.
     */
    @Test
    void aRowStatingWhatTheJvmDoesNotAnswerIsNotAcceptedAtAll() {
        assertThatThrownBy(() -> CheckedProgram.of(List.of("""
                module calculation

                behavior add : (a: Int, b: Int) -> Int
                let add (a, b) = a + b

                example add
                    | "what nothing answers" : (2, 40) -> 41
                """)))
                .isInstanceOf(CompileException.class)
                .hasMessageContaining("41");
    }

    /**
     * Runs every row the program states and asks the row whether the answer keeps it.
     *
     * <p>Every row, and every way a row can arrive. A corpus written to be run is one where each
     * row ran, so a row arriving any other way is this test's population having shrunk under it —
     * which is the one thing a count of what it did compare could never tell it. Said as a switch
     * with no arm standing for the rest, so a way of arriving added later has to be answered here.
     */
    static void assertEveryRowHolds(String source) throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(source));
        int asked = 0;
        try (Running running = Running.of(program)) {
            for (CheckedModule module : program.modules()) {
                for (CheckedBehavior behavior : module.behaviors()) {
                    for (CheckedRow row : behavior.rows()) {
                        String where = row.identity() + " of " + behavior.name();
                        switch (row.statement()) {
                            case CheckedRow.SelfContained states -> {
                                List<ObservedValue> inputs = states.states().inputs();
                                ObservedValue answered =
                                        running.answering(module, behavior, inputs);

                                assertThat(states.holds(answered))
                                        .as("%s, handed %s, answered %s", where, inputs, answered)
                                        .isInstanceOf(Verdict.Held.class);
                                asked++;
                            }
                            // Nothing in this corpus depends on anything or owes its answer, and a
                            // row that did is one nobody put the two carriers to.
                            case CheckedRow.WithStandIns states -> throw new AssertionError(
                                    where + " needs something stood in for: " + states.standsIn());
                            case CheckedRow.AnswerOwed states -> throw new AssertionError(
                                    where + " states no answer to hold anything to: " + states);
                            // The compile did not run it, and says why. Left out silently, this
                            // test would go on being green over fewer and fewer rows.
                            case CheckedRow.NotReproducible why -> throw new AssertionError(
                                    where + " was not run by the compile, so nothing about it was"
                                            + " observed on the JVM either: " + why.why());
                        }
                    }
                }
            }
        }
        assertThat(asked)
                .as("a program whose rows were never reached says nothing about either carrier")
                .isPositive();
    }
}
