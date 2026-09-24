package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.abort.AbortKind;
import souther.compiler.meta.ModulePath;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedRow;
import souther.compiler.program.StandsIn;
import souther.nativecode.transport.ProgramWriter;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a behavior declares of its answer ({@code ensures}) is held where the checker placed the
 * check: where the behavior answers, for one whose body is here, and where the answer crosses into
 * this object's code, for one whose answer arrives from outside. An answer that keeps it is answered
 * and one that does not ends the run with {@code EnsuresNotHeld}.
 *
 * <p>Each ending is put beside a run of the same executable that answers, for the reason the ends
 * of an {@code Int} are: an ending on its own is the absence of an answer, and the run that answers
 * is what says the ending was the rule's.
 */
class AnAnswerIsHeldToWhatItsBehaviorDeclaresTest {

    private static final long MOST = Long.MAX_VALUE;

    private static final String HELD_WHERE_IT_ANSWERS = """
            module held exposing ( shrink, bounded, doubled, idOf )

            behavior shrink : (n: Int) -> Int
                ensures notAbove = value <= n

            let shrink (n) = if n > 100 then n + 1 else n - 1

            behavior bounded : (n: Int) -> Int
                ensures notAbove = value <= n
                ensures notFarBelow = value >= n - 10

            let bounded (n) = if n < 0 then n - 20 else n - 1

            behavior doubled : (n: Int) -> Int
                ensures atLeast = value * 2 >= n

            let doubled (n) = n

            data Found = { id: Int }
            data Missing

            behavior find : (id: Int) -> Found | Missing
                ensures Found -> value.id == id

            let find (id) = if id > 0 then Found { id = id }
                else if id == 0 then Missing
                else Found { id = 0 - id }

            behavior idOf : (id: Int) -> Int
            let idOf (id) = match find(id) with
                | Found as f -> f.id
                | Missing -> 0
            """;

    @Test
    void anAnswerThatKeepsWhatItsBehaviorDeclaresIsAnsweredAndOneThatDoesNotEndsTheRun()
            throws Exception {
        assertEndsButItsNeighbourAnswers(HELD_WHERE_IT_ANSWERS, "shrink", 200L, 5L, 4L);
    }

    /**
     * Every rule is held, and not the first that holds: {@code notAbove} holds of nine answered for
     * ten and of twenty-five below nought answered for five below it, and {@code notFarBelow} holds
     * only of the first.
     */
    @Test
    void everyRuleIsHeldAndNotOnlyTheFirst() throws Exception {
        assertEndsButItsNeighbourAnswers(HELD_WHERE_IT_ANSWERS, "bounded", -5L, 10L, 9L);
    }

    /**
     * A rule that leaves an {@code Int}'s range did not answer false. It did not answer, and the run
     * ends for that reason and not because the rule failed.
     */
    @Test
    void aRuleThatEndsWithoutAnAnswerEndsTheRunForItsOwnReason() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(HELD_WHERE_IT_ANSWERS));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior doubled = named(module, "doubled");

        assertThat(running.answeredOrEnded(module, doubled, given(MOST)))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(running.answeredOrEnded(module, doubled, given(5L)))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(5)));
    }

    /**
     * A rule over a case holds only of an answer that is that case, and another answer passes it by.
     * The behavior is reached through a call from another body, which is one more way in and the
     * same place the answer is held.
     */
    @Test
    void aRuleOverACaseIsHeldOnlyWhereTheAnswerIsThatCase() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(HELD_WHERE_IT_ANSWERS));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior idOf = named(module, "idOf");

        assertThat(running.answeredOrEnded(module, idOf, given(-3L)))
                .as("an id found under another id")
                .isEqualTo(new RunOutcome.Aborted(AbortKind.ENSURES_NOT_HELD));
        assertThat(running.answeredOrEnded(module, idOf, given(3L)))
                .as("an id found under itself")
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(3)));
        assertThat(running.answeredOrEnded(module, idOf, given(0L)))
                .as("nothing found, which the rule says nothing of")
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(0)));
    }

    private static final String HELD_WHERE_IT_CROSSES_IN = """
            module crossing exposing ( twice, looked : Int )

            behavior lookUp : (a: Int) -> Int
                ensures notBelow = value >= a

            behavior twice : (a: Int) -> Int
                depends on lookUp
            let twice (a, lookUp) = lookUp(a) * 2

            behavior doubled : (a: Int) -> Int
            let doubled (a) = a * 2

            behavior looked = lookUp >-> doubled

            fake lookUp
                | (1) -> 21
                | _ -> 0

            example twice
                | "what the dependency answered, doubled" : (1) -> 42
            """;

    /**
     * An answer supplied from outside is held as it crosses into a body that called for it. The
     * dependency answers nought for anything its table does not list, which keeps the rule for an
     * argument below nought and not for one above it.
     */
    @Test
    void anAnswerFromOutsideIsHeldWhereABodyCallsForIt() throws Exception {
        assertCrossingEndsButItsNeighbourAnswers("twice", 5L, -3L, 0L);
    }

    /** The same answer crossing in where a composition applies its stage. */
    @Test
    void anAnswerFromOutsideIsHeldWhereAStageAppliesIt() throws Exception {
        assertCrossingEndsButItsNeighbourAnswers("looked", 5L, -3L, 0L);
    }

    private static final String DECLARING_A_HELPER = """
            module lib.declaring exposing ( lookUp, Nat, Zero, Succ )

            data Zero
            data Succ = { prev: Nat }
            data Nat = Zero | Succ

            let depth (x: Nat): Int = match x with
                | Zero -> 0
                | Succ as s -> 1 + depth(s.prev)

            behavior lookUp : (a: Nat) -> Int
                ensures deepEnough = value >= depth(a)
            """;

    private static final String CALLING_IT = """
            module app.calling exposing ( twice )
            import lib.declaring ( lookUp, Nat, Zero, Succ )

            behavior twice : (n: Int) -> Int
                depends on lookUp
            let twice (n, lookUp) = lookUp(if n > 0 then Succ { prev = Zero } else Zero)

            fake lookUp
                | _ -> 0

            example twice
                | "nothing deep" : (0) -> 0
            """;

    /**
     * A rule is the module's that declares the behavior, and a helper it calls is that module's
     * copy, whichever module's code the answer crosses into.
     *
     * <p>The helper recurses, so the checker leaves the call standing rather than writing the
     * helper out into the rule, and the module whose body holds the answer to the rule carries no
     * copy of it. A rule resolved where the caller stands would reach nothing.
     */
    @Test
    void aRuleReachesTheHelpersOfTheModuleThatDeclaresIt() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DECLARING_A_HELPER, CALLING_IT));
        String written = ProgramWriter.written(program);
        assertThat(written)
                .as("the rule calls the helper rather than holding it written out")
                .contains("\"reaches\":{\"is\":\"helper\",\"declared\":\"lib.declaring.depth\"}");
        assertThat(written)
                .as("only the declaring module holds a copy of it")
                .contains("{\"name\":\"lib.declaring\",\"publishes\":[\"lib.declaring.Zero\"")
                .doesNotContain("\"declared\":\"app.calling.depth\"");
        CheckedModule calling = program.modules().stream()
                .filter(it -> it.name().equals("app.calling"))
                .findFirst()
                .orElseThrow();
        assertThat(calling.helpers()).noneMatch(it -> it.declares().toString().contains("depth"));
        Running running = Running.of(program);
        CheckedBehavior twice = named(calling, "twice");
        List<StandsIn> standIns = switch (twice.rows().getFirst().statement()) {
            case CheckedRow.WithStandIns it -> it.standsIn();
            case CheckedRow.SelfContained it -> throw new AssertionError("no stand-in: " + it);
            case CheckedRow.AnswerOwed it -> throw new AssertionError("no stand-in: " + it);
            case CheckedRow.NotReproducible it -> throw new AssertionError("not run: " + it.why());
        };

        assertThat(running.answeredOrEnded(calling, twice, given(1L), standIns))
                .as("nought answered for something one deep")
                .isEqualTo(new RunOutcome.Aborted(AbortKind.ENSURES_NOT_HELD));
        assertThat(running.answeredOrEnded(calling, twice, given(0L), standIns))
                .as("nought answered for nothing deep")
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(0)));
    }

    private static final String DECLARED_ELSEWHERE = """
            module lib.held exposing ( shrink )

            behavior shrink : (n: Int) -> Int
                ensures notAbove = value <= n

            let shrink (n) = if n > 100 then n + 1 else n - 1
            """;

    /**
     * Another build's behavior is held by that build's object, where it answers, and a call from
     * here reaches the place it is held. Nothing here decides what is done about its rule, and
     * nothing here runs it a second time.
     */
    @Test
    void anotherBuildsBehaviorIsHeldWhereThatBuildAnswersIt() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module app.uses exposing ( shrunk )
                import lib.held ( shrink )

                behavior shrunk : (n: Int) -> Int
                let shrunk (n) = shrink(n)
                """), ModulePath.of(Compiler.compile(DECLARED_ELSEWHERE)));
        byte[] before = NativeArtifacts.object(CheckedProgram.of(List.of(DECLARED_ELSEWHERE)));

        assertThat(ProgramWriter.written(program)).contains(
                "\"module\":\"lib.held\",\"name\":\"shrink\",\"is\":\"elsewhere\"");
        assertThat(ProgramWriter.written(program)).contains("\"ensures\":{\"at\":\"undecided\"}");
        Running running = Running.of(program, List.of(before));
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior shrunk = named(module, "shrunk");

        assertThat(running.answeredOrEnded(module, shrunk, given(200L)))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.ENSURES_NOT_HELD));
        assertThat(running.answeredOrEnded(module, shrunk, given(5L)))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(4)));
    }

    /**
     * Rows over behaviors that declare what their answers owe hold natively, the way every other row
     * does, where the answer is held at the callee and where it crosses in.
     */
    @Test
    void everyRowOverABehaviorThatDeclaresWhatItsAnswerOwesHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds("""
                module rows

                behavior shrink : (n: Int) -> Int
                    ensures notAbove = value <= n

                let shrink (n) = n - 1

                data Found = { id: Int }
                data Missing

                behavior find : (id: Int) -> Found | Missing
                    ensures Found -> value.id == id

                let find (id) = if id > 0 then Found { id = id } else Missing

                behavior idOf : (id: Int) -> Int
                let idOf (id) = match find(id) with
                    | Found as f -> f.id
                    | Missing -> 0

                example shrink
                    | "one less" : (5) -> 4

                example idOf
                    | "found" : (3) -> 3
                    | "missing" : (0) -> 0
                """, HELD_WHERE_IT_CROSSES_IN);
    }

    private static void assertEndsButItsNeighbourAnswers(String source, String behavior, long ends,
                                                         long answers, long answered)
            throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(source));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior reached = named(module, behavior);

        assertThat(running.answeredOrEnded(module, reached, given(ends)))
                .as("%s handed %s", behavior, ends)
                .isEqualTo(new RunOutcome.Aborted(AbortKind.ENSURES_NOT_HELD));
        assertThat(running.answeredOrEnded(module, reached, given(answers)))
                .as("%s handed %s, which is the control the ending above is read against",
                        behavior, answers)
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(answered)));
    }

    /**
     * The dependency is answered by what the row's table says of it, which is the only definition
     * of it this program has; the behavior run is then handed an argument the row does not state.
     */
    private static void assertCrossingEndsButItsNeighbourAnswers(String behavior, long ends,
                                                                 long answers, long answered)
            throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(HELD_WHERE_IT_CROSSES_IN));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior reached = named(module, behavior);
        List<StandsIn> standIns = switch (named(module, "twice").rows().getFirst().statement()) {
            case CheckedRow.WithStandIns it -> it.standsIn();
            case CheckedRow.SelfContained it -> throw new AssertionError("no stand-in: " + it);
            case CheckedRow.AnswerOwed it -> throw new AssertionError("no stand-in: " + it);
            case CheckedRow.NotReproducible it -> throw new AssertionError("not run: " + it.why());
        };

        assertThat(running.answeredOrEnded(module, reached, given(ends), standIns))
                .as("%s handed %s", behavior, ends)
                .isEqualTo(new RunOutcome.Aborted(AbortKind.ENSURES_NOT_HELD));
        assertThat(running.answeredOrEnded(module, reached, given(answers), standIns))
                .as("%s handed %s, which is the control the ending above is read against",
                        behavior, answers)
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(answered)));
    }

    private static CheckedBehavior named(CheckedModule module, String behavior) {
        return module.behaviors().stream()
                .filter(it -> it.name().name().equals(behavior))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + behavior));
    }

    private static List<ObservedValue> given(long value) {
        return List.of(new ObservedValue.Integer(value));
    }
}
