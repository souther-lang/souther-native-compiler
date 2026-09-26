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
 * A recursive helper whose body leaves type variables open, run at the types each call hands it.
 *
 * <p>The standard library's {@code foldFrom} is the first one a program reaches: {@code List.fold}
 * is a call of it (ADR-0051), and it is written over {@code 'acc} and {@code 'a}. What is lowered is
 * a copy of it for each set of types a call needs, so a fold into a number and a fold into text in
 * one program are two functions, and two folds into a number are one.
 */
class AHelperOverTypeVariablesRunsAtTheTypesItIsCalledAtTest {

    private static final String SOURCE = """
            module folding exposing ( summed, again, spelt, any, both )

            behavior summed : (a: Int) -> Int
            let summed (a) = List.fold((acc, x) -> acc + x, 0, [a, 2, 3])

            behavior again : (a: Int) -> Int
            let again (a) = List.fold((acc, x) -> acc + x * 2, a, [1, 2])

            behavior spelt : (a: Int) -> Int
            let spelt (a) = String.length(List.fold((acc, x) -> acc ++ "ab", "", [a, 2, 3]))

            behavior any : (a: Int) -> Int
            let any (a) = if List.any((x) -> x > 2, [1, a]) then 1 else 0

            behavior both : (a: Int) -> Int
            let both (a) =
                String.length(List.fold((acc, x) -> acc ++ "c", "", [a]))
                    + List.fold((acc, x) -> acc + x, 0, [a, a])
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
    void aFoldAnswersWhatItsStepMadeOfEveryElement() throws Exception {
        assertThat(run("summed", 1)).isEqualTo(answered(6));
    }

    /** A second fold at the same types reaches the same copy, and answers for its own step. */
    @Test
    void aSecondFoldAtTheSameTypesAnswersForItsOwnStep() throws Exception {
        assertThat(run("again", 4)).isEqualTo(answered(10));
    }

    /** A fold into text is a copy of its own, beside the fold into a number. */
    @Test
    void aFoldIntoTextIsACopyOfItsOwn() throws Exception {
        assertThat(run("spelt", 1)).isEqualTo(answered(6));
    }

    /** A fold into a truth, which is what {@code List.any} is written as. */
    @Test
    void aFoldIntoATruthIsACopyOfItsOwn() throws Exception {
        assertThat(run("any", 3)).isEqualTo(answered(1));
        assertThat(run("any", 2)).isEqualTo(answered(0));
    }

    /** A fold into text and one into a number in one body, each reaching its own copy. */
    @Test
    void twoFoldsAtTwoTypesInOneBodyEachAnswerForTheirOwn() throws Exception {
        assertThat(run("both", 7)).isEqualTo(answered(15));
    }
}
