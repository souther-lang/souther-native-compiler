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
 * {@code List.map}, {@code List.filter} and the other folds that only grow a list, which the
 * checker's compiler rewrites into a walk that builds the list ({@code List.$build}) and adds to it
 * ({@code List.$grow}).
 */
class AListAWalkGrowsIsBuiltOnceTest {

    private static final String SOURCE = """
            module growing exposing (
                mapped, filtered, emptyTyped, emptyLiteral, filteredThenMapped, closing,
                pastTheFirstRoom, flattened, truths, values, neverASet, neverACopy, dropped
            )

            data A = { v: Int }

            behavior mapped : (a: Int) -> Int
            let mapped (a) = {
                let ys = List.map((x) -> x + a, [1, 2, 3])
                match List.get(2, ys) with
                    | Some y -> List.length(ys) * 100 + y
                    | None -> -1
            }

            behavior filtered : (a: Int) -> Int
            let filtered (a) = {
                let ys = List.filter((x) -> x > a, [1, 5, 2, 6, 3])
                match List.get(1, ys) with
                    | Some y -> List.length(ys) * 100 + y
                    | None -> -1
            }

            behavior emptyTyped : (a: Int) -> Int
            let emptyTyped (a) = {
                let none: List<Int> = []
                List.length(List.map((x) -> x + a, none))
            }

            behavior emptyLiteral : (a: Int) -> Int
            let emptyLiteral (a) = List.length(List.map((x) -> a, [])) + a

            behavior neverASet : (a: Int) -> Int
            let neverASet (a) = List.length(List.map((x) -> Set.singleton(a), [])) + a

            behavior neverACopy : (a: Int) -> Int
            let neverACopy (a) = List.length(List.map((x) -> List.drop(1, [a, a]), [])) + a

            behavior dropped : (a: Int) -> Int
            let dropped (a) = List.length(List.drop(a, [1, 2, 3]))

            behavior filteredThenMapped : (a: Int) -> Int
            let filteredThenMapped (a) = {
                let ys = List.map((x) -> x * 10, List.filter((x) -> x > a, [1, 2, 3, 4]))
                match List.get(0, ys) with
                    | Some y -> List.length(ys) * 1000 + y
                    | None -> -1
            }

            behavior closing : (a: Int) -> Int
            let closing (a) = {
                let k = a * 3
                let ys = List.map((x) -> {
                    let f = if x > a then (y) -> y + k else (y) -> y * x
                    f(x)
                }, [1, 2, 3])
                match List.get(0, ys) with
                    | Some first -> match List.get(2, ys) with
                        | Some last -> first * 1000 + last
                        | None -> -1
                    | None -> -1
            }

            behavior pastTheFirstRoom : (a: Int) -> Int
            let pastTheFirstRoom (a) = {
                let ys = List.map((x) -> x + a,
                    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20])
                match List.get(19, ys) with
                    | Some y -> List.length(ys) * 100 + y
                    | None -> -1
            }

            behavior flattened : (a: Int) -> Int
            let flattened (a) = {
                let ys = List.flatMap((x) -> [x, x + a, x * a], [1, 2, 3, 4, 5, 6, 7])
                match List.get(20, ys) with
                    | Some y -> List.length(ys) * 100 + y
                    | None -> -1
            }

            behavior truths : (a: Int) -> Int
            let truths (a) = match List.get(1, List.map((x) -> x > a, [1, 2, 3])) with
                | Some it -> if it then 1 else 0
                | None -> -1

            behavior values : (a: Int) -> Int
            let values (a) = match List.get(1, List.map((x) -> A { v = x * a }, [1, 2, 3])) with
                | Some it -> it.v
                | None -> -1
            """;

    private static Running running;
    private static CheckedModule module;

    @BeforeAll
    static void build() throws Exception {
        CheckedProgram program = Checked.of(List.of(SOURCE));
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
    void mapAnswersEachElementAsTheFunctionDoes() throws Exception {
        assertThat(run("mapped", 10)).isEqualTo(answered(313));
    }

    @Test
    void filterKeepsWhatHoldsInItsOrder() throws Exception {
        assertThat(run("filtered", 2)).isEqualTo(answered(306));
    }

    @Test
    void anEmptyListWalksToAnEmptyList() throws Exception {
        assertThat(run("emptyTyped", 1)).isEqualTo(answered(0));
        assertThat(run("emptyLiteral", 4)).isEqualTo(answered(4));
    }

    /**
     * The step of a walk over an empty list literal never runs, so nothing in it is asked of this
     * backend: not a set, which nothing here lays out, and not a call of a helper whose copy is
     * then never made.
     */
    @Test
    void whatAStepThatNeverRunsWouldDoIsNotAskedFor() throws Exception {
        assertThat(run("neverASet", 4)).isEqualTo(answered(4));
        assertThat(run("neverACopy", 5)).isEqualTo(answered(5));
    }

    /**
     * A fold seeded with {@code []} that nothing rewrites into a walk hands its step the
     * accumulator at the type the fold settles, and runs: {@code List.drop} is one.
     */
    @Test
    void aFoldSeededWithAnEmptyListItDoesNotGrowRuns() throws Exception {
        assertThat(run("dropped", 0)).isEqualTo(answered(3));
        assertThat(run("dropped", 2)).isEqualTo(answered(1));
        assertThat(run("dropped", 5)).isEqualTo(answered(0));
    }

    @Test
    void aFilterThenAMapIsOneWalk() throws Exception {
        assertThat(run("filteredThenMapped", 2)).isEqualTo(answered(2030));
    }

    @Test
    void aFunctionValueInsideAStepClosesOverTheStepsElement() throws Exception {
        assertThat(run("closing", 1)).isEqualTo(answered(1006));
    }

    @Test
    void aListGrownPastItsFirstRoomKeepsEveryElement() throws Exception {
        assertThat(run("pastTheFirstRoom", 100)).isEqualTo(answered(2120));
    }

    @Test
    void aListAddedWholeIsAddedElementByElement() throws Exception {
        assertThat(run("flattened", 3)).isEqualTo(answered(2121));
    }

    @Test
    void anElementIsASlotWhateverItHolds() throws Exception {
        assertThat(run("truths", 1)).isEqualTo(answered(1));
        assertThat(run("values", 7)).isEqualTo(answered(14));
    }
}
