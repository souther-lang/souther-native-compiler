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
 * Every operator and kernel over a {@code Decimal}, held to what the JVM answers for the same
 * program.
 *
 * <p>The rows are the oracle, as they are for the other kernels: a row that ran and kept its answer
 * is one the JVM answered, and the native run of it is held to that answer. A boundary writes a
 * {@code Decimal} as its amount and not at its scale, so a row about a scale answers the value's
 * plain notation, which is written at its scale ({@code String.fromDecimal}).
 *
 * <p>The rows are where the two carriers could part: the scale a sum, a difference and a product
 * answer at, each rounding mode either side of a half and at it, a scale below nought, a quotient
 * a long way below the unit it is rounded to, decimal text with and without its fraction, and a
 * rounding mode made in one place and read in another.
 */
class ADecimalAnswersWhatTheJvmAnswersTest {

    private static final String DECIMALS = """
            module decimals

            let mode (n: Int): RoundingMode =
                if n == 0 then HALF_UP
                else if n == 1 then HALF_EVEN
                else if n == 2 then HALF_DOWN
                else if n == 3 then UP
                else if n == 4 then DOWN
                else if n == 5 then CEILING
                else FLOOR

            behavior literal : (a: Int) -> String
            let literal (a) = String.fromDecimal(-1.50m)

            behavior negated : (d: Decimal) -> String
            let negated (d) = String.fromDecimal(-d)

            behavior sum : (a: Decimal, b: Decimal) -> String
            let sum (a, b) = String.fromDecimal(a + b)

            behavior difference : (a: Decimal, b: Decimal) -> String
            let difference (a, b) = String.fromDecimal(a - b)

            behavior product : (a: Decimal, b: Decimal) -> String
            let product (a, b) = String.fromDecimal(a * b)

            behavior added : (a: Decimal, b: Decimal) -> Decimal
            let added (a, b) = Decimal.add(a, b)

            behavior subtracted : (a: Decimal, b: Decimal) -> Decimal
            let subtracted (a, b) = Decimal.subtract(a, b)

            behavior multiplied : (a: Decimal, b: Decimal) -> Decimal
            let multiplied (a, b) = Decimal.multiply(a, b)

            behavior same : (a: Decimal, b: Decimal) -> Bool
            let same (a, b) = a == b

            behavior below : (a: Decimal, b: Decimal) -> Bool
            let below (a, b) = a < b

            behavior compared : (a: Decimal, b: Decimal) -> Int
            let compared (a, b) = Decimal.compare(a, b)

            behavior widened : (n: Int) -> String
            let widened (n) = String.fromDecimal(Decimal.fromInt(n))

            behavior whole : (n: Int, d: Decimal) -> Int
            let whole (n, d) = Decimal.toInt(mode(n), d)

            behavior rounded : (scale: Int, n: Int, d: Decimal) -> String
            let rounded (scale, n, d) = String.fromDecimal(Decimal.round(scale, mode(n), d))

            behavior divided : (a: Decimal, b: Decimal, scale: Int, n: Int) -> String
            let divided (a, b, scale, n) = match Decimal.divide(a, b, scale, mode(n)) with
                | Decimal as q -> String.fromDecimal(q)
                | DivisionByZero -> "nothing divided"

            behavior read : (s: String) -> String
            let read (s) = match String.toDecimal(s) with
                | Decimal as d -> String.fromDecimal(d)
                | NotANumber -> "no number"

            behavior amount : (d: Decimal) -> Decimal
            let amount (d) = d

            behavior total : (xs: List<Decimal>) -> String
            let total (xs) = String.fromDecimal(List.sum(xs))

            behavior multipliedOut : (xs: List<Decimal>) -> String
            let multipliedOut (xs) = String.fromDecimal(List.product(xs))

            behavior sorted : (xs: List<Decimal>) -> List<Decimal>
            let sorted (xs) = List.sort(xs)

            behavior greatest : (xs: List<Decimal>) -> String
            let greatest (xs) = match List.max(xs) with
                | Some d -> String.fromDecimal(d)
                | None -> "empty"

            behavior named : (n: Int) -> Int
            let named (n) = match mode(n) with
                | HALF_UP -> 0
                | HALF_EVEN -> 1
                | HALF_DOWN -> 2
                | UP -> 3
                | DOWN -> 4
                | CEILING -> 5
                | FLOOR -> 6

            behavior sameMode : (a: Int, b: Int) -> Bool
            let sameMode (a, b) = mode(a) == mode(b)

            example literal
                | "a literal keeps its scale" : (0) -> "-1.50"

            example negated
                | "the scale stays" : (1.50m) -> "-1.50"
                | "nought has no sign" : (0.00m) -> "0.00"

            example sum
                | "at the larger scale" : (1.5m, 2.25m) -> "3.75"
                | "to nought at a scale" : (2.5m, -2.50m) -> "0.00"
                | "past what a long holds" : (99999999999999999999.99m, 0.01m) -> "100000000000000000000.00"

            example difference
                | "at the larger scale" : (1m, 0.001m) -> "0.999"
                | "below nought" : (-5m, 3.5m) -> "-8.5"

            example product
                | "at the sum of the scales" : (0.10m, 100.0m) -> "10.000"
                | "of two signs" : (-1.5m, 2m) -> "-3.0"

            example added
                | "an amount" : (1.5m, 2.25m) -> 3.75m

            example subtracted
                | "an amount" : (1m, 0.25m) -> 0.75m

            example multiplied
                | "an amount" : (1.5m, 1.5m) -> 2.25m

            example same
                | "whatever the scales" : (1.0m, 1.00m) -> true
                | "two amounts" : (1.0m, 1.01m) -> false
                | "nought at two scales" : (0m, 0.000m) -> true

            example below
                | "by amount" : (1.9m, 1.10m) -> false
                | "below nought" : (-2m, -1.99m) -> true

            example compared
                | "below" : (1m, 1.01m) -> -1
                | "at, whatever the scales" : (12.30m, 12.3m) -> 0
                | "above" : (-1m, -1.01m) -> 1

            example widened
                | "at scale nought" : (-42) -> "-42"
                | "the smallest Int but one" : (-9223372036854775807) -> "-9223372036854775807"

            example whole
                | "half up" : (0, 2.5m) -> 3
                | "half even at an even" : (1, 2.5m) -> 2
                | "half even at an odd" : (1, 3.5m) -> 4
                | "half down" : (2, 2.5m) -> 2
                | "up" : (3, 1.1m) -> 2
                | "down" : (4, 1.9m) -> 1
                | "ceiling below nought" : (5, -1.9m) -> -1
                | "floor below nought" : (6, -1.1m) -> -2
                | "half up below nought" : (0, -2.5m) -> -3
                | "the largest Int" : (4, 9223372036854775807.9m) -> 9223372036854775807

            example rounded
                | "to two places" : (2, 0, 1.005m) -> "1.01"
                | "half even to two places" : (2, 1, 1.005m) -> "1.00"
                | "to more places" : (4, 0, 1.5m) -> "1.5000"
                | "to a scale below nought" : (-2, 0, 1250m) -> "1300"
                | "floor below nought" : (0, 6, -0.5m) -> "-1"
                | "a long way below the unit" : (0, 3, 0.0000000000000000000001m) -> "1"

            example divided
                | "a third" : (10m, 3m, 2, 0) -> "3.33"
                | "two thirds down" : (2m, 3m, 2, 4) -> "0.66"
                | "floor below nought" : (-2m, 3m, 2, 6) -> "-0.67"
                | "an eighth half even" : (1m, 8m, 2, 1) -> "0.12"
                | "three eighths half even" : (3m, 8m, 2, 1) -> "0.38"
                | "by a fraction" : (100m, 0.5m, 0, 0) -> "200"
                | "to a scale below nought" : (12345m, 1m, -2, 0) -> "12300"
                | "a long way below the unit" : (1m, 10000000000m, 2, 3) -> "0.01"
                | "nought at a scale" : (0m, 7m, 3, 0) -> "0.000"
                | "by nought" : (1m, 0.00m, 2, 0) -> "nothing divided"
                | "by nought at no scale a Decimal has" : (1m, 0m, 4294967298, 0) -> "nothing divided"

            example read
                | "a whole number" : ("1") -> "1"
                | "leading zeros and a scale" : ("001.50") -> "1.50"
                | "a sign" : ("+7.25") -> "7.25"
                | "nought below nought" : ("-0.00") -> "0.00"
                | "no digits" : ("-") -> "no number"
                | "a word" : ("ten") -> "no number"

            example amount
                | "written as its amount" : (1.50m) -> 1.5m
                | "a whole amount" : (100.00m) -> 100m

            example total
                | "of none" : ([]) -> "0"
                | "at the largest scale" : ([1.5m, 2.25m, 3m]) -> "6.75"

            example multipliedOut
                | "of none" : ([]) -> "1"
                | "at the sum of the scales" : ([1.5m, 2.0m, 0.10m]) -> "0.3000"

            example sorted
                | "by amount" : ([2.5m, -1m, 1.25m, 1.2m]) -> [-1m, 1.2m, 1.25m, 2.5m]

            example greatest
                | "the first of the greatest" : ([1.0m, 2.50m, 2.5m]) -> "2.50"
                | "of none" : ([]) -> "empty"

            example named
                | "half up" : (0) -> 0
                | "half even" : (1) -> 1
                | "half down" : (2) -> 2
                | "up" : (3) -> 3
                | "down" : (4) -> 4
                | "ceiling" : (5) -> 5
                | "floor" : (6) -> 6

            example sameMode
                | "one mode" : (3, 3) -> true
                | "two modes" : (3, 4) -> false
            """;

    @Test
    void everyDecimalRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(DECIMALS);
    }

    /**
     * A behavior handed a {@code Decimal} is handed its integer and its scale, as a host makes one,
     * and what it answers is read back as the amount the boundary wrote.
     */
    @Test
    void aDecimalHandedOverIsTheValueItWasMadeOf() throws Exception {
        Asked decimals = new Asked(DECIMALS);
        assertThat(decimals.outcome("sum", decimal("1.50"), decimal("-0.25")))
                .isEqualTo(answered(new ObservedValue.Text("1.25")));
        assertThat(decimals.outcome("negated", decimal("12E+2")))
                .isEqualTo(answered(new ObservedValue.Text("-1200")));
        assertThat(decimals.outcome("amount", decimal("123456789012345678901234567890.1230")))
                .isEqualTo(answered(decimal("123456789012345678901234567890.123")));
    }

    /**
     * A result whose scale is outside the 32-bit range ends the run, and so does a scale asked for
     * outside it and a whole number no {@code Int} holds: each for the one reason the operation's
     * contract names.
     */
    @Test
    void aDecimalOperationEndsTheRunWhereItsContractSays() throws Exception {
        Asked decimals = new Asked(DECIMALS);
        ObservedValue tiny = decimal("1E-2147483647");
        RunOutcome noPlace = new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE);
        assertThat(decimals.outcome("product", tiny, decimal("0.1"))).isEqualTo(noPlace);
        assertThat(decimals.outcome("multiplied", tiny, decimal("0.1"))).isEqualTo(noPlace);
        assertThat(decimals.outcome("rounded", integer(2147483648L), integer(0), decimal("1")))
                .isEqualTo(noPlace);
        assertThat(decimals.outcome("divided", decimal("1"), decimal("3"), integer(2147483648L),
                integer(0))).isEqualTo(noPlace);
        assertThat(decimals.outcome("whole", integer(4), decimal("9223372036854775808")))
                .isEqualTo(noPlace);
    }

    /**
     * Decimal text is the grammar the language states for it (spec §string-decimal-text): no
     * exponent, a digit either side of a point, and ASCII digits only. The JVM this build is held
     * to reads {@code String.toDecimal} as {@code new BigDecimal(text)}, which reads all four of
     * these, and souther-lang/souther reads them by the grammar from f391aa62a on; so these are
     * held to what the language says and not to a row the JVM answered.
     */
    @Test
    void decimalTextIsTheGrammarTheLanguageStates() throws Exception {
        Asked decimals = new Asked(DECIMALS);
        for (String notDecimalText : List.of("1e5", ".5", "5.", "１２３")) {
            assertThat(decimals.outcome("read", new ObservedValue.Text(notDecimalText)))
                    .as(notDecimalText)
                    .isEqualTo(answered(new ObservedValue.Text("no number")));
        }
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
