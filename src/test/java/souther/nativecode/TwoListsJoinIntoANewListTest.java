package souther.nativecode;

import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * {@code ++} over two lists: a new list of the first's elements and then the second's, neither of
 * the two changed.
 */
class TwoListsJoinIntoANewListTest {

    private static final String SOURCE = """
            module joining exposing ( joined, leftEmpty, rightEmpty, bothEmpty, kept, compound )

            data A = { v: Int }

            behavior joined : (a: Int) -> Int
            let joined (a) = if [a, 2] ++ [3] == [a, 2, 3] then 1 else 0

            behavior leftEmpty : (a: Int) -> Int
            let leftEmpty (a) = {
                let empty: List<Int> = []
                if empty ++ [a, 2] == [a, 2] then 1 else 0
            }

            behavior rightEmpty : (a: Int) -> Int
            let rightEmpty (a) = {
                let empty: List<Int> = []
                if [a, 2] ++ empty == [a, 2] then 1 else 0
            }

            behavior bothEmpty : (a: Int) -> Int
            let bothEmpty (a) = {
                let empty: List<Int> = []
                List.length(empty ++ empty) + a
            }

            behavior kept : (a: Int) -> Int
            let kept (a) = {
                let first = [a, 2]
                let second = [3]
                let both = first ++ second
                List.length(first) * 100 + List.length(second) * 10 + List.length(both)
            }

            behavior compound : (a: Int) -> Int
            let compound (a) = match List.get(2, [A { v = 1 }] ++ [A { v = 2 }, A { v = a }]) with
                | Some it -> it.v
                | None -> -1
            """;

    private static Running running;
    private static CheckedModule module;

    @BeforeAll
    static void build() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SOURCE));
        running = Running.of(program);
        module = program.modules().getFirst();
    }

    private static RunOutcome run(String name, long argument) throws Exception {
        CheckedBehavior behavior = module.behaviors().stream()
                .filter(it -> it.name().name().equals(name))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + name));
        return running.answeredOrEnded(module, behavior,
                List.of(new ObservedValue.Integer(argument)));
    }

    private static RunOutcome answered(long value) {
        return new RunOutcome.Answered(new ObservedValue.Integer(value));
    }

    @Test
    void theJoinedListHoldsTheFirstsElementsAndThenTheSeconds() throws Exception {
        assertThat(run("joined", 1)).isEqualTo(answered(1));
    }

    @Test
    void anEmptyListOnEitherSideLeavesTheOther() throws Exception {
        assertThat(run("leftEmpty", 1)).isEqualTo(answered(1));
        assertThat(run("rightEmpty", 1)).isEqualTo(answered(1));
        assertThat(run("bothEmpty", 4)).isEqualTo(answered(4));
    }

    @Test
    void neitherListIsChangedByTheJoin() throws Exception {
        assertThat(run("kept", 1)).isEqualTo(answered(213));
    }

    /** An element is a slot whatever it holds, a value built from fields included. */
    @Test
    void aValueBuiltFromFieldsIsJoinedAsWhatItIs() throws Exception {
        assertThat(run("compound", 9)).isEqualTo(answered(9));
    }
}
