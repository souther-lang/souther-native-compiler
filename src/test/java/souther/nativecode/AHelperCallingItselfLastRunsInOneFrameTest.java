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
 * A helper whose last act is to call itself, run as a loop and not as a call.
 *
 * <p>What is asked is how deep a recursion can go and still answer, and a call that takes a frame
 * a step answers only as deep as the stack lets it. The depths here are ones a frame a step does
 * not reach: ten million steps, and a fold over a list of over a million elements, which is what
 * {@code List.fold} is written as (ADR-0051).
 */
class AHelperCallingItselfLastRunsInOneFrameTest {

    private static final String SOURCE = """
            module looping exposing ( counted, swapped, through, fold, notLast, settled, climbed )

            partial let count (n: Int, acc: Int): Int = if n == 0 then acc else count(n - 1, acc + 1)

            behavior counted : (n: Int) -> Int
            let counted (n) = count(n, 0)

            partial let swap (a: Int, b: Int, n: Int): Int =
                if n == 0 then a * 10 + b else swap(b, a, n - 1)

            behavior swapped : (n: Int) -> Int
            let swapped (n) = swap(1, 2, n)

            partial let walk (n: Int, acc: Int): Int = {
                let less = n - 1
                match List.get(0, [n]) with
                    | Some m -> if m == 0 then acc else walk(less, acc + 2)
                    | None -> acc
            }

            behavior through : (n: Int) -> Int
            let through (n) = walk(n, 0)

            partial let doubled (xs: List<Int>, times: Int): List<Int> =
                if times == 0 then xs else doubled(xs ++ xs, times - 1)

            behavior fold : (n: Int) -> Int
            let fold (n) = List.fold((acc, x) -> acc + x, 0, doubled([n], 20))

            partial let depth (n: Int): Int = if n == 0 then 0 else 1 + depth(n - 1)

            data Positive = { n: Int }
                invariant above = n >= 1

            partial let settle (n: Int, acc: Int): Int = {
                guard Positive { n = n } as p
                    else | above -> acc
                settle(p.n - 1, acc + 1)
            }

            behavior settled : (n: Int) -> Int
            let settled (n) = settle(n, 0)

            partial let climb (n: Int, acc: Int): Int = {
                guard Positive { n = n } as p
                    else | above -> climb(n + 1, acc + 1)
                acc
            }

            behavior climbed : (n: Int) -> Int
            let climbed (n) = climb(1 - n, 0)

            behavior notLast : (n: Int) -> Int
            let notLast (n) = depth(n)
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
    void aRecursionTenMillionDeepAnswers() throws Exception {
        assertThat(run("counted", 10_000_000)).isEqualTo(answered(10_000_000));
    }

    /**
     * Every argument is worked out before any parameter takes its new value, so a call handing its
     * parameters round reads each one as it was.
     */
    @Test
    void aCallSwappingTwoParametersAnswersWhatTheSourceSays() throws Exception {
        assertThat(run("swapped", 0)).isEqualTo(answered(12));
        assertThat(run("swapped", 1)).isEqualTo(answered(21));
        assertThat(run("swapped", 2)).isEqualTo(answered(12));
        assertThat(run("swapped", 1_000_001)).isEqualTo(answered(21));
    }

    /** The body of a {@code let}, an arm of a {@code match} and a branch of an {@code if} it
     *  answers from are all where the helper answers. */
    @Test
    void aCallUnderALetAMatchAndAnIfIsStillLast() throws Exception {
        assertThat(run("through", 5_000_000)).isEqualTo(answered(10_000_000));
    }

    /**
     * An attempted construction answers what the branch it takes answers, so a call to itself
     * where the value was built, or in the arm answering a clause that did not hold, is last too.
     */
    @Test
    void aCallInEitherBranchOfAnAttemptedConstructionIsStillLast() throws Exception {
        assertThat(run("settled", 10_000_000)).isEqualTo(answered(10_000_000));
        assertThat(run("climbed", 10_000_000)).isEqualTo(answered(10_000_000));
    }

    /** A fold over a list of 2^20 elements, each the number handed in. */
    @Test
    void aFoldOverAListOfOverAMillionElementsAnswers() throws Exception {
        assertThat(run("fold", 3)).isEqualTo(answered(3L << 20));
    }

    /** A call whose answer is still to be used is a call, and answers as one. */
    @Test
    void aCallThatIsNotLastIsStillACall() throws Exception {
        assertThat(run("notLast", 1_000)).isEqualTo(answered(1_000));
    }
}
