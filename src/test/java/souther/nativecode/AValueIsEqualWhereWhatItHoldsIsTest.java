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
 * Two values of one type are equal where what they are made of is, and not where they are kept.
 *
 * <p>Every value here is built twice, apart, inside the run, so the two are always at different
 * addresses: a comparison of the addresses would answer false to every one of them. What each
 * behavior is handed says whether the second is built the same as the first or differs in one
 * place, and each kind of value is asked both ways.
 */
class AValueIsEqualWhereWhatItHoldsIsTest {

    private static final String SOURCE = """
            module equality exposing ( lines, cart, cased, staged, held, paired, nested, unlike )

            data Sku = String
            data OrderLine = { sku: Sku, quantity: Int }
            data Circle = { radius: Int }
            data Square = { side: Int }
            data Shape = Circle | Square
            data Tree = { value: Int, children: List<Tree> }
            data Prospecting
            data Won
            data Stage = Prospecting | Won

            let line (n: Int): OrderLine = OrderLine { sku = Sku("s"), quantity = n }

            behavior lines : (a: Int, b: Int) -> Bool
            let lines (a, b) = [line(1), line(a)] == [line(1), line(b)]

            behavior cart : (a: Int, b: Int) -> Bool
            let cart (a, b) = List.length([line(1)]) == 1 && [line(a)] == [line(a), line(b)]

            behavior cased : (a: Int, b: Int) -> Bool
            let cased (a, b) = {
                let one: Shape = if a > 0 then Circle { radius = a } else Square { side = -a }
                let other: Shape = if b > 0 then Circle { radius = b } else Square { side = -b }
                one == other
            }

            behavior staged : (a: Int, b: Int) -> Bool
            let staged (a, b) = {
                let one: Stage = if a > 0 then Won else Prospecting
                let other: Stage = if b > 0 then Won else Prospecting
                one == other
            }

            behavior held : (a: Int, b: Int) -> Bool
            let held (a, b) = List.get(a, [10, 20]) == List.get(b, [10, 20])

            behavior paired : (a: Int, b: Int) -> Bool
            let paired (a, b) = (a, "x") == (b, "x")

            behavior nested : (a: Int, b: Int) -> Bool
            let nested (a, b) =
                Tree { value = 0, children = [Tree { value = a, children = [] }] }
                    == Tree { value = 0, children = [Tree { value = b, children = [] }] }

            behavior unlike : (a: Int, b: Int) -> Bool
            let unlike (a, b) = [line(a)] /= [line(b)]
            """;

    private static Running running;
    private static CheckedModule module;

    @BeforeAll
    static void build() throws Exception {
        CheckedProgram program = Checked.of(List.of(SOURCE));
        running = Running.of(program);
        module = program.modules().getFirst();
    }

    private static RunOutcome run(String name, long a, long b) throws Exception {
        CheckedBehavior behavior = module.behaviors().stream()
                .filter(it -> it.name().name().equals(name))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + name));
        return running.answeredOrEnded(module, behavior,
                List.of(new ObservedValue.Integer(a), new ObservedValue.Integer(b)));
    }

    private static RunOutcome answered(boolean value) {
        return new RunOutcome.Answered(new ObservedValue.Bool(value));
    }

    @Test
    void twoListsOfDeclaredValuesAreEqualElementByElement() throws Exception {
        assertThat(run("lines", 2, 2)).isEqualTo(answered(true));
        assertThat(run("lines", 2, 3)).isEqualTo(answered(false));
    }

    @Test
    void twoListsOfDifferentLengthsAreNotEqual() throws Exception {
        assertThat(run("cart", 1, 1)).isEqualTo(answered(false));
    }

    @Test
    void twoValuesOfASumAreEqualWhereTheyAreOneCaseHoldingTheSame() throws Exception {
        assertThat(run("cased", 3, 3)).isEqualTo(answered(true));
        assertThat(run("cased", -3, -3)).isEqualTo(answered(true));
        assertThat(run("cased", 3, 4)).isEqualTo(answered(false));
        assertThat(run("cased", 3, -3)).isEqualTo(answered(false));
    }

    @Test
    void twoValuesOfASumOfUnitsAreEqualWhereTheyAreOneCase() throws Exception {
        assertThat(run("staged", 1, 1)).isEqualTo(answered(true));
        assertThat(run("staged", 0, 0)).isEqualTo(answered(true));
        assertThat(run("staged", 1, 0)).isEqualTo(answered(false));
    }

    /** An optional is made by reading a list, which answers nothing past its end. */
    @Test
    void twoOptionalsAreEqualWhereBothAreAbsentOrBothHoldTheSame() throws Exception {
        assertThat(run("held", 5, 6)).isEqualTo(answered(true));
        assertThat(run("held", 1, 1)).isEqualTo(answered(true));
        assertThat(run("held", 1, 5)).isEqualTo(answered(false));
        assertThat(run("held", 5, 1)).isEqualTo(answered(false));
        assertThat(run("held", 0, 1)).isEqualTo(answered(false));
    }

    @Test
    void twoTuplesAreEqualMemberByMember() throws Exception {
        assertThat(run("paired", 1, 1)).isEqualTo(answered(true));
        assertThat(run("paired", 1, 2)).isEqualTo(answered(false));
    }

    /** A type holding a list of itself: its comparison reaches itself, and is written once. */
    @Test
    void aTypeHoldingItselfIsComparedAllTheWayDown() throws Exception {
        assertThat(run("nested", 1, 1)).isEqualTo(answered(true));
        assertThat(run("nested", 1, 2)).isEqualTo(answered(false));
    }

    @Test
    void notEqualIsTheOtherAnswer() throws Exception {
        assertThat(run("unlike", 1, 1)).isEqualTo(answered(false));
        assertThat(run("unlike", 1, 2)).isEqualTo(answered(true));
    }
}
