package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A program asked many questions is written out, handed to a harness and linked once, however the
 * questions are asked.
 *
 * <p>What is kept was always found again, so a test asking the same program thousands of questions
 * answered every one of them correctly while it wrote the program out and generated its harness for
 * each: the temporal oracle spent fourteen seconds of CI on it (#133). Nothing a question answers
 * says so, and a count of links does not either, since the link was found each time. So the
 * preparing is counted here, and asked through a Running made for every question, which is how
 * the helpers that make one per question ask.
 */
class AProgramIsPreparedOnceHoweverOftenItIsAskedTest {

    private static final String SOURCE = """
            module prepared exposing ( doubled, lessened )

            behavior doubled : (n: Int) -> Int
            let doubled (n) = n * 2

            behavior lessened : (n: Int) -> Int
            let lessened (n) = n - 1
            """;

    @Test
    void aProgramIsWrittenGeneratedForAndLinkedOnceForEveryQuestionAskedOfIt() throws Exception {
        CheckedProgram program = Checked.of(List.of(SOURCE));
        CheckedModule module = program.modules().getFirst();

        for (int n = 0; n < 50; n++) {
            for (String name : List.of("doubled", "lessened")) {
                CheckedBehavior behavior = module.behaviors().stream()
                        .filter(it -> it.name().name().equals(name))
                        .findFirst()
                        .orElseThrow();
                long expected = name.equals("doubled") ? n * 2L : n - 1L;
                assertThat(Running.of(program).answeredOrEnded(module, behavior,
                        List.of(new ObservedValue.Integer(n))))
                        .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(expected)));
            }
        }

        assertThat(NativeArtifacts.writingsOf(program)).as("writings").isEqualTo(1);
        assertThat(Running.harnessesOf(program)).as("harnesses").isEqualTo(1);
        assertThat(NativeArtifacts.compilationsOf(program)).as("compilations").isEqualTo(1);
        assertThat(NativeArtifacts.linksOf(program)).as("links").isEqualTo(1);
    }
}
