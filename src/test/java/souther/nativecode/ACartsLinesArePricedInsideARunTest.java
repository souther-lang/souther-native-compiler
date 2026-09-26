package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The part of the cart model #58 ports that rests on a list, run natively and held to its own
 * rows: an order's lines as a {@code List<OrderLine>}, a priced cart that refuses to be empty with
 * {@code List.length(lines) >= 1}, and a subtotal worked out by a helper walking the lines with a
 * function value it is handed.
 *
 * <p>The subtotal is a helper of this module's and not {@code List.fold}. The standard library's
 * {@code fold} is written over {@code foldFrom}, a helper polymorphic in what it walks, and a
 * helper like that is #64; a user's own recursive helper at concrete types is what is asked here.
 * Every row states scalars; rows stating an order are in {@link ARowHoldsWhereverItIsRunTest}.
 */
class ACartsLinesArePricedInsideARunTest {

    private static final String CART = """
            module pricing exposing ( emptied )

            data OrderLine = { sku: String, quantity: Int, unitPrice: Int }
                invariant quantity >= 1

            data PricedCart = { lines: List<OrderLine>, total: Int }
                invariant List.length(lines) >= 1

            partial let sumFrom (step: (Int, OrderLine) -> Int, acc: Int, xs: List<OrderLine>, i: Int): Int =
                match List.get(i, xs) with
                    | Some x -> sumFrom(step, step(acc, x), xs, i + 1)
                    | None -> acc

            partial let subtotalOf (lines: List<OrderLine>): Int =
                sumFrom((acc, line) -> acc + line.quantity * line.unitPrice, 0, lines, 0)

            let linesOf (apples: Int, pears: Int): List<OrderLine> =
                [ OrderLine { sku = "apple", quantity = apples, unitPrice = 150 }
                , OrderLine { sku = "pear", quantity = pears, unitPrice = 90 }
                ]

            behavior priced : (apples: Int, pears: Int) -> Int
            let priced (apples, pears) = {
                let lines = linesOf(apples, pears)
                PricedCart { lines = lines, total = subtotalOf(lines) }.total
            }

            behavior counted : (apples: Int, pears: Int) -> Int
            let counted (apples, pears) = List.length(PricedCart {
                lines = linesOf(apples, pears), total = 0 }.lines)

            behavior unchanged : (apples: Int, pears: Int) -> Bool
            let unchanged (apples, pears) = linesOf(apples, pears) == linesOf(2, 3)

            behavior emptied : (apples: Int) -> Int
            let emptied (apples) = {
                let lines: List<OrderLine> = if apples > 100 then linesOf(apples, 1) else []
                PricedCart { lines = lines, total = apples }.total
            }

            example priced
                | "two apples and three pears" : (2, 3) -> 570
                | "one of each" : (1, 1) -> 240

            example counted
                | "both lines" : (2, 3) -> 2

            example unchanged
                | "the same lines built twice" : (2, 3) -> true
                | "one line differs" : (2, 4) -> false
            """;

    @Test
    void everyRowOfTheCartHoldsNatively() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(CART);
    }

    /** A priced cart with no lines is not one: its clause ends the run as the JVM's does. */
    @Test
    void aCartWithNoLinesIsNotBuilt() throws Exception {
        CheckedProgram program = Checked.of(List.of(CART));
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior emptied = module.behaviors().stream()
                .filter(it -> it.name().name().equals("emptied"))
                .findFirst()
                .orElseThrow();

        assertThat(Running.of(program).answeredOrEnded(module, emptied,
                List.of(new ObservedValue.Integer(1))))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.INVARIANT_NOT_HELD));
    }
}
