package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.Verdict;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedRow;
import souther.compiler.diag.CompileException;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * What a native run answers, held against what the program's own rows say.
 *
 * <p>The rows are the oracle and not this project's idea of one, and what that establishes is worth
 * stating exactly. A row arrives as {@link CheckedRow.SelfContained} only where the compile ran it,
 * and a row that ran whose answer did not keep it refuses the program — so for such a row the JVM
 * answered and the answer kept the row. Putting the native run to the same row therefore holds both
 * carriers to one statement without this backend writing down what either of them should say.
 *
 * <p>What it says nothing about is a row the compile did not run. Acceptance says so itself: a row
 * whose classes will not link observes nothing and does not refuse the program. Such a row arrives
 * as {@link CheckedRow.NotReproducible} carrying why, so the checked program does say which rows
 * those are — and a reader that filtered them out would quietly compare fewer rows than the program
 * states and stay green. Every arm is answered below for that reason.
 *
 * <p>Nor is this carriers compared against each other. Holding two of them to one statement is not
 * running both and comparing what came back.
 *
 * <p>A row is run through the entry the object carries for it, which is what the object does with
 * the row rather than something arranged out here: the values the row states were written into the
 * object when the program crossed. So what runs is the same whether the module publishes the
 * behavior or keeps it, and a corpus of kept names is not a corpus this test had to leave out.
 *
 * <p>Whether an answer is the one a row states is asked of the row. A test deciding that for itself
 * would be a second reading of what a row means, and the two carriers would then agree only as far
 * as this file agreed with the language.
 */
class ARowHoldsWhereverItIsRunTest {

    /**
     * A behavior the module publishes, so that what the object exports is among what runs here as
     * well as what it keeps. Every other corpus below writes no {@code exposing} clause, which is
     * a module that publishes nothing — and a row of a name nobody outside the module may reach is
     * run the same way as any other, through the entry the object carries for it.
     */
    private static final String ARITHMETIC = """
            module calculation exposing ( add )

            behavior add : (a: Int, b: Int) -> Int
            let add (a, b) = a + b

            example add
                | "two and forty" : (2, 40) -> 42
                | "one of them below nought" : (-5, 3) -> -2
                | "nothing and nothing" : (0, 0) -> 0
            """;

    private static final String COMPARING = """
            module comparing

            behavior less : (a: Int, b: Int) -> Int
            let less (a, b) = a - b

            behavior times : (a: Int, b: Int) -> Int
            let times (a, b) = a * b

            behavior atLeast : (a: Int, b: Int) -> Bool
            let atLeast (a, b) = a >= b

            behavior same : (a: Bool, b: Bool) -> Bool
            let same (a, b) = a == b

            behavior larger : (a: Int, b: Int) -> Int
            let larger (a, b) = if a > b then a else b

            example less
                | "what is left of it" : (40, 2) -> 38
                | "past nought" : (2, 40) -> -38

            example times
                | "twice" : (21, 2) -> 42
                | "by nothing" : (21, 0) -> 0
                | "signs" : (-6, 7) -> -42

            example atLeast
                | "above" : (2, 1) -> true
                | "level" : (1, 1) -> true
                | "below" : (0, 1) -> false

            example same
                | "both" : (true, true) -> true
                | "one of them" : (true, false) -> false

            example larger
                | "the left one" : (40, 2) -> 40
                | "the right one" : (2, 40) -> 40
                | "neither" : (7, 7) -> 7
            """;

    /**
     * A condition whose right side would abort at the values its left side exists to exclude.
     *
     * <p>Which operands run is part of what `&&` and `||` mean, so a row here is not about a
     * backend's arrangement: lowered eagerly, the multiplication leaves the range an `Int` holds
     * and the run ends where the language says it answers.
     */
    private static final String STOPPING = """
            module stopping

            behavior small : (a: Int) -> Bool
            let small (a) = a < 1000 && a * a < 1000000

            behavior anyAtAll : (a: Int) -> Bool
            let anyAtAll (a) = a > 0 || a * a > 0

            example small
                | "small enough to square" : (10) -> true
                | "too large to square at all" : (4000000000) -> false

            example anyAtAll
                | "settled by the left" : (4000000000) -> true
                | "the right one decides" : (-3) -> true
            """;

    private static final String NAMING = """
            module naming

            behavior flip : (a: Int) -> Int
            let flip (a) = -a

            behavior spread : (a: Int, b: Int) -> Int
            let spread (a, b) = {
                let low = if a < b then a else b
                let high = if a > b then a else b
                high - low
            }

            example flip
                | "above nought" : (7) -> -7
                | "below it" : (-7) -> 7
                | "nought itself" : (0) -> 0

            example spread
                | "the left one is larger" : (40, 2) -> 38
                | "the right one is" : (2, 40) -> 38
                | "neither" : (7, 7) -> 0
            """;

    /** A model's own types: built, read a field off, and forked on which case a value is. */
    private static final String SHAPES = """
            module shapes

            data Round = { across: Int }
            data Square = { side: Int }
            data Shape = Round | Square

            data Side = Int
            data Flat
            data Edged = { side: Side }
            data Face = Flat | Edged

            behavior edge : (size: Int, round: Bool) -> Int
            let edge (size, round) = {
                let shape: Shape = if round then Round { across = size } else Square { side = size }
                match shape with
                    | Round as r -> r.across * 2
                    | Square as q -> q.side * 4
            }

            // A type with nothing in it beside one that wraps a single value, so a fork tells
            // apart two values that are made of different amounts of nothing.
            behavior around : (size: Int, edged: Bool) -> Int
            let around (size, edged) = {
                let face: Face = if edged then Edged { side = Side(size) } else Flat
                match face with
                    | Flat -> 0
                    | Edged as e -> e.side.value * 3
            }

            example edge
                | "round" : (5, true) -> 10
                | "square" : (5, false) -> 20
                | "nothing of it" : (0, true) -> 0

            example around
                | "with an edge" : (5, true) -> 15
                | "with none" : (5, false) -> 0
            """;

    /** A value that may be absent, and several values carried as one. */
    private static final String HOLDING = """
            module holding

            data Held = { value: Int? }
            data Asked = { value: Bool? }

            behavior orElse : (a: Int, has: Bool) -> Int
            let orElse (a, has) = {
                let held = if has then Held { value = a } else Held { value = None }
                match held.value with
                    | Some v -> v
                    | None -> 0
            }

            // What it holds is narrower than a slot, so an arm reading it at the slot's width
            // would answer a number where the language says a truth.
            behavior settled : (a: Bool, has: Bool) -> Bool
            let settled (a, has) = {
                let held = if has then Asked { value = a } else Asked { value = None }
                match held.value with
                    | Some v -> v == true
                    | None -> false
            }

            behavior both : (a: Int, b: Int) -> Int
            let both (a, b) = {
                let pair = (a, b)
                let (one, other) = pair
                one * 100 + other
            }

            example orElse
                | "holding one" : (7, true) -> 7
                | "holding nothing" : (7, false) -> 0
                | "holding nought, which is not nothing" : (0, true) -> 0

            example both
                | "two of them" : (3, 4) -> 304
                | "nothing and something" : (0, 9) -> 9

            example settled
                | "holding a truth" : (true, true) -> true
                | "holding the other one" : (false, true) -> false
                | "holding nothing" : (true, false) -> false
            """;

    /** A definition the module holds and reaches, including one that reaches itself. */
    private static final String REACHING = """
            module reaching

            partial let countDown (n: Int): Int = if n <= 0 then 0 else n + countDown(n - 1)

            behavior total : (a: Int) -> Int
            let total (a) = countDown(a)

            example total
                | "three of them" : (3) -> 6
                | "none of them" : (0) -> 0
                | "one" : (1) -> 1
            """;

    /**
     * One published definition, held by two modules, reached by each.
     *
     * <p>A module carries every definition it reaches, so a copy each — and a call reaching the
     * other module's copy would be one module's answer standing for another's wherever the two
     * came to differ.
     */
    private static final String COUNTING = """
            module lib.counting exposing ( downTo )

            partial let downTo (n: Int): Int = if n <= 0 then 0 else n + downTo(n - 1)
            """;

    private static final String ONE = """
            module one
            import lib.counting ( downTo )

            behavior summed : (a: Int) -> Int
            let summed (a) = downTo(a)

            example summed
                | "three of them" : (3) -> 6
            """;

    private static final String TWO = """
            module two
            import lib.counting ( downTo )

            behavior doubled : (a: Int) -> Int
            let doubled (a) = downTo(a) * 2

            example doubled
                | "three of them, twice" : (3) -> 12
            """;

    /**
     * A behavior the object names and does not define, and one that reaches it.
     *
     * <p>What answers it is settled where the object is linked, so the row's stand-in is the
     * definition the linker was missing rather than something arranged around the run.
     */
    private static final String DEPENDING = """
            module depending

            behavior lookUp : (a: Int) -> Int

            behavior twice : (a: Int) -> Int
                depends on lookUp
            let twice (a, lookUp) = lookUp(a) * 2

            // A dependency asked one way and no other. What stands in for it states the arguments
            // of that one call, and there are none of them.
            behavior enabled : () -> Bool

            behavior allowed : (a: Int) -> Bool
                depends on enabled
            let allowed (a, enabled) = enabled() && a > 0

            fake lookUp
                | (1) -> 21
                | _ -> 0

            fake enabled
                | () -> true

            example twice
                | "what the dependency answered, doubled" : (1) -> 42
                | "what it answers for anything else" : (2) -> 0

            example allowed
                | "allowed and above nought" : (1) -> true
                | "allowed and not above nought" : (0) -> false
            """;

    @Test
    void everyRowOfEveryBehaviorHoldsWhenTheNativeObjectAnswersIt() throws Exception {
        assertEveryRowHolds(ARITHMETIC);
        assertEveryRowHolds(COMPARING);
        assertEveryRowHolds(STOPPING);
        assertEveryRowHolds(NAMING);
        assertEveryRowHolds(SHAPES);
        assertEveryRowHolds(HOLDING);
        assertEveryRowHolds(REACHING);
        assertEveryRowHolds(DEPENDING);
        assertEveryRowHolds(COUNTING, ONE, TWO);
    }

    /**
     * What makes the oracle an oracle. Were a row's answer merely what its author typed, a row
     * would say nothing about what the JVM does, and agreeing with one would be agreeing with the
     * person who wrote it.
     */
    @Test
    void aRowStatingWhatTheJvmDoesNotAnswerIsNotAcceptedAtAll() {
        assertThatThrownBy(() -> CheckedProgram.of(List.of("""
                module calculation

                behavior add : (a: Int, b: Int) -> Int
                let add (a, b) = a + b

                example add
                    | "what nothing answers" : (2, 40) -> 41
                """)))
                .isInstanceOf(CompileException.class)
                .hasMessageContaining("41");
    }

    /**
     * Runs every row the program states and asks the row whether the answer keeps it.
     *
     * <p>Every row, and every way a row can arrive. A corpus written to be run is one where each
     * row ran, so a row arriving any other way is this test's population having shrunk under it —
     * which is the one thing a count of what it did compare could never tell it. Said as a switch
     * with no arm standing for the rest, so a way of arriving added later has to be answered here.
     */
    static void assertEveryRowHolds(String... sources) throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(sources));
        int asked = 0;
        try (Running running = Running.of(program)) {
            for (CheckedModule module : program.modules()) {
                for (CheckedBehavior behavior : module.behaviors()) {
                    List<CheckedRow> rows = behavior.rows();
                    for (int at = 0; at < rows.size(); at++) {
                        CheckedRow row = rows.get(at);
                        String where = row.identity() + " of " + behavior.name();
                        switch (row.statement()) {
                            case CheckedRow.SelfContained states -> {
                                ObservedValue answered =
                                        running.rowAnswering(module, behavior, at, List.of());

                                assertThat(states.holds(answered))
                                        .as("%s answered %s", where, answered)
                                        .isInstanceOf(Verdict.Held.class);
                                asked++;
                            }
                            // A behavior that depends on another is run with what the row says
                            // that other one answers, which is the object's undefined symbol being
                            // given a definition rather than the run being arranged around it.
                            case CheckedRow.WithStandIns states -> {
                                ObservedValue answered = running.rowAnswering(
                                        module, behavior, at, states.standsIn());

                                assertThat(states.holds(answered))
                                        .as("%s answered %s", where, answered)
                                        .isInstanceOf(Verdict.Held.class);
                                asked++;
                            }
                            case CheckedRow.AnswerOwed states -> throw new AssertionError(
                                    where + " states no answer to hold anything to: " + states);
                            // The compile did not run it, and says why. Left out silently, this
                            // test would go on being green over fewer and fewer rows.
                            case CheckedRow.NotReproducible why -> throw new AssertionError(
                                    where + " was not run by the compile, so nothing about it was"
                                            + " observed on the JVM either: " + why.why());
                        }
                    }
                }
            }
        }
        assertThat(asked)
                .as("a program whose rows were never reached says nothing about either carrier")
                .isPositive();
    }
}
