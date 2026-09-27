package souther.nativecode;

import tools.jackson.databind.DeserializationFeature;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;
import net.unit8.raoh.Err;
import net.unit8.raoh.Issue;
import net.unit8.raoh.Result;
import net.unit8.raoh.decode.Decoder;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.generated.MemoryClassLoader;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.List;
import java.util.Map;
import java.util.StringJoiner;
import java.util.TreeMap;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A broken clause of a newtype is reported as the JVM's decoder reports it (#97): where the checker
 * found a clause to be a standard constraint, as that constraint's code, message key and metadata,
 * and where it found none, or not the whole clause, as {@code invariant_violation}.
 *
 * <p>The JVM is the oracle. The same module is compiled to classes and each document decoded by the
 * generated {@code jsonDecoder()}, and what its issues say is held to what the native decoder of the
 * same type answers: the path, the code, the message key, and the metadata, numbers compared as the
 * numbers they are. So every constraint the checker has a form for is here once, beside the shapes
 * where the order of what is reported matters: two constraints in one clause, a constraint and a
 * rule no constraint states in one clause, a constraint declared after a clause stated as none, and
 * a product of one field, whose clauses run whole whatever they are as constraints.
 */
class ABrokenClauseIsReportedAsTheJvmReportsItTest {

    private static final String MODULE = """
            module held exposing ( Short, Long, Exact, Coded, Positive, Counted, AtLeast, Below, \
            Price, Capped, Charge, Owed, Tags, Several, Few, Pair, Distinct, Words, Keyed, Filled, \
            Sparse, Ranged, Partly, Digits, Box )

            data Short = String
                invariant String.length(value) > 0

            data Long = String
                invariant String.length(value) <= 3

            data Exact = String
                invariant String.length(value) == 4

            data Coded = String
                invariant String.matches("[0-9]{3}", value)

            data Positive = Int
                invariant value > 0

            data Counted = Int
                invariant value >= 0

            data AtLeast = Int
                invariant value >= 3

            data Below = Int
                invariant value < 10

            data Price = Decimal
                invariant value >= 1.5m

            data Capped = Decimal
                invariant value <= 99.99m

            data Charge = Decimal
                invariant value > 0.0m

            data Owed = Decimal
                invariant value >= 0.0m

            data Tags = List<Int>
                invariant List.length(value) >= 1

            data Several = List<Int>
                invariant List.length(value) >= 3

            data Few = List<Int>
                invariant List.length(value) <= 1

            data Pair = List<Int>
                invariant List.length(value) == 2

            data Distinct = List<Int>
                invariant List.allDistinctBy(x -> x, value)

            data Words = List<String>
                invariant List.allDistinctBy(x -> x, value)

            data Keyed = Map<String, Int>
                invariant Map.size(value) >= 2

            data Filled = Map<String, Int>
                invariant Map.size(value) >= 1

            data Sparse = Map<String, Int>
                invariant Map.size(value) <= 2

            data Ranged = String
                invariant String.length(value) >= 2 && String.length(value) <= 4

            data Partly = String
                invariant partly = String.length(value) >= 2 && List.all(c -> c <= 57, String.codePoints(value))

            data Digits = String
                invariant digitsOnly = List.all(c -> c <= 57, String.codePoints(value))
                invariant long = String.length(value) >= 5

            data Box = { n: Int }
                invariant n >= 0
            """;

    /** Every document, under a label, read as a type. */
    private static final List<Decoding.Row> ROWS = List.of(
            new Decoding.Row("short", "Short", "\"\""),
            new Decoding.Row("short held", "Short", "\"a\""),
            new Decoding.Row("long", "Long", "\"abcd\""),
            new Decoding.Row("exact", "Exact", "\"abc\""),
            new Decoding.Row("exact in characters", "Exact", "\"\\u304c\\u304c\\u304c\\u304c\""),
            new Decoding.Row("coded", "Coded", "\"12a\""),
            new Decoding.Row("positive", "Positive", "0"),
            new Decoding.Row("counted", "Counted", "-1"),
            new Decoding.Row("at least", "AtLeast", "2"),
            new Decoding.Row("below", "Below", "10"),
            new Decoding.Row("price", "Price", "1.49"),
            new Decoding.Row("price at its scale", "Price", "1.50"),
            new Decoding.Row("capped", "Capped", "100.00"),
            new Decoding.Row("charge", "Charge", "0.00"),
            new Decoding.Row("owed", "Owed", "-0.5"),
            new Decoding.Row("tags", "Tags", "[]"),
            new Decoding.Row("several", "Several", "[1, 2]"),
            new Decoding.Row("few", "Few", "[1, 2]"),
            new Decoding.Row("pair", "Pair", "[1]"),
            new Decoding.Row("distinct", "Distinct", "[3, 1, 3, 2, 1, 3]"),
            new Decoding.Row("distinct held", "Distinct", "[1, 2]"),
            new Decoding.Row("words", "Words", "[\"a\", \"b\", \"a\"]"),
            new Decoding.Row("keyed", "Keyed", "{\"a\": 1}"),
            new Decoding.Row("filled", "Filled", "{}"),
            new Decoding.Row("sparse", "Sparse", "{\"a\": 1, \"b\": 2, \"c\": 3}"),
            new Decoding.Row("ranged short", "Ranged", "\"a\""),
            new Decoding.Row("ranged long", "Ranged", "\"abcde\""),
            new Decoding.Row("partly short", "Partly", "\"1\""),
            new Decoding.Row("partly letters", "Partly", "\"ab\""),
            new Decoding.Row("digits both", "Digits", "\"ab\""),
            new Decoding.Row("digits short", "Digits", "\"12\""),
            new Decoding.Row("box", "Box", "{\"n\": -1}"));

    /** Reads a fraction as the decimal it was written as, which a boundary reads a {@code Decimal}
     *  from, and not as a {@code double}, which it refuses. */
    private static final JsonMapper JSON = JsonMapper.builder()
            .enable(DeserializationFeature.USE_BIG_DECIMAL_FOR_FLOATS)
            .build();

    private static ClassLoader jvm;

    @BeforeAll
    static void compile() {
        jvm = new MemoryClassLoader(Compiler.compile(MODULE),
                ABrokenClauseIsReportedAsTheJvmReportsItTest.class.getClassLoader());
    }

    @Test
    void everyIssueIsTheOneTheJvmsDecoderReports() throws Exception {
        Decoding decoding = new Decoding();
        for (Decoding.Row row : ROWS) {
            decoding.type("held", row.type());
        }
        StringBuilder expected = new StringBuilder();
        for (Decoding.Row row : ROWS) {
            decoding.row(row.label(), row.type(), row.document());
            expected.append(row.label()).append(": ").append(jvmRead(row)).append('\n');
        }

        String answered = AValueIsReadFromTheFormItIsWrittenInTest.run(
                Checked.of(List.of(MODULE)), decoding.harness());

        assertThat(valuesUnwritten(answered)).isEqualTo(expected.toString());
    }

    private static final String DECLARED = """
            module shelf exposing ( Sku, Skus, Point, Points )

            data Sku = String
                invariant String.length(value) > 0

            data Skus = List<Sku>
                invariant List.allDistinctBy(x -> x, value)

            data Point = { x: Int, y: Int }

            data Points = List<Point>
                invariant List.allDistinctBy(p -> p, value)
            """;

    /**
     * The elements a list repeats are reported as a boundary writes them, whatever they are: a
     * newtype as what it holds and a product as an object. The JVM puts the model's own values in
     * its metadata, which have no one written form to hold these to, so this is held to the form a
     * boundary writes, which is the one the native library hands a host anywhere else.
     */
    @Test
    void theElementsAListRepeatsAreWrittenAsABoundaryWritesThem() throws Exception {
        Decoding decoding = new Decoding().type("shelf", "Skus").type("shelf", "Points")
                .row("skus", "Skus", "[\"a\", \"b\", \"a\"]")
                .row("points", "Points", "[{\"x\": 1, \"y\": 2}, {\"x\": 1, \"y\": 2}]");

        String answered = AValueIsReadFromTheFormItIsWrittenInTest.run(
                Checked.of(List.of(DECLARED)), decoding.harness());

        assertThat(answered).isEqualTo("""
                skus: issues [@ duplicate_element {"duplicates":["a"]}]
                points: issues [@ duplicate_element {"duplicates":[{"x":1,"y":2}]}]
                """);
    }

    /** What the JVM's decoder says of the document, in the words the native harness writes. */
    private static String jvmRead(Decoding.Row row) throws Exception {
        @SuppressWarnings("unchecked")
        Decoder<JsonNode, ?> decoder = (Decoder<JsonNode, ?>) jvm.loadClass("held." + row.type())
                .getMethod("jsonDecoder").invoke(null);
        Result<?> result = decoder.decode(JSON.readTree(row.document()), net.unit8.raoh.Path.ROOT);
        if (!(result instanceof Err<?> refused)) {
            return "value";
        }
        StringBuilder said = new StringBuilder("issues");
        for (Issue issue : refused.issues().asList()) {
            said.append(" [@").append(issue.path().toJsonPointer()).append(' ').append(issue.code());
            if (!issue.messageKey().equals(issue.code())) {
                said.append(" key=").append(issue.messageKey());
            }
            if (!issue.meta().isEmpty()) {
                said.append(' ').append(json(issue.meta()));
            }
            said.append(']');
        }
        return said.toString();
    }

    /** The native lines with what a value is written back as left out: the JVM half says only that
     *  it read one. */
    private static String valuesUnwritten(String answered) {
        StringBuilder out = new StringBuilder();
        for (String line : answered.split("\n")) {
            int at = line.indexOf(": value ");
            out.append(at < 0 ? line : line.substring(0, at) + ": value").append('\n');
        }
        return out.toString();
    }

    /**
     * Metadata as the native runtime writes it: an object's entries in the order of their names, a
     * number as the amount it is, a string escaped as JSON escapes one.
     */
    private static String json(Object said) throws Exception {
        return switch (said) {
            case Map<?, ?> map -> {
                StringJoiner entries = new StringJoiner(",", "{", "}");
                for (Map.Entry<?, ?> entry : new TreeMap<>(map).entrySet()) {
                    entries.add(JSON.writeValueAsString(entry.getKey().toString()) + ":"
                            + json(entry.getValue()));
                }
                yield entries.toString();
            }
            case List<?> list -> {
                StringJoiner items = new StringJoiner(",", "[", "]");
                for (Object item : list) {
                    items.add(json(item));
                }
                yield items.toString();
            }
            case String text -> JSON.writeValueAsString(text);
            case Integer number -> number.toString();
            case Long number -> number.toString();
            case BigInteger number -> number.toString();
            case BigDecimal number -> number.signum() == 0
                    ? "0" : number.stripTrailingZeros().toPlainString();
            default -> throw new IllegalArgumentException(
                    "metadata this test has no written form for: " + said.getClass());
        };
    }
}
