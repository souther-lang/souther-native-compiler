package souther.nativecode;

import net.unit8.raoh.Err;
import net.unit8.raoh.Issue;
import net.unit8.raoh.Result;
import net.unit8.raoh.decode.Decoder;
import souther.compiler.Compiler;
import souther.compiler.generated.MemoryClassLoader;
import tools.jackson.databind.DeserializationFeature;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.io.IOException;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.file.Files;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.StringJoiner;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * What an issue's metadata is to a host: each value the type Raoh holds it as, as the JVM's
 * decoder holds it. The library writes each value as its type ({@code META_TYPES}), and each host's
 * runtime makes it a value of its own Raoh; this is what the test of each host holds what its Raoh
 * hands the host to, so the type is held the whole way, to where a host reads it, and not only to
 * the JSON the library writes.
 *
 * <p>{@link #MODULE} has an issue of every type the generation's record says the metadata writes,
 * each from the documents in {@link #ROWS}, and {@link #expected} is what the JVM's
 * {@code jsonDecoder()} answers for them, in the written form every host writes too: an issue as
 * {@code [path code key=… {"name":value,…}]}, its entries by name, and each value its type, a colon
 * and its message form ({@code int:3}, {@code decimal:1.50}, {@code string:"a"}, {@code date:…},
 * {@code list:[…]}). {@link #typesIn} reads which types a written answer holds, and a host's test
 * holds that to {@link #metaTypes}, so a type added to the metadata is a type this has to reach.
 */
public final class IssueMetaContract {

    private IssueMetaContract() {
    }

    /** A document read as a type of {@link #MODULE}, under a label. */
    public record Row(String label, String type, String document) {
    }

    public static final String MODULE = """
            module meta exposing ( Counted, Price, Charge, Capped, Named, Shade, Flags, Days, Hours, \
            Stamps, Moments, Grid, Sku, Skus )

            data Counted = Int
                invariant value >= 3

            data Price = Decimal
                invariant value >= 1.5m

            data Charge = Decimal
                invariant value > 0m

            data Capped = Decimal
                invariant value <= 100m

            data Named = String
                invariant String.length(value) >= 2

            data Shade = Red | Green

            data Flags = List<Bool>
                invariant List.allDistinctBy(x -> x, value)

            data Days = List<Date>
                invariant List.allDistinctBy(x -> x, value)

            data Hours = List<Time>
                invariant List.allDistinctBy(x -> x, value)

            data Stamps = List<DateTime>
                invariant List.allDistinctBy(x -> x, value)

            data Moments = List<Instant>
                invariant List.allDistinctBy(x -> x, value)

            data Grid = List<List<Int>>
                invariant List.allDistinctBy(x -> x, value)

            data Sku = String
                invariant String.length(value) > 0

            data Skus = List<Sku>
                invariant List.allDistinctBy(x -> x, value)
            """;

    /** The types of {@link #MODULE} that are sums, which a binding may read through a codec of their own. */
    public static final Set<String> SUMS = Set.of("Shade");

    public static final List<Row> ROWS = List.of(
            new Row("int", "Counted", "2"),
            new Row("decimal", "Price", "1.49"),
            new Row("decimal of nought", "Charge", "0.00"),
            new Row("decimal of scale nought", "Capped", "100.5"),
            new Row("string", "Named", "7"),
            new Row("list of string", "Shade", "\"Blue\""),
            new Row("bool", "Flags", "[true, false, true]"),
            new Row("date", "Days", "[\"2026-01-31\", \"2026-02-01\", \"2026-01-31\"]"),
            new Row("time", "Hours", "[\"10:00\", \"10:00:30\", \"10:00:30\"]"),
            new Row("datetime", "Stamps", "[\"2026-01-31T10:00\", \"2026-01-31T10:00\"]"),
            new Row("instant", "Moments", "[\"2026-01-31T10:00:00Z\", \"2026-01-31T10:00:00Z\"]"),
            new Row("list of list", "Grid", "[[1, 2], [1, 2]]"),
            new Row("newtype", "Skus", "[\"a\", \"b\", \"a\"]"));

    /** A fraction read as the decimal it was written as, which a boundary reads a Decimal from. */
    private static final JsonMapper JSON = JsonMapper.builder()
            .enable(DeserializationFeature.USE_BIG_DECIMAL_FOR_FLOATS)
            .build();

    private static final Pattern TYPE = Pattern.compile("\\b([a-z]+):");

    /** What the JVM's decoder answers for each of {@link #ROWS}, a line each, in the written form. */
    public static String expected() {
        ClassLoader jvm = new MemoryClassLoader(Compiler.compile(MODULE),
                IssueMetaContract.class.getClassLoader());
        StringBuilder lines = new StringBuilder();
        for (Row row : ROWS) {
            lines.append(row.label()).append(": ").append(read(jvm, row)).append('\n');
        }
        return lines.toString();
    }

    private static String read(ClassLoader jvm, Row row) {
        try {
            @SuppressWarnings("unchecked")
            Decoder<JsonNode, ?> decoder = (Decoder<JsonNode, ?>) jvm.loadClass("meta." + row.type())
                    .getMethod("jsonDecoder").invoke(null);
            Result<?> result = decoder.decode(JSON.readTree(row.document()), net.unit8.raoh.Path.ROOT);
            if (!(result instanceof Err<?> refused)) {
                return "ok";
            }
            StringJoiner issues = new StringJoiner(" ");
            for (Issue issue : refused.issues().asList()) {
                String key = issue.messageKey().equals(issue.code()) ? "" : " key=" + issue.messageKey();
                issues.add("[" + issue.path().toJsonPointer() + " " + issue.code() + key + " "
                        + entries(issue.meta()) + "]");
            }
            return issues.toString();
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    private static String entries(Map<String, Object> meta) {
        StringJoiner entries = new StringJoiner(",", "{", "}");
        new TreeMap<>(meta).forEach((name, value) ->
                entries.add(JSON.writeValueAsString(name) + ":" + written(value)));
        return entries.toString();
    }

    /** A value of the JVM's metadata as its type and its message form. */
    private static String written(Object said) {
        return switch (said) {
            case Integer number -> "int:" + number;
            case Long number -> "int:" + number;
            case BigInteger number -> "int:" + number;
            case BigDecimal number -> "decimal:" + number;
            case String text -> "string:" + JSON.writeValueAsString(text);
            case Boolean truth -> "bool:" + truth;
            case java.time.LocalDate date -> "date:" + date;
            case java.time.LocalTime time -> "time:" + time;
            case java.time.LocalDateTime dateTime -> "datetime:" + dateTime;
            case java.time.Instant instant -> "instant:" + instant;
            case List<?> list -> {
                StringJoiner items = new StringJoiner(",", "list:[", "]");
                list.forEach(item -> items.add(written(item)));
                yield items.toString();
            }
            // A newtype of the model, which a host is handed as what it holds.
            case Record value when value.getClass().getRecordComponents().length == 1 -> {
                try {
                    yield written(value.getClass().getRecordComponents()[0].getAccessor().invoke(value));
                } catch (ReflectiveOperationException e) {
                    throw new IllegalStateException(e);
                }
            }
            default -> throw new IllegalArgumentException(
                    "metadata this contract has no written form for: " + said.getClass());
        };
    }

    /** The types of metadata the library writes, as the record of this generation says them. */
    public static Set<String> metaTypes() throws IOException {
        Set<String> types = new TreeSet<>();
        for (String line : Files.readAllLines(Repository.file("native", "crates", "abi", "generations",
                ManifestReader.ABI + ".txt"))) {
            if (line.startsWith("meta ")) {
                types.add(line.substring("meta ".length(), line.indexOf(':')));
            }
        }
        return types;
    }

    /** The types a written answer holds values of: each line's answer, after its label. */
    public static Set<String> typesIn(String written) {
        Set<String> types = new TreeSet<>();
        for (String line : written.split("\n")) {
            String answer = line.substring(line.indexOf(": ") + 2);
            Matcher type = TYPE.matcher(answer.replaceAll("\"(?:[^\"\\\\]|\\\\.)*\"", "\"\""));
            while (type.find()) {
                types.add(type.group(1));
            }
        }
        types.remove("key");
        return types;
    }
}
