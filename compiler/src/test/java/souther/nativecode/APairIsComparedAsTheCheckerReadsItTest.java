package souther.nativecode;

import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.util.Arrays;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Two values the checker lets meet are compared as what it reads them as, and ordered where the
 * language orders them.
 *
 * <p>A newtype beside a bare literal of what it wraps is compared by the value it wraps, and a case
 * beside its sum as a value of the sum. Every such pair is asked with the operands both ways round,
 * since which side the literal or the case stands on is what the author wrote and not what the
 * comparison means.
 *
 * <p>The units are declared in the reverse of the order the enumeration lists them, so the tokens
 * they carry are laid out the other way from where each case stands in the sum: an order read off
 * where the tokens are would answer every question here backwards.
 */
class APairIsComparedAsTheCheckerReadsItTest {

    private static final String SOURCE = """
            module comparing exposing ( atHundred, hundredAt, notHundred, under, atLeast, over, atMost,
                managerAtMost, skuUnder, amounts, stages, stagesNamed, isWon, wonIs, beforeWon,
                qualifiedBefore, openIs, sameStage, wonAtMost, closedBefore )

            data Lost
            data Won
            data Qualified
            data Prospecting
            data Open = Prospecting | Qualified
            data Stage = Open | Won | Lost
            data StageN = Stage
            data Amount = Int
            data Level = Int
            data Manager = Level
            data Sku = String

            let stage (n: Int): Stage =
                if n == 0 then Prospecting
                else if n == 1 then Qualified
                else if n == 2 then Won
                else Lost

            let open (n: Int): Open = if n == 0 then Prospecting else Qualified

            behavior atHundred : (a: Int) -> Bool
            let atHundred (a) = Amount(a) == 100

            behavior hundredAt : (a: Int) -> Bool
            let hundredAt (a) = 100 == Amount(a)

            behavior notHundred : (a: Int) -> Bool
            let notHundred (a) = 100 /= Amount(a)

            behavior under : (a: Int) -> Bool
            let under (a) = Amount(a) < 100

            behavior atLeast : (a: Int) -> Bool
            let atLeast (a) = 100 <= Amount(a)

            behavior over : (a: Int) -> Bool
            let over (a) = Amount(a) > 100

            behavior atMost : (a: Int) -> Bool
            let atMost (a) = 100 >= Amount(a)

            behavior managerAtMost : (a: Int) -> Bool
            let managerAtMost (a) = Manager(Level(a)) <= 5

            behavior skuUnder : (a: Int) -> Bool
            let skuUnder (a) = Sku(if a > 0 then "b" else "a") < "b"

            behavior amounts : (a: Int, b: Int) -> Bool
            let amounts (a, b) = Amount(a) < Amount(b)

            behavior stages : (a: Int, b: Int) -> Bool
            let stages (a, b) = stage(a) < stage(b)

            behavior stagesNamed : (a: Int, b: Int) -> Bool
            let stagesNamed (a, b) = StageN(stage(a)) >= StageN(stage(b))

            behavior isWon : (a: Int) -> Bool
            let isWon (a) = stage(a) == Won

            behavior wonIs : (a: Int) -> Bool
            let wonIs (a) = Won /= stage(a)

            behavior beforeWon : (a: Int) -> Bool
            let beforeWon (a) = stage(a) < Won

            behavior qualifiedBefore : (a: Int) -> Bool
            let qualifiedBefore (a) = Qualified < stage(a)

            behavior openIs : (a: Int, b: Int) -> Bool
            let openIs (a, b) = open(a) == stage(b)

            behavior sameStage : (a: Int) -> Bool
            let sameStage (a) = stage(a) == Qualified

            behavior wonAtMost : (a: Int) -> Bool
            let wonAtMost (a) = {
                let w: Won = Won
                w <= w && w >= w
            }

            behavior closedBefore : (a: Int, b: Int) -> Bool
            let closedBefore (a, b) = {
                let one: Won | Lost = if a == 0 then Won else Lost
                let other: Won | Lost = if b == 0 then Won else Lost
                one < other
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

    private static RunOutcome run(String name, long... given) throws Exception {
        CheckedBehavior behavior = module.behaviors().stream()
                .filter(it -> it.name().name().equals(name))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + name));
        List<ObservedValue> arguments = Arrays.stream(given)
                .<ObservedValue>mapToObj(ObservedValue.Integer::new)
                .toList();
        return running.answeredOrEnded(module, behavior, arguments);
    }

    private static RunOutcome answered(boolean value) {
        return new RunOutcome.Answered(new ObservedValue.Bool(value));
    }

    /** What crosses says the checker read the literal as the newtype, for this operator only. */
    @Test
    void theCheckerReadsTheLiteralAsTheNewtype() {
        assertThat(ProgramWriter.written(program))
                .contains("\"reading\":{\"is\":\"in\",\"type\":{\"ref\":{\"is\":\"declared\",\"declared\":\"comparing.Amount\"}}}");
    }

    @Test
    void aNewtypeIsEqualToALiteralOfWhatItWrapsOnEitherSide() throws Exception {
        assertThat(run("atHundred", 100)).isEqualTo(answered(true));
        assertThat(run("atHundred", 99)).isEqualTo(answered(false));
        assertThat(run("hundredAt", 100)).isEqualTo(answered(true));
        assertThat(run("hundredAt", 101)).isEqualTo(answered(false));
        assertThat(run("notHundred", 100)).isEqualTo(answered(false));
        assertThat(run("notHundred", 7)).isEqualTo(answered(true));
    }

    @Test
    void aNewtypeIsOrderedAgainstALiteralByWhatItWraps() throws Exception {
        assertThat(run("under", 99)).isEqualTo(answered(true));
        assertThat(run("under", 100)).isEqualTo(answered(false));
        assertThat(run("atLeast", 100)).isEqualTo(answered(true));
        assertThat(run("atLeast", 99)).isEqualTo(answered(false));
        assertThat(run("over", 101)).isEqualTo(answered(true));
        assertThat(run("over", 100)).isEqualTo(answered(false));
        assertThat(run("atMost", 100)).isEqualTo(answered(true));
        assertThat(run("atMost", 101)).isEqualTo(answered(false));
    }

    @Test
    void aNewtypeOverANewtypeIsOpenedAllTheWayDown() throws Exception {
        assertThat(run("managerAtMost", 5)).isEqualTo(answered(true));
        assertThat(run("managerAtMost", 6)).isEqualTo(answered(false));
    }

    @Test
    void aNewtypeOverTextIsOrderedAsText() throws Exception {
        assertThat(run("skuUnder", 0)).isEqualTo(answered(true));
        assertThat(run("skuUnder", 1)).isEqualTo(answered(false));
    }

    @Test
    void twoValuesOfANewtypeAreOrderedByWhatTheyWrap() throws Exception {
        assertThat(run("amounts", 1, 2)).isEqualTo(answered(true));
        assertThat(run("amounts", 2, 1)).isEqualTo(answered(false));
        assertThat(run("amounts", 2, 2)).isEqualTo(answered(false));
    }

    /**
     * Prospecting, then Qualified, then Won, then Lost: the leaves of the sum in the order they
     * are reached, Open's two before the cases that follow it.
     */
    @Test
    void anEnumerationIsOrderedByWhereItsCasesStandInItsDeclaration() throws Exception {
        for (int a = 0; a < 4; a++) {
            for (int b = 0; b < 4; b++) {
                assertThat(run("stages", a, b)).as("%d < %d", a, b).isEqualTo(answered(a < b));
                assertThat(run("stagesNamed", a, b)).as("%d >= %d", a, b).isEqualTo(answered(a >= b));
            }
        }
    }

    @Test
    void aSumIsEqualToOneOfItsCasesOnEitherSide() throws Exception {
        for (int a = 0; a < 4; a++) {
            assertThat(run("isWon", a)).as("%d", a).isEqualTo(answered(a == 2));
            assertThat(run("wonIs", a)).as("%d", a).isEqualTo(answered(a != 2));
            assertThat(run("sameStage", a)).as("%d", a).isEqualTo(answered(a == 1));
        }
    }

    @Test
    void aSumIsOrderedAgainstOneOfItsCasesByWhereTheCaseStands() throws Exception {
        for (int a = 0; a < 4; a++) {
            assertThat(run("beforeWon", a)).as("%d", a).isEqualTo(answered(a < 2));
            assertThat(run("qualifiedBefore", a)).as("%d", a).isEqualTo(answered(1 < a));
        }
    }

    /**
     * A case is ordered by the one enumeration listing it, and so is a union of cases: Won and
     * Lost are listed by Stage and by nothing else, which places Won before Lost.
     */
    @Test
    void aCaseAndAUnionOfCasesAreOrderedByTheEnumerationListingThem() throws Exception {
        assertThat(run("wonAtMost", 0)).isEqualTo(answered(true));
        for (int a = 0; a < 2; a++) {
            for (int b = 0; b < 2; b++) {
                assertThat(run("closedBefore", a, b)).as("%d < %d", a, b)
                        .isEqualTo(answered(a == 0 && b == 1));
            }
        }
    }

    @Test
    void aSumIsEqualToASumOfSomeOfItsCases() throws Exception {
        for (int a = 0; a < 2; a++) {
            for (int b = 0; b < 4; b++) {
                assertThat(run("openIs", a, b)).as("%d == %d", a, b).isEqualTo(answered(a == b));
            }
        }
    }
}
