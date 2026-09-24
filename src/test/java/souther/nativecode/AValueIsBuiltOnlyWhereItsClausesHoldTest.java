package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A construction of a type that states what its values owe runs those clauses, in the order the
 * type states them, and answers the value only where every one holds.
 *
 * <p>Each ending is put beside a run that answers through the same executable, for the reason the
 * ends of an {@code Int} are: an abort on its own is the absence of an answer, and a symbol that did
 * not resolve looks the same from here. The run that answers is what says the ending was the
 * clause's.
 *
 * <p>What a behavior answers is read off a field of what it built rather than the value itself, so
 * what is asked is whether the value was built and not how a value of the type is written out.
 */
class AValueIsBuiltOnlyWhereItsClausesHoldTest {

    private static final long MOST = Long.MAX_VALUE;

    private static final String SOURCE = """
            module owing exposing ( checked, width, named, twice )

            data Positive = Int
                invariant positive = value > 0

            data Span = { lo: Int, hi: Int }
                invariant ordered = lo <= hi
                invariant roomy = lo + 1 <= hi

            data Labelled = { ...Span, label: String }

            behavior checked : (n: Int) -> Int
            let checked (n) = Positive(n).value

            behavior width : (lo: Int, hi: Int) -> Int
            let width (lo, hi) = {
                let span = Span { lo = lo, hi = hi }
                span.hi - span.lo
            }

            behavior named : (lo: Int, hi: Int) -> Int
            let named (lo, hi) = {
                let span = Labelled { lo = lo, hi = hi, label = "between" }
                span.hi - span.lo
            }

            behavior twice : (n: Int) -> Int
            let twice (n) = Positive(Positive(n).value * 2).value
            """;

    @Test
    void aValueWhoseClauseHoldsIsBuiltAndOneWhoseClauseDoesNotEndsTheRun() throws Exception {
        assertEndsButItsNeighbourAnswers("checked", List.of(0L), AbortKind.INVARIANT_NOT_HELD,
                List.of(1L), 1);
        assertEndsButItsNeighbourAnswers("checked", List.of(-7L), AbortKind.INVARIANT_NOT_HELD,
                List.of(7L), 7);
    }

    /**
     * The clauses run in the order the type states them and the first that does not hold is where
     * the run ends: with {@code lo} at the top of the range and {@code hi} below it, {@code ordered}
     * fails before {@code roomy} is reached, and {@code roomy} is the one that would have left the
     * range.
     */
    @Test
    void theFirstClauseThatDoesNotHoldIsWhereTheRunEnds() throws Exception {
        assertEndsButItsNeighbourAnswers("width", List.of(MOST, 0L), AbortKind.INVARIANT_NOT_HELD,
                List.of(1L, 5L), 4);
        assertEndsButItsNeighbourAnswers("width", List.of(3L, 3L), AbortKind.INVARIANT_NOT_HELD,
                List.of(3L, 4L), 1);
    }

    /**
     * A clause that leaves an {@code Int}'s range did not answer false. It did not answer, and the
     * run ends for that reason and not because the clause failed.
     */
    @Test
    void aClauseThatEndsWithoutAnAnswerEndsTheRunForItsOwnReason() throws Exception {
        assertEndsButItsNeighbourAnswers("width", List.of(MOST, MOST),
                AbortKind.REQUIRED_FORM_HAS_NO_PLACE, List.of(MOST - 1, MOST), 1);
    }

    /** A clause a spread takes in holds a value of the type that takes it in. */
    @Test
    void aClauseASpreadTakesInIsRunWhereItIsTakenIn() throws Exception {
        assertEndsButItsNeighbourAnswers("named", List.of(5L, 1L),
                AbortKind.INVARIANT_NOT_HELD, List.of(1L, 5L), 4);
    }

    /** Every construction runs the clauses, one built from what another was built from too. */
    @Test
    void eachConstructionRunsTheClauses() throws Exception {
        assertEndsButItsNeighbourAnswers("twice", List.of(0L), AbortKind.INVARIANT_NOT_HELD,
                List.of(3L), 6);
        assertEndsButItsNeighbourAnswers("twice", List.of(MOST), AbortKind.REQUIRED_FORM_HAS_NO_PLACE,
                List.of(MOST / 2), MOST / 2 * 2);
    }

    /**
     * Rows over types that state what their values owe hold natively, the way every other row does,
     * a clause reading a field a spread took in among them.
     */
    @Test
    void everyRowOverATypeThatStatesClausesHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds("""
                module rows

                data Positive = Int
                    invariant positive = value > 0

                data Span = { lo: Int, hi: Int }
                    invariant ordered = lo <= hi

                data Labelled = { ...Span, label: String }

                behavior width : (lo: Int, hi: Int) -> Int
                let width (lo, hi) = {
                    let span = Labelled { lo = lo, hi = hi, label = "between" }
                    span.hi - span.lo + Positive(1).value
                }

                example width
                    | "apart" : (1, 5) -> 5
                    | "together" : (2, 2) -> 1
                """);
    }

    /**
     * A clause may name a unit's value, which is built while the clause runs, and whatever the
     * module says of the unit: kept, and named nowhere but in the clause, it is still there to be
     * built.
     */
    @Test
    void aClauseThatNamesAUnitRuns() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module ready exposing ( checked )

                data Ready
                data Waiting
                data State = Ready | Waiting

                let accepts (s: State): Bool = match s with
                    | Ready -> true
                    | Waiting -> false

                data Positive = Int
                    invariant ready = accepts(Ready)
                    invariant positive = value > 0

                behavior checked : (n: Int) -> Int
                let checked (n) = Positive(n).value
                """));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior checked = module.behaviors().getFirst();

        assertThat(running.answeredOrEnded(module, checked, given(List.of(0L))))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.INVARIANT_NOT_HELD));
        assertThat(running.answeredOrEnded(module, checked, given(List.of(5L))))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(5)));
    }

    private static void assertEndsButItsNeighbourAnswers(String behavior, List<Long> ends,
                                                         AbortKind with, List<Long> answers,
                                                         long answered) throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SOURCE));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior reached = module.behaviors().stream()
                .filter(it -> it.name().name().equals(behavior))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + behavior));

        assertThat(running.answeredOrEnded(module, reached, given(ends)))
                .as("%s handed %s", behavior, ends)
                .isEqualTo(new RunOutcome.Aborted(with));
        assertThat(running.answeredOrEnded(module, reached, given(answers)))
                .as("%s handed %s, which is the control the ending above is read against",
                        behavior, answers)
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(answered)));
    }

    private static List<ObservedValue> given(List<Long> values) {
        return values.stream().map(it -> (ObservedValue) new ObservedValue.Integer(it)).toList();
    }
}
