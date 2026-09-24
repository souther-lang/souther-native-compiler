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
 * {@code Int.truncatingDivide} and {@code Int.truncatingRemainder} answer {@code Int |
 * DivisionByZero}: a zero divisor is a case of the answer, and the one pair whose quotient no
 * {@code Int} holds ends a quotient.
 *
 * <p>The answer is a union with a primitive among its cases, which is a value of its own here and
 * not something the kernel call and the {@code match} after it agree on between them. So it is
 * matched where it is answered, held in a binding first, answered by a helper and matched by its
 * caller, and made by an {@code Int} standing as the union, and each of those has to reach the arm
 * its case is.
 */
class AWholeNumberDivisionAnswersACaseTest {

    /** What no division below answers, so an arm answering it is the zero divisor's and no other. */
    private static final long NOTHING_DIVIDED = 424242;

    private static final String SOURCE = """
            module dividing exposing ( quotient, remainder, held, halved, standing, same, listed )

            behavior quotient : (a: Int, b: Int) -> Int
            let quotient (a, b) = match Int.truncatingDivide(a, b) with
                | Int as q -> q
                | DivisionByZero -> 424242

            behavior remainder : (a: Int, b: Int) -> Int
            let remainder (a, b) = match Int.truncatingRemainder(a, b) with
                | Int as r -> r
                | DivisionByZero -> 424242

            behavior held : (a: Int, b: Int) -> Int
            let held (a, b) = {
                let answered = Int.truncatingDivide(a, b)
                let doubled = a * 2
                match answered with
                    | Int as q -> q + doubled - doubled
                    | DivisionByZero -> 424242
            }

            let half (n: Int, by: Int): Int | DivisionByZero = Int.truncatingDivide(n, by)

            behavior halved : (n: Int, by: Int) -> Int
            let halved (n, by) = match half(n, by) with
                | Int as q -> q
                | DivisionByZero -> 424242

            behavior standing : (n: Int) -> Int
            let standing (n) = {
                let answered: Int | DivisionByZero = n
                match answered with
                    | Int as q -> q + 1
                    | DivisionByZero -> 424242
            }

            behavior same : (a: Int, b: Int, c: Int, d: Int) -> Bool
            let same (a, b, c, d) = Int.truncatingDivide(a, b) == Int.truncatingDivide(c, d)

            behavior listed : (a: Int, b: Int, at: Int) -> Int
            let listed (a, b, at) = {
                let answers = [Int.truncatingDivide(a, b), Int.truncatingDivide(b, a)]
                match List.get(at, answers) with
                    | Some answered -> match answered with
                        | Int as q -> q
                        | DivisionByZero -> 424242
                    | None -> -1
            }
            """;

    /** Truncated toward zero, so the quotient takes the sign the two operands' signs give it. */
    @Test
    void aQuotientIsTruncatedTowardZero() throws Exception {
        assertAnswers("quotient", 7, 3, 2);
        assertAnswers("quotient", -7, 3, -2);
        assertAnswers("quotient", 7, -3, -2);
        assertAnswers("quotient", -7, -3, 2);
        assertAnswers("quotient", 6, 3, 2);
        assertAnswers("quotient", 0, 5, 0);
        assertAnswers("quotient", Long.MIN_VALUE, 1, Long.MIN_VALUE);
    }

    /** What is left over takes the dividend's sign, which is what truncating the quotient means. */
    @Test
    void aRemainderTakesTheDividendsSign() throws Exception {
        assertAnswers("remainder", 7, 3, 1);
        assertAnswers("remainder", -7, 3, -1);
        assertAnswers("remainder", 7, -3, 1);
        assertAnswers("remainder", -7, -3, -1);
        assertAnswers("remainder", 6, 3, 0);
    }

    /** A zero divisor is answered, as the case it is, and does not end the run. */
    @Test
    void aZeroDivisorIsTheDivisionByZeroCase() throws Exception {
        assertAnswers("quotient", 7, 0, NOTHING_DIVIDED);
        assertAnswers("quotient", 0, 0, NOTHING_DIVIDED);
        assertAnswers("remainder", -7, 0, NOTHING_DIVIDED);
        assertAnswers("remainder", Long.MIN_VALUE, 0, NOTHING_DIVIDED);
    }

    /**
     * The smallest {@code Int} over -1 has a quotient no {@code Int} holds, and the run ends; the
     * pair beside it answers through the same executable. The remainder of that pair is nought,
     * which an {@code Int} holds, so it is answered.
     */
    @Test
    void theOnePairNoIntHoldsTheQuotientOfEndsTheRun() throws Exception {
        assertThat(outcome("quotient", Long.MIN_VALUE, -1))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertAnswers("quotient", Long.MIN_VALUE + 1, -1, Long.MAX_VALUE);
        assertAnswers("remainder", Long.MIN_VALUE, -1, 0);
        assertAnswers("remainder", Long.MIN_VALUE + 1, -1, 0);
    }

    /**
     * The answer is a value of the union wherever it goes: held in a binding while something else
     * is worked out, answered by a helper and matched by its caller, or made by an {@code Int}
     * standing as the union.
     */
    @Test
    void theAnswerIsAValueOfTheUnionWhereverItIsHeld() throws Exception {
        assertAnswers("held", -9, 2, -4);
        assertAnswers("held", 9, 0, NOTHING_DIVIDED);
        assertAnswers("halved", 11, 2, 5);
        assertAnswers("halved", 11, 0, NOTHING_DIVIDED);

        assertThat(outcome("standing", 41))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(42)));
        assertThat(outcome("standing", Long.MIN_VALUE))
                .isEqualTo(new RunOutcome.Answered(
                        new ObservedValue.Integer(Long.MIN_VALUE + 1)));
    }

    /**
     * Two answers are equal where they are the same case and, for the {@code Int} case, the same
     * number: which case is read off the token, and the number out of what carries it.
     */
    @Test
    void twoAnswersAreEqualWhereTheyAreTheSameCaseHoldingTheSame() throws Exception {
        assertThat(outcome("same", 7, 2, 6, 2)).isEqualTo(answered(true));
        assertThat(outcome("same", 7, 2, 8, 2)).isEqualTo(answered(false));
        assertThat(outcome("same", 7, 0, 5, 0)).isEqualTo(answered(true));
        assertThat(outcome("same", 7, 0, 7, 1)).isEqualTo(answered(false));
        assertThat(outcome("same", 0, 5, 1, 0)).isEqualTo(answered(false));
    }

    /** An answer held in a list is read back out as the case it was. */
    @Test
    void anAnswerHeldInAListIsReadBackAsItsCase() throws Exception {
        assertThat(outcome("listed", 9, 2, 0))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(4)));
        assertThat(outcome("listed", 9, 0, 0))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(NOTHING_DIVIDED)));
        assertThat(outcome("listed", 9, 0, 1))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(0)));
        assertThat(outcome("listed", 9, 2, 2))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(-1)));
    }

    private static RunOutcome answered(boolean truth) {
        return new RunOutcome.Answered(new ObservedValue.Bool(truth));
    }

    /**
     * The cart's discount: ten per cent of a subtotal of 5000 or more, truncated to a whole amount,
     * and nothing below it. The charge is built of the model's own newtypes, and the rows read what
     * it was built with; they hold natively.
     */
    @Test
    void theCartsDiscountRowsHold() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds("""
                module cart exposing ( discounted, charged )

                data Money = Int
                data Charge = { subtotal: Money, discount: Money, total: Money }

                let discountOf (sub: Int): Int =
                    if sub >= 5000 then
                        match Int.truncatingDivide(sub * 10, 100) with
                            | Int as d -> d
                            | DivisionByZero -> 0
                    else 0

                let makeCharge (sub: Int): Charge = Charge {
                    subtotal = Money(sub),
                    discount = Money(discountOf(sub)),
                    total = Money(sub - discountOf(sub))
                }

                behavior discounted : (sub: Int) -> Int
                let discounted (sub) = makeCharge(sub).discount.value

                behavior charged : (sub: Int) -> Int
                let charged (sub) = makeCharge(sub).total.value

                example discounted
                    | "below the threshold nothing is taken off" : (4999) -> 0
                    | "at the threshold ten per cent is taken off" : (5000) -> 500
                    | "above it too" : (6000) -> 600
                    | "a share that is not whole is truncated" : (5009) -> 500

                example charged
                    | "below the threshold the subtotal is charged" : (4999) -> 4999
                    | "at the threshold" : (5000) -> 4500
                    | "above it" : (6000) -> 5400
                """);
    }

    private static void assertAnswers(String behavior, long dividend, long divisor, long answered)
            throws Exception {
        assertThat(outcome(behavior, dividend, divisor))
                .as("%s of %d by %d", behavior, dividend, divisor)
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(answered)));
    }

    private static RunOutcome outcome(String behavior, long... handed) throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SOURCE));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior reached = module.behaviors().stream()
                .filter(it -> it.name().name().equals(behavior))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + behavior));
        List<ObservedValue> inputs = java.util.Arrays.stream(handed)
                .mapToObj(it -> (ObservedValue) new ObservedValue.Integer(it))
                .toList();
        return running.answeredOrEnded(module, reached, inputs);
    }
}
