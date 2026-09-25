package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * An attempted construction runs the type's clauses the way a construction does, and where one does
 * not hold it takes the arm naming that clause instead of ending the run.
 *
 * <p>Each behavior answers a number that says which way the run went: the value's width where it
 * was built, and a negative number per arm otherwise. The rows below are the JVM's answers, and
 * every one of them is run natively too.
 */
class AnAttemptedConstructionTakesTheArmItsClauseNamesTest {

    private static final long MOST = Long.MAX_VALUE;

    private static final String SOURCE = """
            module attempting exposing ( measured, reordered, narrowed, anyway )

            data Span = { lo: Int, hi: Int }
                invariant ordered = lo <= hi
                invariant roomy = lo + 1 <= hi

            data Bounded = { lo: Int, hi: Int }
                invariant ordered = lo <= hi
                invariant hi - lo <= 100

            behavior measured : (lo: Int, hi: Int) -> Int
            let measured (lo, hi) = {
                guard Span { lo = lo, hi = hi } as s
                    else | ordered -> -1 | roomy -> -2
                s.hi - s.lo
            }

            behavior reordered : (lo: Int, hi: Int) -> Int
            let reordered (lo, hi) = {
                guard Span { lo = lo, hi = hi } as s
                    else | roomy -> -2 | ordered -> -1
                s.hi - s.lo
            }

            behavior narrowed : (lo: Int, hi: Int) -> Int
            let narrowed (lo, hi) = {
                guard Bounded { lo = lo, hi = hi } as b
                    else | ordered -> -1 | _ -> -3
                b.hi - b.lo
            }

            behavior anyway : (lo: Int, hi: Int) -> Int
            let anyway (lo, hi) = {
                guard Span { lo = lo, hi = hi } as s else 0
                s.hi - s.lo
            }

            example measured
                | "built" : (1, 5) -> 4
                | "out of order" : (5, 1) -> -1
                | "no room" : (3, 3) -> -2

            example reordered
                | "built" : (1, 5) -> 4
                | "out of order" : (5, 1) -> -1
                | "no room" : (3, 3) -> -2

            example narrowed
                | "built" : (0, 100) -> 100
                | "out of order" : (5, 1) -> -1
                | "too wide" : (0, 500) -> -3

            example anyway
                | "built" : (1, 5) -> 4
                | "out of order" : (5, 1) -> 0
                | "no room" : (3, 3) -> 0
            """;

    @Test
    void everyRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(SOURCE);
    }

    /** What the branch reads is the value that was built, and each clause takes its own arm. */
    @Test
    void theArmTakenIsTheOneNamingTheClauseThatDidNotHold() throws Exception {
        assertThat(ran("measured", 1, 5)).isEqualTo(answered(4));
        assertThat(ran("measured", 5, 1)).isEqualTo(answered(-1));
        assertThat(ran("measured", 3, 3)).isEqualTo(answered(-2));
    }

    /** An arm is found by the clause it names, whichever order the arms are written in. */
    @Test
    void theArmsAreMatchedByNameAndNotByWhereTheyAreWritten() throws Exception {
        assertThat(ran("reordered", 5, 1)).isEqualTo(answered(-1));
        assertThat(ran("reordered", 3, 3)).isEqualTo(answered(-2));
    }

    /** A clause with no name is answered by the arm naming none. */
    @Test
    void aClauseWithNoNameTakesTheArmNamingNone() throws Exception {
        assertThat(ran("narrowed", 0, 500)).isEqualTo(answered(-3));
        assertThat(ran("narrowed", 5, 1)).isEqualTo(answered(-1));
    }

    /** One {@code else} answers every clause. */
    @Test
    void oneElseAnswersEveryClause() throws Exception {
        assertThat(ran("anyway", 5, 1)).isEqualTo(answered(0));
        assertThat(ran("anyway", 3, 3)).isEqualTo(answered(0));
        assertThat(ran("anyway", 1, 5)).isEqualTo(answered(4));
    }

    /**
     * The clauses run in the order the type states them, and nothing after the first that does not
     * hold runs: with {@code lo} at the top of the range, {@code ordered} does not hold, and
     * {@code roomy}, which would have left the range, is never reached.
     */
    @Test
    void nothingAfterTheClauseThatDidNotHoldRuns() throws Exception {
        assertThat(ran("measured", MOST, 0)).isEqualTo(answered(-1));
    }

    /**
     * A clause that leaves an {@code Int}'s range did not answer false, so no arm answers it: the
     * run ends for that reason, as a construction's does.
     */
    @Test
    void aClauseThatEndsWithoutAnAnswerEndsTheRunAndTakesNoArm() throws Exception {
        assertThat(ran("measured", MOST, MOST))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(ran("measured", MOST - 1, MOST)).isEqualTo(answered(1));
    }

    /**
     * It crosses as one node that says which way the run goes, and not as a construction: a
     * construction under it would read as one that ends the run where a clause does not hold.
     */
    @Test
    void itCrossesAsAnAttemptAndNotAsAConstruction() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(SOURCE)));

        assertThat(written)
                .contains("{\"core\":\"attempt\",\"declared\":\"attempting.Span\"")
                .contains("\"departures\":[{\"clause\":\"roomy\"")
                .contains("\"departures\":[{\"clause\":null")
                .doesNotContain("\"core\":\"construct\"");
    }

    private static RunOutcome ran(String behavior, long lo, long hi) throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SOURCE));
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior reached = module.behaviors().stream()
                .filter(it -> it.name().name().equals(behavior))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + behavior));
        return Running.of(program).answeredOrEnded(module, reached,
                List.of(new ObservedValue.Integer(lo), new ObservedValue.Integer(hi)));
    }

    private static RunOutcome answered(long value) {
        return new RunOutcome.Answered(new ObservedValue.Integer(value));
    }
}
