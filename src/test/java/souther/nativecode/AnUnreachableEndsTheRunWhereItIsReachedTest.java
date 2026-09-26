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
 * An {@code unreachable} ends the run where it is reached, and nowhere else.
 *
 * <p>Asked of each place it stands beside the input that does not reach it, through the same
 * executable, for the reason {@link AnIntThatLeavesItsRangeEndsTheRunTest} gives: a run that ends
 * is on its own only the absence of an answer, and the answer beside it is what says the ending
 * was this {@code unreachable}'s. Where the position states a type the checker types it as that,
 * and where it states none, as what does not answer, which the fork around it is then typed past.
 */
class AnUnreachableEndsTheRunWhereItIsReachedTest {

    private static final String SOURCE = """
            module ending exposing ( positive, never, kept, joined, forked )

            behavior positive : (a: Int) -> Int
            let positive (a) = if a > 0 then a else unreachable "not positive"

            behavior never : (a: Int) -> Int
            let never (a) = unreachable "never answers"

            behavior joined : (a: Int) -> Int
            let joined (a) = {
                let n = if a > 0 then a else unreachable "not positive"
                n + 1
            }

            behavior forked : (a: Int) -> Int
            let forked (a) = {
                let n = if a > 0 then a else if a < -5 then unreachable "far" else unreachable "near"
                n + 1
            }

            let keep (e: DivisionByZero): Int = 424242

            behavior kept : (a: Int, b: Int) -> Int
            let kept (a, b) = match Int.truncatingDivide(a, b) with
                | Int as q -> q
                | DivisionByZero as e -> keep(e)
            """;

    @Test
    void anArmThatIsReachedEndsTheRunAndTheOneBesideItAnswers() throws Exception {
        assertThat(run("positive", 0L))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.UNREACHABLE_REACHED));
        assertThat(run("positive", 7L))
                .as("the branch beside the unreachable, which the ending above is read against")
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(7)));
    }

    /**
     * Where the position states no type, the checker types the {@code unreachable} as what does
     * not answer and the fork as what its other branch answers. The width is the fork's, as the
     * checker's own emitter takes it, and a fork whose every branch ends the run ends it.
     */
    @Test
    void anUnreachableWhereNothingStatesATypeTakesTheForksAndEndsTheRun() throws Exception {
        assertThat(run("joined", 0L))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.UNREACHABLE_REACHED));
        assertThat(run("joined", 7L))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(8)));
        assertThat(run("forked", -9L))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.UNREACHABLE_REACHED));
        assertThat(run("forked", 0L))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.UNREACHABLE_REACHED));
        assertThat(run("forked", 7L))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(8)));
    }

    @Test
    void aBodyThatIsUnreachableEndsEveryRun() throws Exception {
        assertThat(run("never", 1L))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.UNREACHABLE_REACHED));
    }

    /**
     * An arm binding a case the language gives binds it as the type the checker names it by, the
     * case on its own, and that is handed on as a union holds it. What was refused as a case named
     * where only a declaration stands is now laid out.
     */
    @Test
    void aCaseTheLanguageGivesIsBoundAndHandedOnAsItIs() throws Exception {
        assertThat(run("kept", 7L, 0L))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(424242)));
        assertThat(run("kept", 7L, 2L))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(3)));
    }

    private static RunOutcome run(String behavior, Long... values) throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SOURCE));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior reached = module.behaviors().stream()
                .filter(it -> it.name().name().equals(behavior))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + behavior));
        List<ObservedValue> given = List.of(values).stream()
                .map(it -> (ObservedValue) new ObservedValue.Integer(it))
                .toList();
        return running.answeredOrEnded(module, reached, given);
    }
}
