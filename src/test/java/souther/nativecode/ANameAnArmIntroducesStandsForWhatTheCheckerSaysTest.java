package souther.nativecode;

import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The name an arm introduces stands for what the checker says it does ({@code Core.ArmBinding}):
 * {@code Some v} names what the optional holds, and {@code None as n} the optional itself, which
 * can be matched again. Which of the two is written out by the checker and read here, not worked
 * out from what the arm tests.
 */
class ANameAnArmIntroducesStandsForWhatTheCheckerSaysTest {

    private static final String SOURCE = """
            module absent exposing ( counted )

            behavior counted : (n: Int) -> Int
            let counted (n) = match List.get(n, [7]) with
                | Some v -> v + 1
                | None as none -> {
                    match none with
                        | Some w -> w
                        | None -> -1
                }
            """;

    private static CheckedProgram program;
    private static Running running;
    private static CheckedModule module;

    @BeforeAll
    static void build() throws Exception {
        program = Checked.of(List.of(SOURCE));
        running = Running.of(program);
        module = program.modules().getFirst();
    }

    private static RunOutcome counted(long n) throws Exception {
        CheckedBehavior behavior = module.behaviors().getFirst();
        return running.answeredOrEnded(module, behavior, List.of(new ObservedValue.Integer(n)));
    }

    @Test
    void theArmsCrossAsTheNamesTheCheckerSaysTheyAre() {
        String written = ProgramWriter.written(program);

        assertThat(written)
                .contains("\"selects\":[{\"tests\":\"held\"}],\"binding\":{\"stands\":\"payload\"")
                .contains("\"selects\":[{\"tests\":\"nothing\"}],\"binding\":{\"stands\":\"selected\"");
    }

    @Test
    void whatAPresentValueHoldsIsNamedAndAnAbsentOneIsHandedOnAsItself() throws Exception {
        assertThat(counted(0)).isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(8)));
        assertThat(counted(1)).isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(-1)));
    }
}
