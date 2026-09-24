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
 * A list built inside a run, and read back through the two kernels the language reads one with:
 * {@code List.length} and {@code List.get}.
 *
 * <p>{@code List.fold}, and the combinators the standard library writes over it, are Souther over
 * {@code List.get} (ADR-0051) and not kernels, so what is asked here is how a list is laid out
 * and read, and nothing the standard library builds on it. What a
 * behavior answers is an {@code Int}, so the list never leaves the run and what is asked is how it
 * is laid out and read, not how it is written out.
 */
class AListIsItsLengthAndItsElementsTest {

    private static final String SOURCE = """
            module listing exposing ( counted, none, at, lined, summed, widened )

            data Order = { number: Int, lines: List<Int> }

            data A = { v: Int }
            data B = { v: Int }
            data S = A | B

            behavior counted : (a: Int) -> Int
            let counted (a) = List.length([a, a + 1, a + 2])

            behavior none : (a: Int) -> Int
            let none (a) = {
                let empty: List<Int> = []
                List.length(empty) + a
            }

            behavior at : (index: Int) -> Int
            let at (index) = match List.get(index, [10, 20, 30]) with
                | Some x -> x
                | None -> -1

            behavior lined : (a: Int) -> Int
            let lined (a) = {
                let order = Order { number = a, lines = [a, a * 2] }
                List.length(order.lines) * 100 + order.number
            }

            partial let sumFrom (step: (Int, Int) -> Int, acc: Int, xs: List<Int>, i: Int): Int =
                match List.get(i, xs) with
                    | Some x -> sumFrom(step, step(acc, x), xs, i + 1)
                    | None -> acc

            behavior summed : (a: Int) -> Int
            let summed (a) = sumFrom((acc, x) -> acc + x, 0, [a, 2, 3], 0)

            let asCases (xs: List<A>): List<S> = xs

            behavior widened : (a: Int) -> Int
            let widened (a) = List.length(asCases([A { v = a }, A { v = a }]))
            """;

    private static CheckedProgram program;
    private static Running running;
    private static CheckedModule module;

    @BeforeAll
    static void build() throws Exception {
        program = CheckedProgram.of(List.of(SOURCE));
        running = Running.of(program);
        module = program.modules().getFirst();
    }

    private static CheckedBehavior behavior(String name) {
        return module.behaviors().stream()
                .filter(it -> it.name().name().equals(name))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + name));
    }

    private static RunOutcome run(String name, long argument) throws Exception {
        return running.answeredOrEnded(module, behavior(name),
                List.of(new ObservedValue.Integer(argument)));
    }

    private static RunOutcome answered(long value) {
        return new RunOutcome.Answered(new ObservedValue.Integer(value));
    }

    @Test
    void aListHoldsAsManyElementsAsItWasWrittenWith() throws Exception {
        assertThat(run("counted", 5)).isEqualTo(answered(3));
    }

    @Test
    void theEmptyListHoldsNone() throws Exception {
        assertThat(run("none", 7)).isEqualTo(answered(7));
    }

    @Test
    void anElementIsReadAtItsIndex() throws Exception {
        assertThat(run("at", 0)).isEqualTo(answered(10));
        assertThat(run("at", 2)).isEqualTo(answered(30));
    }

    /** Past the end and before the start alike are outside the list, and neither ends the run. */
    @Test
    void anIndexOutsideTheListReadsNothing() throws Exception {
        assertThat(run("at", 3)).isEqualTo(answered(-1));
        assertThat(run("at", -1)).isEqualTo(answered(-1));
        assertThat(run("at", Long.MIN_VALUE)).isEqualTo(answered(-1));
    }

    @Test
    void aListIsAFieldLikeAnyOther() throws Exception {
        assertThat(run("lined", 4)).isEqualTo(answered(204));
    }

    /**
     * A helper walking a list the way {@code foldFrom} does: by index, through {@code List.get},
     * applying a function value it was handed to each element.
     */
    @Test
    void aHelperWalksAListByIndex() throws Exception {
        assertThat(run("summed", 1)).isEqualTo(answered(6));
    }

    /** A list of a case stands where a list of its sum is taken, as the checker lets it. */
    @Test
    void aListOfACaseStandsAsAListOfItsSum() throws Exception {
        assertThat(run("widened", 1)).isEqualTo(answered(2));
    }
}
