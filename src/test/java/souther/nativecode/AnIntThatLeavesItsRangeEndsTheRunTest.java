package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

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
 * <p>Which abort it was is not asked. Nothing carries a reason out of a native run.
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
            module edges exposing ( add, less, times, flip )

            behavior add : (a: Int, b: Int) -> Int
            let add (a, b) = a + b

            behavior less : (a: Int, b: Int) -> Int
            let less (a, b) = a - b

            behavior times : (a: Int, b: Int) -> Int
            let times (a, b) = a * b

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

    /** Turning the smallest `Int` around is a subtraction from nought like any other. */
    @Test
    void theSmallestIntTurnedAroundEndsTheRunAndTheNextOneAlongAnswers() throws Exception {
        assertEndsButItsNeighbourAnswers("flip", List.of(LEAST), List.of(LEAST + 1), MOST);
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
                    .isEmpty();
            assertThat(running.answeredOrEnded(module, reached, given(answers)))
                    .as("%s handed %s, which is the control the ending above is read against",
                            behavior, answers)
                    .contains(new ObservedValue.Integer(with));
        }
    }

    private static List<ObservedValue> given(List<Long> values) {
        return values.stream().map(it -> (ObservedValue) new ObservedValue.Integer(it)).toList();
    }
}
