package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.math.BigDecimal;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every operator and kernel that reaches a {@code Rational}, held to what the JVM answers for the
 * same program.
 *
 * <p>A {@code Rational} has no external form, so no behavior takes or answers one and every row
 * reads one back through what a boundary does carry: {@code Rational.toFiniteDecimal} written in
 * plain notation, or a case saying it has none. The rows are the oracle, as they are for the other
 * kernels: a row that ran and kept its answer is one the JVM answered, and the native run of it is
 * held to that answer.
 *
 * <p>The rows are where the two carriers could part: which pairs are read at their exact values and
 * which are refused, a quotient of two {@code Int}s, a value that has no decimal spelling, each
 * rounding mode either side of a half, a decimal at either end of the scale range that is an
 * exponent here and nothing built, and a whole number one past what an {@code Int} holds.
 */
class ARationalAnswersWhatTheJvmAnswersTest {

    private static final String RATIONALS = """
            module rationals

            let mode (n: Int): RoundingMode =
                if n == 0 then HALF_UP
                else if n == 1 then HALF_EVEN
                else if n == 2 then HALF_DOWN
                else if n == 3 then UP
                else if n == 4 then DOWN
                else if n == 5 then CEILING
                else FLOOR

            let render (r: Rational): String = match Rational.toFiniteDecimal(r) with
                | Decimal as d -> String.fromDecimal(d)
                | NotAFiniteDecimal -> "repeating"

            let pick (n: Int, a: Int, b: Int): Int | Rational =
                if n < 0 then n else a / b

            behavior quotient : (a: Int, b: Int) -> String
            let quotient (a, b) = render(a / b)

            behavior mixedSum : (n: Int, a: Int, b: Int) -> String
            let mixedSum (n, a, b) = render(n + a / b)

            behavior mixedDifference : (n: Int, a: Int, b: Int) -> String
            let mixedDifference (n, a, b) = render(n - a / b)

            behavior mixedProduct : (n: Int, a: Int, b: Int) -> String
            let mixedProduct (n, a, b) = render(n * (a / b))

            behavior mixedQuotient : (n: Int, a: Int, b: Int) -> String
            let mixedQuotient (n, a, b) = render(n / (a / b))

            behavior decimalSum : (d: Decimal, a: Int, b: Int) -> String
            let decimalSum (d, a, b) = render(d + a / b)

            behavior decimalProduct : (d: Decimal, a: Int, b: Int) -> String
            let decimalProduct (d, a, b) = render(d * (a / b))

            behavior decimalQuotient : (d: Decimal, a: Int, b: Int) -> String
            let decimalQuotient (d, a, b) = render(d / (a / b))

            behavior difference : (a: Int, b: Int, c: Int, d: Int) -> String
            let difference (a, b, c, d) = render(a / b - c / d)

            behavior quotients : (a: Int, b: Int, c: Int, d: Int) -> String
            let quotients (a, b, c, d) = render((a / b) / (c / d))

            behavior negated : (a: Int, b: Int) -> String
            let negated (a, b) = render(-(a / b))

            behavior below : (n: Int, a: Int, b: Int) -> Bool
            let below (n, a, b) = n < a / b

            behavior atMost : (a: Int, b: Int, n: Int) -> Bool
            let atMost (a, b, n) = a / b <= n

            behavior same : (n: Int, a: Int, b: Int) -> Bool
            let same (n, a, b) = n == a / b

            behavior sameAsDecimal : (d: Decimal, a: Int, b: Int) -> Bool
            let sameAsDecimal (d, a, b) = d == a / b

            behavior aboveDecimal : (d: Decimal, a: Int, b: Int) -> Bool
            let aboveDecimal (d, a, b) = d > a / b

            behavior sameRatio : (a: Int, b: Int, c: Int, d: Int) -> Bool
            let sameRatio (a, b, c, d) = a / b == c / d

            behavior compared : (a: Int, b: Int, c: Int, d: Int) -> Int
            let compared (a, b, c, d) = Rational.compare(a / b, c / d)

            behavior widened : (n: Int) -> String
            let widened (n) = render(Rational.fromInt(n))

            behavior widenedDecimal : (d: Decimal) -> String
            let widenedDecimal (d) = render(Rational.fromDecimal(d))

            behavior whole : (a: Int, b: Int) -> String
            let whole (a, b) = match Rational.toWholeNumber(a / b) with
                | Int as n -> String.fromInt(n)
                | NotWhole -> "not whole"

            behavior wholeDecimal : (d: Decimal) -> String
            let wholeDecimal (d) = match Rational.toWholeNumber(Rational.fromDecimal(d)) with
                | Int as n -> String.fromInt(n)
                | NotWhole -> "not whole"

            behavior wholePlusOne : (n: Int) -> String
            let wholePlusOne (n) =
                match Rational.toWholeNumber(Rational.fromInt(n) + Rational.fromInt(1)) with
                    | Int as m -> String.fromInt(m)
                    | NotWhole -> "not whole"

            behavior plusOne : (d: Decimal) -> String
            let plusOne (d) = render(Rational.fromDecimal(d) + Rational.fromInt(1))

            behavior lesser : (a: Decimal, b: Decimal) -> Bool
            let lesser (a, b) = Rational.fromDecimal(a) < Rational.fromDecimal(b)

            behavior rounded : (n: Int, a: Int, b: Int) -> Int
            let rounded (n, a, b) = Rational.toInt(mode(n), a / b)

            behavior roundedAt : (scale: Int, n: Int, a: Int, b: Int) -> String
            let roundedAt (scale, n, a, b) =
                String.fromDecimal(Rational.toDecimal(scale, mode(n), a / b))

            behavior roundedDecimal : (scale: Int, n: Int, d: Decimal) -> String
            let roundedDecimal (scale, n, d) =
                String.fromDecimal(Rational.toDecimal(scale, mode(n), Rational.fromDecimal(d)))

            behavior total : (a: Int, b: Int, c: Int) -> String
            let total (a, b, c) = render(List.sum([1 / a, 1 / b, 1 / c]))

            behavior multipliedOut : (a: Int, b: Int) -> String
            let multipliedOut (a, b) = render(List.product([a / 2, b / 3]))

            behavior greatest : (a: Int, b: Int, c: Int) -> String
            let greatest (a, b, c) = match List.max([a / 3, b / 3, c / 3]) with
                | Some r -> render(r)
                | None -> "empty"

            behavior least : (a: Int, b: Int, c: Int) -> String
            let least (a, b, c) = match List.min([a / 3, b / 3, c / 3]) with
                | Some r -> render(r)
                | None -> "empty"

            behavior picked : (n: Int, a: Int, b: Int) -> String
            let picked (n, a, b) = match pick(n, a, b) with
                | Int as i -> String.fromInt(i)
                | Rational as r -> render(r)

            example quotient
                | "a half" : (1, 2) -> "0.5"
                | "a third has no decimal" : (1, 3) -> "repeating"
                | "below nought" : (7, -4) -> "-1.75"
                | "nought" : (0, -3) -> "0"
                | "a whole number" : (6, 3) -> "2"
                | "a hundred over eight" : (100, 8) -> "12.5"
                | "a power of two below" : (1, 1024) -> "0.0009765625"
                | "over the sign" : (3, -6) -> "-0.5"
                | "tens" : (300, 3) -> "100"

            example mixedSum
                | "with a half" : (1, 1, 2) -> "1.5"
                | "with a third" : (1, 1, 3) -> "repeating"
                | "below nought" : (-1, 1, 2) -> "-0.5"
                | "nought" : (0, 0, 1) -> "0"

            example mixedDifference
                | "a half from one" : (1, 1, 2) -> "0.5"
                | "a half from nought" : (0, 1, 2) -> "-0.5"

            example mixedProduct
                | "a half of six" : (6, 1, 2) -> "3"
                | "a fifth of three" : (3, 1, 5) -> "0.6"

            example mixedQuotient
                | "over a half" : (3, 1, 2) -> "6"
                | "over a third" : (1, 1, 3) -> "3"
                | "into a third" : (1, 3, 1) -> "repeating"

            example decimalSum
                | "half and half" : (0.5m, 1, 2) -> "1"
                | "a tenth and a third" : (0.10m, 1, 3) -> "repeating"
                | "at the scale it was" : (1.25m, 1, 4) -> "1.5"

            example decimalProduct
                | "a quarter of a tenth" : (0.10m, 1, 4) -> "0.025"
                | "a third of three" : (1.5m, 2, 3) -> "1"

            example decimalQuotient
                | "over a half" : (1.5m, 1, 2) -> "3"
                | "over a third" : (0.1m, 1, 3) -> "0.3"

            example difference
                | "a sixth" : (1, 2, 1, 3) -> "repeating"
                | "a quarter" : (1, 2, 1, 4) -> "0.25"
                | "nought" : (1, 3, 1, 3) -> "0"
                | "below nought" : (1, 4, 1, 2) -> "-0.25"

            example quotients
                | "over a quarter" : (1, 2, 1, 4) -> "2"
                | "two thirds over three quarters" : (2, 3, 3, 4) -> "repeating"
                | "nought over a third" : (0, 1, 1, 3) -> "0"

            example negated
                | "a half" : (1, 2) -> "-0.5"
                | "nought has no sign" : (0, 1) -> "0"
                | "below nought" : (-1, 8) -> "0.125"

            example below
                | "one below a half and one" : (1, 3, 2) -> true
                | "two above" : (2, 3, 2) -> false
                | "at" : (1, 2, 2) -> false
                | "below nought" : (-1, -1, 2) -> true

            example atMost
                | "at" : (4, 2, 2) -> true
                | "above" : (5, 2, 2) -> false
                | "a third" : (1, 3, 1) -> true

            example same
                | "two Ints" : (2, 4, 2) -> true
                | "a half and one" : (1, 1, 2) -> false

            example sameAsDecimal
                | "whatever the scale" : (0.500m, 1, 2) -> true
                | "a third" : (0.5m, 1, 3) -> false

            example aboveDecimal
                | "above a third" : (0.34m, 1, 3) -> true
                | "below a third" : (0.33m, 1, 3) -> false

            example sameRatio
                | "one written twice" : (1, 2, 2, 4) -> true
                | "two values" : (1, 2, 1, 3) -> false

            example compared
                | "below" : (1, 3, 1, 2) -> -1
                | "at" : (2, 4, 1, 2) -> 0
                | "above" : (-1, 3, -1, 2) -> 1

            example widened
                | "an Int" : (-42) -> "-42"
                | "the smallest Int but one" : (-9223372036854775807) -> "-9223372036854775807"
                | "nought" : (0) -> "0"

            example widenedDecimal
                | "at a scale" : (1.50m) -> "1.5"
                | "a whole amount" : (100.00m) -> "100"
                | "nought at a scale" : (0.000m) -> "0"
                | "below nought" : (-0.125m) -> "-0.125"

            example whole
                | "a quotient that is one" : (6, 3) -> "2"
                | "a half" : (5, 2) -> "not whole"
                | "nought" : (0, 7) -> "0"
                | "below nought" : (-8, 4) -> "-2"
                | "the smallest Int but one" : (-9223372036854775807, 1) -> "-9223372036854775807"

            example wholeDecimal
                | "a whole amount" : (100.00m) -> "100"
                | "with a fraction" : (1.50m) -> "not whole"
                | "tens" : (5000m) -> "5000"

            example wholePlusOne
                | "one more" : (41) -> "42"

            example plusOne
                | "a tenth" : (0.10m) -> "1.1"

            example lesser
                | "by amount" : (1.9m, 1.10m) -> false
                | "below" : (-2m, -1.99m) -> true

            example rounded
                | "half up" : (0, 5, 2) -> 3
                | "half even at an odd" : (1, 7, 2) -> 4
                | "half even at an even" : (1, 5, 2) -> 2
                | "half down" : (2, 5, 2) -> 2
                | "up" : (3, 1, 3) -> 1
                | "down" : (4, 5, 2) -> 2
                | "ceiling below nought" : (5, -5, 2) -> -2
                | "floor below nought" : (6, -5, 2) -> -3
                | "half up below nought" : (0, -5, 2) -> -3
                | "a whole number" : (4, 7, 1) -> 7
                | "a third up" : (3, -1, 3) -> -1
                | "a third down" : (4, -1, 3) -> 0

            example roundedAt
                | "a third" : (2, 0, 1, 3) -> "0.33"
                | "two thirds" : (2, 0, 2, 3) -> "0.67"
                | "an eighth" : (4, 0, 1, 8) -> "0.1250"
                | "an eighth half even" : (2, 1, 1, 8) -> "0.12"
                | "an eighth half up" : (2, 0, 1, 8) -> "0.13"
                | "to a whole number" : (0, 3, 1, 3) -> "1"
                | "to a scale below nought" : (-2, 0, 1250, 1) -> "1300"
                | "below nought" : (3, 0, -1, 3) -> "-0.333"
                | "floor below nought" : (0, 6, -1, 3) -> "-1"
                | "nought at a scale" : (2, 0, 0, 5) -> "0.00"

            example roundedDecimal
                | "to two places" : (2, 0, 1.005m) -> "1.01"
                | "half even to two places" : (2, 1, 1.005m) -> "1.00"
                | "to more places than it has" : (4, 0, 1.5m) -> "1.5000"
                | "a long way below the unit" : (0, 3, 0.0000000000000000000001m) -> "1"

            example total
                | "a whole" : (2, 3, 6) -> "1"
                | "thirds" : (3, 3, 3) -> "1"
                | "with none finite" : (1, 2, 3) -> "repeating"

            example multipliedOut
                | "a sixth" : (1, 1) -> "repeating"
                | "one and a half" : (3, 3) -> "1.5"

            example greatest
                | "the last" : (1, 2, 3) -> "1"
                | "below nought" : (-1, -2, -4) -> "repeating"

            example least
                | "the first" : (1, 2, 3) -> "repeating"
                | "below nought" : (-3, -6, -2) -> "-2"

            example picked
                | "an Int" : (-5, 1, 2) -> "-5"
                | "a Rational" : (5, 1, 2) -> "0.5"
                | "a Rational with no decimal" : (5, 1, 3) -> "repeating"
            """;

    @Test
    void everyRationalRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(RATIONALS);
    }

    /** A zero divisor ends the run for the reason a zero divisor is, whatever it divides. */
    @Test
    void aZeroDivisorEndsTheRun() throws Exception {
        Asked rationals = new Asked(RATIONALS);
        RunOutcome byNought = new RunOutcome.Aborted(AbortKind.DIVISION_BY_ZERO);
        assertThat(rationals.outcome("quotient", integer(1), integer(0))).isEqualTo(byNought);
        assertThat(rationals.outcome("mixedQuotient", integer(1), integer(0), integer(5)))
                .isEqualTo(byNought);
        assertThat(rationals.outcome("quotients", integer(1), integer(2), integer(0), integer(3)))
                .isEqualTo(byNought);
        assertThat(rationals.outcome("decimalQuotient", decimal("1.5"), integer(0), integer(3)))
                .isEqualTo(byNought);
    }

    /**
     * A value with no place ends the run for the one reason there is: a whole number one past what
     * an {@code Int} holds, a sum whose common exponent would have to be built two billion places
     * wide, and a value rounded to an {@code Int} that is not one.
     */
    @Test
    void aValueWithNoPlaceEndsTheRun() throws Exception {
        Asked rationals = new Asked(RATIONALS);
        RunOutcome noPlace = new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE);
        assertThat(rationals.outcome("wholePlusOne", integer(Long.MAX_VALUE))).isEqualTo(noPlace);
        assertThat(rationals.outcome("plusOne", decimal("1E-2147483647"))).isEqualTo(noPlace);
        assertThat(rationals.outcome("wholeDecimal", decimal("1E+100"))).isEqualTo(noPlace);
        assertThat(rationals.outcome("roundedDecimal", integer(2147483648L), integer(0),
                decimal("1"))).isEqualTo(noPlace);
        assertThat(rationals.outcome("rounded", integer(0), integer(Long.MAX_VALUE), integer(1)))
                .isEqualTo(answered(new ObservedValue.Integer(Long.MAX_VALUE)));
    }

    /**
     * A decimal at either end of its scale range is an exponent here and nothing built, so an order
     * over it needs no more than the exponents, and answers as the JVM's does.
     */
    @Test
    void aDecimalAtEitherEndOfItsScaleIsOrderedByItsExponent() throws Exception {
        Asked rationals = new Asked(RATIONALS);
        RunOutcome yes = answered(new ObservedValue.Bool(true));
        RunOutcome no = answered(new ObservedValue.Bool(false));
        assertThat(rationals.outcome("lesser", decimal("1E-2147483647"), decimal("1")))
                .isEqualTo(yes);
        assertThat(rationals.outcome("lesser", decimal("1"), decimal("1E-2147483647")))
                .isEqualTo(no);
        assertThat(rationals.outcome("lesser", decimal("-1E+2147483648"), decimal("1E-2147483647")))
                .isEqualTo(yes);
        assertThat(rationals.outcome("lesser", decimal("1E+2147483647"), decimal("2E+2147483647")))
                .isEqualTo(yes);
        // The plain notation of what it reads back as is more text than a string holds.
        // Two that differ in the twenty-fifth digit at the largest exponent, which no bound tells
        // apart, and one that is a decimal's smallest step from the other.
        assertThat(rationals.outcome("lesser", decimal("1E+2147483647"),
                decimal("10000000000000000000000001E+2147483622"))).isEqualTo(yes);
        assertThat(rationals.outcome("lesser", decimal("10000000000000000000000001E+2147483622"),
                decimal("1E+2147483647"))).isEqualTo(no);
        assertThat(rationals.outcome("widenedDecimal", decimal("1E-1500000000")))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
    }

    private static ObservedValue decimal(String written) {
        return new ObservedValue.Decimal(new BigDecimal(written));
    }

    private static ObservedValue integer(long value) {
        return new ObservedValue.Integer(value);
    }

    private static RunOutcome answered(ObservedValue value) {
        return new RunOutcome.Answered(value);
    }

    /** A program checked once and asked as many questions as a test has. */
    private static final class Asked {

        private final CheckedProgram program;
        private final Running running;

        Asked(String source) {
            this.program = Checked.of(List.of(source));
            this.running = Running.of(program);
        }

        RunOutcome outcome(String behavior, ObservedValue... handed) throws Exception {
            CheckedModule module = program.modules().getFirst();
            CheckedBehavior reached = module.behaviors().stream()
                    .filter(it -> it.name().name().equals(behavior))
                    .findFirst()
                    .orElseThrow(() -> new AssertionError("no behavior " + behavior));
            return running.answeredOrEnded(module, reached, List.of(handed));
        }
    }
}
