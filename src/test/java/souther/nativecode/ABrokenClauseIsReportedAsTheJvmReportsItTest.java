package souther.nativecode;

import tools.jackson.databind.DeserializationFeature;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;
import net.unit8.raoh.Err;
import net.unit8.raoh.Issue;
import net.unit8.raoh.Ok;
import net.unit8.raoh.Result;
import net.unit8.raoh.decode.Decoder;
import net.unit8.raoh.encode.Encoder;
import souther.compiler.program.CheckedData;
import tools.jackson.core.JsonParser;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.generated.MemoryClassLoader;

import java.math.BigDecimal;
import java.lang.reflect.RecordComponent;
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
 * generated {@code jsonDecoder()}, and what it answers is held to what the native decoder of the
 * same type answers: a value as the JVM's {@code encoder()} writes it, and an issue's path, code,
 * message key and metadata. Nothing of the JVM's answer is brought to the native one's form: each
 * side is read into one written form, a number as the decimal it is, scale and all, so {@code 1.50}
 * and {@code 1.5} are two answers. So every constraint the checker has a form for is here once, beside the shapes
 * where the order of what is reported matters: two constraints in one clause, a constraint and a
 * rule no constraint states in one clause, a constraint declared after a clause stated as none, and
 * a product of one field, whose clauses run whole whatever they are as constraints.
 */
class ABrokenClauseIsReportedAsTheJvmReportsItTest {

    private static final String MODULE = """
            module held exposing ( Short, Long, Exact, Coded, Positive, Counted, AtLeast, Below, \
            Price, Floor, Capped, Charge, Owed, Tags, Several, Few, Pair, Distinct, Words, Line, \
            Lines, Keyed, Filled, Sparse, Ranged, Partly, Digits, Box, Amounts, Grid )

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

            data Floor = Decimal
                invariant value >= 1.50m

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

            data Line = { amount: Decimal }

            data Lines = List<Line>
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

            data Amounts = List<Decimal>
                invariant List.allDistinctBy(x -> x, value)

            data Grid = List<List<Decimal>>
                invariant List.allDistinctBy(x -> x, value)
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
            new Decoding.Row("floor", "Floor", "1.49"),
            new Decoding.Row("floor held at its scale", "Floor", "1.500"),
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
            new Decoding.Row("lines", "Lines", "[{\"amount\": 1.0}, {\"amount\": 1.00}]"),
            new Decoding.Row("amounts twice", "Amounts", "[1.0, 1.0, 2.50]"),
            new Decoding.Row("amounts at two scales", "Amounts", "[1.0, 1.00]"),
            new Decoding.Row("grid at two scales", "Grid", "[[1.0], [1.00]]"),
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
    void everyAnswerIsTheOneTheJvmsDecoderGives() throws Exception {
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

        assertThat(read(answered)).isEqualTo(expected.toString());
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

    /** What the JVM's decoder says of the document, in the one written form both sides are read
     *  into ({@link #written}). */
    private static String jvmRead(Decoding.Row row) throws Exception {
        Class<?> type = jvm.loadClass("held." + row.type());
        @SuppressWarnings("unchecked")
        Decoder<JsonNode, ?> decoder = (Decoder<JsonNode, ?>) type.getMethod("jsonDecoder")
                .invoke(null);
        Result<?> result = decoder.decode(JSON.readTree(row.document()), net.unit8.raoh.Path.ROOT);
        if (result instanceof Ok<?> read) {
            @SuppressWarnings("unchecked")
            Encoder<Object, ?> encoder = (Encoder<Object, ?>) type.getMethod("encoder").invoke(null);
            return "value " + written(encoder.encode(read.value()));
        }
        StringBuilder said = new StringBuilder("issues");
        for (Issue issue : ((Err<?>) result).issues().asList()) {
            said.append(" [@").append(issue.path().toJsonPointer()).append(' ').append(issue.code());
            if (!issue.messageKey().equals(issue.code())) {
                said.append(" key=").append(issue.messageKey());
            }
            if (!issue.meta().isEmpty()) {
                said.append(' ').append(written(issue.meta()));
            }
            said.append(']');
        }
        return said.toString();
    }

    /**
     * The native harness's lines, each value and each issue's metadata read as JSON and written in
     * the one form {@link #written} writes: what the JVM's answer is written in too.
     */
    private static String read(String answered) throws Exception {
        StringBuilder out = new StringBuilder();
        for (String line : answered.split("\n")) {
            int value = line.indexOf(": value ");
            if (value >= 0) {
                out.append(line, 0, value).append(": value ")
                        .append(written(JSON.readTree(line.substring(value + ": value ".length()))))
                        .append('\n');
                continue;
            }
            StringBuilder rewritten = new StringBuilder();
            int at = 0;
            while (at < line.length()) {
                char c = line.charAt(at);
                if (c == '{') {
                    try (JsonParser parser = JSON.createParser(line.substring(at))) {
                        JsonNode meta = parser.readValueAsTree();
                        rewritten.append(written(meta));
                        at += (int) parser.currentLocation().getCharOffset();
                    }
                    continue;
                }
                rewritten.append(c);
                at++;
            }
            out.append(rewritten).append('\n');
        }
        return out.toString();
    }

    /**
     * A JSON value in one written form: an object's entries in the order of their names, and a
     * number as the decimal it is, {@code BigDecimal.toString} of its value at its own scale, so
     * {@code 2}, {@code 2.00} and {@code 2E+1} are three answers and not one.
     */
    private static String written(JsonNode said) {
        if (said.isObject()) {
            StringJoiner entries = new StringJoiner(",", "{", "}");
            new TreeMap<>(said.properties().stream()
                    .collect(java.util.stream.Collectors.toMap(Map.Entry::getKey,
                            Map.Entry::getValue)))
                    .forEach((name, value) -> entries.add(quoted(name) + ":" + written(value)));
            return entries.toString();
        }
        if (said.isArray()) {
            StringJoiner items = new StringJoiner(",", "[", "]");
            said.values().forEach(item -> items.add(written(item)));
            return items.toString();
        }
        if (said.isNumber()) {
            return said.decimalValue().toString();
        }
        if (said.isString()) {
            return quoted(said.stringValue());
        }
        return said.toString();
    }

    /**
     * What the JVM answered, in the same written form: a value of the model as the checker says it
     * crosses, a newtype as what it holds and a product as an object of its fields, and a number as
     * the decimal it is.
     */
    private static String written(Object said) {
        return switch (said) {
            case null -> "null";
            case Map<?, ?> map -> {
                StringJoiner entries = new StringJoiner(",", "{", "}");
                new TreeMap<>(map).forEach((name, value) ->
                        entries.add(quoted(name.toString()) + ":" + written(value)));
                yield entries.toString();
            }
            case List<?> list -> {
                StringJoiner items = new StringJoiner(",", "[", "]");
                list.forEach(item -> items.add(written(item)));
                yield items.toString();
            }
            case String text -> quoted(text);
            case Boolean truth -> truth.toString();
            case Integer number -> new BigDecimal(number).toString();
            case Long number -> new BigDecimal(number).toString();
            case BigInteger number -> new BigDecimal(number).toString();
            case BigDecimal number -> number.toString();
            case Record value -> modelValue(value);
            default -> throw new IllegalArgumentException(
                    "an answer this test has no written form for: " + said.getClass());
        };
    }

    /** A value of a type the module declares, written as the checker says it crosses. */
    private static String modelValue(Record value) {
        String name = value.getClass().getSimpleName();
        CheckedData data = Checked.of(List.of(MODULE)).modules().getFirst().data().stream()
                .filter(it -> it.name().name().equals(name))
                .findFirst()
                .orElseThrow(() -> new IllegalArgumentException(name + " is no type of held"));
        RecordComponent[] fields = value.getClass().getRecordComponents();
        try {
            return switch (data) {
                case CheckedData.Newtype it -> written(fields[0].getAccessor().invoke(value));
                case CheckedData.Product it -> {
                    StringJoiner entries = new StringJoiner(",", "{", "}");
                    TreeMap<String, Object> named = new TreeMap<>();
                    for (RecordComponent field : fields) {
                        named.put(field.getName(), field.getAccessor().invoke(value));
                    }
                    named.forEach((field, held) ->
                            entries.add(quoted(field) + ":" + written(held)));
                    yield entries.toString();
                }
                default -> throw new IllegalArgumentException(name + " is not built from fields");
            };
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    private static String quoted(String text) {
        return JSON.writeValueAsString(text);
    }
}
