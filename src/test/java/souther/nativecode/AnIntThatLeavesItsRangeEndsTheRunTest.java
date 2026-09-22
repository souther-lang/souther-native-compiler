package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * The ends of what an `Int` holds, where the checks this backend writes are decided.
 *
 * <p>An overflow check is a fact about sign bits at two values and about nothing else, so a corpus
 * of ordinary numbers passes through one whether it is right, wrong or missing. These are the pairs
 * that tell those apart.
 *
 * <p>Each of them is asked twice. A run that ends is what the language says overflowing does, but
 * on its own it is the absence of an answer and every other way a run can fail looks the same from
 * here — a symbol that did not resolve, a harness that would not start. So each ending is put
 * beside the nearest pair that answers, through the same executable: the pair that answers is what
 * says the ending was this computation's and not the arrangement around it.
 *
 * <p>Since issue #9, which abort it was is asked, and it is always {@code
 * REQUIRED_FORM_HAS_NO_PLACE} — the one law the specification states for an {@code Int} leaving
 * the range it holds, whichever of these four operations reaches it.
 */
class AnIntThatLeavesItsRangeEndsTheRunTest {

    private static final long MOST = Long.MAX_VALUE;
    private static final long LEAST = Long.MIN_VALUE;

    /**
     * Published, because what is reached here is the behavior itself and not one of its rows: the
     * values are the ends of the range and no row states them. A name the module kept would be
     * local to the object and reached by nothing out here.
     */
    private static final String SOURCE = """
            module edges exposing ( add, less, times )

            behavior add : (a: Int, b: Int) -> Int
            let add (a, b) = a + b

            behavior less : (a: Int, b: Int) -> Int
            let less (a, b) = a - b

            behavior times : (a: Int, b: Int) -> Int
            let times (a, b) = a * b
            """;

    /**
     * A module of its own, kept apart from {@link #SOURCE}: {@code flip} does not compile on this
     * backend today, and a module that failed to compile at all would take {@code add}, {@code
     * less} and {@code times} down with it — they are one object, and {@link Running#of} builds
     * the whole of it or none of it.
     */
    private static final String FLIPPING = """
            module flipping exposing ( flip )

            behavior flip : (a: Int) -> Int
            let flip (a) = -a
            """;

    @Test
    void asumPastTheEndEndsTheRunAndTheOneBesideItAnswers() throws Exception {
        assertEndsButItsNeighbourAnswers("add", List.of(MOST, 1L), List.of(MOST, 0L), MOST);
        assertEndsButItsNeighbourAnswers("add", List.of(LEAST, -1L), List.of(LEAST, 0L), LEAST);
    }

    @Test
    void aDifferencePastTheEndEndsTheRunAndTheOneBesideItAnswers() throws Exception {
        assertEndsButItsNeighbourAnswers("less", List.of(LEAST, 1L), List.of(LEAST, 0L), LEAST);
        assertEndsButItsNeighbourAnswers("less", List.of(MOST, -1L), List.of(MOST, 0L), MOST);
    }

    /**
     * Turning the smallest {@code Int} around is a subtraction from nought like any other, and
     * mathematically overflows exactly as much as {@code Int.MIN - 1} does. {@code
     * CheckedProgram#abortsAt} answers {@code AbortSet.NONE} for {@code Core.Neg} today, though
     * (souther-lang/souther#1878) — a known-wrong answer, since negation overflows for the same
     * representational reason {@code +}/{@code -}/{@code *} do. This backend trusts {@code
     * abortsAt} everywhere else, which is the whole point of reading it off the checker instead of
     * re-deriving it (see {@code overflow_status}'s own doc in the driver crate), but a machine
     * condition it can already see firing and a checker answer of no reason at all is exactly the
     * two halves disagreeing about what kind of site this is — so rather than trust it into
     * answering {@code Int.MIN} back as though nothing had happened, this refuses the program.
     * {@code NotLowered} and not a wrong value: once #1878 lands this behaves like the other three
     * and the shared helper above covers it.
     */
    @Test
    void theSmallestIntTurnedAroundIsNotLoweredPendingSoutherIssue1878() {
        CheckedProgram program = CheckedProgram.of(List.of(FLIPPING));

        assertThatThrownBy(() -> Running.of(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("souther-lang/souther#1878");
    }

    @Test
    void aProductPastTheEndEndsTheRunAndTheOneBesideItAnswers() throws Exception {
        assertEndsButItsNeighbourAnswers("times", List.of(MOST, 2L), List.of(MOST, 1L), MOST);
        assertEndsButItsNeighbourAnswers("times", List.of(LEAST, -1L), List.of(LEAST, 1L), LEAST);
    }

    private static void assertEndsButItsNeighbourAnswers(String behavior, List<Long> ends,
                                                         List<Long> answers, long with)
            throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SOURCE));
        try (Running running = Running.of(program)) {
            CheckedModule module = program.modules().getFirst();
            CheckedBehavior reached = module.behaviors().stream()
                    .filter(it -> it.name().name().equals(behavior))
                    .findFirst()
                    .orElseThrow(() -> new AssertionError("no behavior " + behavior));

            assertThat(running.answeredOrEnded(module, reached, given(ends)))
                    .as("%s handed %s", behavior, ends)
                    .isEqualTo(new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
            assertThat(running.answeredOrEnded(module, reached, given(answers)))
                    .as("%s handed %s, which is the control the ending above is read against",
                            behavior, answers)
                    .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(with)));
        }
    }

    private static List<ObservedValue> given(List<Long> values) {
        return values.stream().map(it -> (ObservedValue) new ObservedValue.Integer(it)).toList();
    }
}
