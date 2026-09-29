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
 * A call reaching {@code Core.Reached.OfKernel}, carried the whole way: written by {@link
 * souther.nativecode.transport.ProgramWriter} without refusing it, read by the driver as {@code
 * reaches: "kernel"}, and lowered by reusing the same {@code iadd} instructions {@code a + b}
 * already answers with — the first kernel this backend lowers, chosen for issue #9 because it
 * proves the whole path (writer, transport, kernel dispatch, the {@code status + out} abort ABI)
 * with no new representation and no new runtime algorithm to write.
 *
 * <p>{@code Int.add(a, b)} and {@code a + b} are the same site kind by a different route to it —
 * both {@code REQUIRED_FORM_HAS_NO_PLACE} on overflow — so this is read beside {@link
 * AnAdditionCrossesAsWhatTheCheckerSettledTest} and {@link AnIntThatLeavesItsRangeEndsTheRunTest}
 * rather than repeating what they already cover about the operator itself.
 */
class AKernelCallCrossesAsWhatTheLanguageImplementsTest {

    private static final String SOURCE = """
            module calculation exposing ( add )

            behavior add : (a: Int, b: Int) -> Int
            let add (a, b) = Int.add(a, b)
            """;

    @Test
    void aKernelCallAnswersTheSameSumAnOperatorDoes() throws Exception {
        CheckedProgram program = Checked.of(List.of(SOURCE));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior add = module.behaviors().stream()
                .filter(it -> it.name().name().equals("add"))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior add"));

        assertThat(running.answeredOrEnded(module, add,
                List.of(new ObservedValue.Integer(2), new ObservedValue.Integer(40))))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(42)));
    }

    /**
     * The acceptance test issue #9 exists for: a kernel call's own overflow answers the same
     * {@link AbortKind} a binary operator's does, carried out of the run rather than potted as an
     * undifferentiated ending.
     */
    @Test
    void aKernelCallPastTheEndAnswersRequiredFormHasNoPlace() throws Exception {
        CheckedProgram program = Checked.of(List.of(SOURCE));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior add = module.behaviors().stream()
                .filter(it -> it.name().name().equals("add"))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior add"));

        assertThat(running.answeredOrEnded(module, add,
                List.of(new ObservedValue.Integer(Long.MAX_VALUE),
                        new ObservedValue.Integer(1))))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
    }
}
