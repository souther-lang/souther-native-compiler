package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.Verdict;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedRow;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A behavior is called with a capability for each behavior it requires, which it was constructed
 * with, and a call through one reaches whatever the capability holds: a body, a host's
 * implementation, or what a row states. Nothing recovers what answers a requirement from the
 * behavior's name (#72).
 *
 * <p>So what a row stands in with reaches the requirement wherever the body calls it: inside a
 * function value the body makes, however deep; in each stage of a composition, which is handed the
 * capabilities of what it requires out of the composition's own; and in place of a behavior with a
 * body of its own, which a row may stand in for as it may for one a host implements.
 */
class ABehaviorIsCalledWithWhatItWasConstructedWithTest {

    private static final String CONSTRUCTED = """
            module constructed

            behavior lookUp : (a: Int) -> Int

            behavior other : (a: Int) -> Int

            let applied (f: (Int) -> Int, x: Int) = f(x)

            behavior viaClosure : (a: Int) -> Int
                depends on lookUp
            let viaClosure (a, lookUp) = applied((x) -> lookUp(x) + 1, a)

            behavior viaNested : (a: Int) -> Int
                depends on lookUp
            let viaNested (a, lookUp) = applied((x) -> applied((y) -> lookUp(y) * 10, x), a)

            behavior first : (a: Int) -> Int
                depends on lookUp
            let first (a, lookUp) = lookUp(a) + 1

            behavior second : (a: Int) -> Int
                depends on lookUp, other
            let second (a, lookUp, other) = lookUp(a) * other(a)

            behavior staged = first >-> second

            behavior restaged = staged >-> first

            behavior quote : (a: Int) -> Int
                depends on lookUp
            let quote (a, lookUp) = lookUp(a) + 100

            behavior both : (a: Int) -> Int
                depends on quote, lookUp
            let both (a, quote, lookUp) = quote(a) + lookUp(a)

            fake lookUp
                | (1) -> 21
                | (22) -> 3
                | _ -> 0

            fake other
                | (22) -> 2
                | _ -> 5

            fake quote
                | (1) -> 7
                | _ -> 0

            example viaClosure
                | "through a function value the body made" : (1) -> 22

            example viaNested
                | "through one made inside another" : (1) -> 210

            example staged
                | "each stage handed what it requires, one of them shared" : (1) -> 6

            example restaged
                | "a composition staged inside another" : (1) -> 1

            example quote
                | "a body calling what it requires" : (1) -> 121

            example both
                | "a behavior with a body stood in for, beside what it requires itself" : (1) -> 28
            """;

    @Test
    void whatARowStandsInWithIsReachedWhereverTheBodyCallsIt() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(CONSTRUCTED);
    }

    private static final String STANDING = """
            module standing

            data Found = { id: Int }

            data Missing

            behavior find : (id: Int) -> Found | Missing

            behavior named : (id: Int) -> Int
                depends on find
            let named (id, find) = match find(id) with
                | Found as found -> found.id
                | Missing -> 0

            fake find
                | (1) -> Found { id = 7 }
                | _ -> Missing

            example named
                | "a case of what the dependency answers" : (1) -> 7
                | "another, for the rest" : (2) -> 0
            """;

    /**
     * What a row's stand-in states is computed by the definitions the checker names for it, so a
     * case stated where the dependency answers a union stands there as the checker says it does.
     */
    @Test
    void aStandInStatingACaseOfWhatItAnswersStandsAsTheCheckerSays() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(STANDING);
    }

    private static final String PORT = """
            module lib.port exposing ( lookUp, looked )

            behavior lookUp : (a: Int) -> Int

            behavior looked : (a: Int) -> Int
                depends on lookUp
            let looked (a, lookUp) = lookUp(a)
            """;

    private static final String PIPED = """
            module app.piped exposing ( piped : Int )

            import lib.port ( looked )

            behavior doubled : (a: Int) -> Int
            let doubled (a) = a * 2

            behavior piped = looked >-> doubled

            fake lib.port.lookUp
                | (2) -> 40
                | _ -> 0

            example piped
                | "a stage another build implements, handed what it requires" : (2) -> 80
            """;

    /**
     * A stage another build implements is handed what it requires in the order that build
     * published it, picked out of what the composition was handed, as a stage of its own module is.
     * The row stands in for what the stage requires, and the stage, which that other build's object
     * runs, reaches it.
     */
    @Test
    void aStageAnotherBuildImplementsIsHandedWhatItRequires() throws Exception {
        Map<String, ClassFileImage> published = Compiler.compile(PORT);
        byte[] port = NativeArtifacts.object(Checked.of(List.of(PORT)));
        CheckedProgram piped = Checked.of(List.of(PIPED), ModulePath.of(published));
        CheckedModule module = piped.modules().getFirst();
        CheckedBehavior behavior = module.behaviors().stream()
                .filter(it -> it.name().name().equals("piped")).findFirst().orElseThrow();
        CheckedRow.WithStandIns row = (CheckedRow.WithStandIns) behavior.rows().getFirst().statement();

        ObservedValue answered = Running.of(piped, List.of(port))
                .rowAnswering(module, behavior, 0, List.of());

        assertThat(row.holds(answered)).as("answered %s", answered).isInstanceOf(Verdict.Held.class);
    }
}
