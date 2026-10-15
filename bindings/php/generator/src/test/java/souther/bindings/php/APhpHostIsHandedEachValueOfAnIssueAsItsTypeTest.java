package souther.bindings.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;
import souther.nativecode.Generated;
import souther.nativecode.IssueMetaContract;
import souther.nativecode.Php;
import souther.nativecode.Repository;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.StringJoiner;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A PHP host is handed each value of an issue's metadata as a value of the type raoh-php holds it
 * as, as the JVM's decoder holds it: what the host reads off its {@code Raoh\Issue} is held to what
 * the JVM answers ({@link IssueMetaContract}), and holds a value of every type the library writes.
 * The library writing each value as its type is held elsewhere; this is the other half, the runtime
 * making it a PHP value, where a type it did not keep would be lost to the host.
 */
class APhpHostIsHandedEachValueOfAnIssueAsItsTypeTest {

    private static final Path RUNTIME = Repository.file("bindings", "php", "runtime");

    private static String host() {
        StringJoiner rows = new StringJoiner("\n");
        StringJoiner rendered = new StringJoiner("\n");
        for (IssueMetaContract.Row row : IssueMetaContract.ROWS) {
            String reader = row.type() + (IssueMetaContract.SUMS.contains(row.type()) ? "Codec" : "");
            rows.add("    $out[] = '%s: ' . said(from_json(\\Acme\\Shop\\Meta\\%s::decoder())->decode('%s'));"
                    .formatted(row.label(), reader, row.document()));
            rendered.add("    $out[] = '%s: ' . render(from_json(\\Acme\\Shop\\Meta\\%s::decoder())->decode('%s'));"
                    .formatted(row.label(), reader, row.document()));
        }
        return """
                <?php
                declare(strict_types=1);

                require $argv[1] . '/vendor/autoload.php';
                require $argv[2] . '/autoload.php';

                use Acme\\Shop\\Binding;
                use Raoh\\Issue;
                use Raoh\\Result;
                use Raoh\\Value\\Decimal;
                use Raoh\\Value\\Temporal\\Instant;
                use Raoh\\Value\\Temporal\\LocalDate;
                use Raoh\\Value\\Temporal\\LocalDateTime;
                use Raoh\\Value\\Temporal\\LocalTime;

                use function Raoh\\Boundary\\Json\\from_json;

                function written(mixed $value): string {
                    return match (true) {
                        is_int($value) => 'int:' . $value,
                        $value instanceof Decimal => 'decimal:' . $value,
                        is_string($value) => 'string:' . json_encode($value, JSON_UNESCAPED_UNICODE),
                        is_bool($value) => 'bool:' . ($value ? 'true' : 'false'),
                        $value instanceof LocalDate => 'date:' . $value,
                        $value instanceof LocalTime => 'time:' . $value,
                        $value instanceof LocalDateTime => 'datetime:' . $value,
                        $value instanceof Instant => 'instant:' . $value,
                        is_array($value) && array_is_list($value)
                            => 'list:[' . implode(',', array_map(written(...), $value)) . ']',
                        default => throw new \\LogicException(
                            'metadata of no type the library writes: ' . get_debug_type($value)),
                    };
                }

                function said(Result $result): string {
                    return $result->fold(
                        fn (mixed $value): string => 'ok',
                        fn ($issues): string => implode(' ', array_map(
                            function (Issue $it): string {
                                $meta = $it->meta;
                                ksort($meta, SORT_STRING);
                                $entries = [];
                                foreach ($meta as $name => $value) {
                                    $entries[] = json_encode((string) $name) . ':' . written($value);
                                }
                                return '[' . $it->path->toJsonPointer() . ' ' . $it->code
                                    . ($it->messageKey === $it->code ? '' : ' key=' . $it->messageKey)
                                    . ' {' . implode(',', $entries) . '}]';
                            },
                            $issues->toArray())));
                }

                function render(Result $result): string {
                    return $result->fold(
                        fn (mixed $value): string => 'ok',
                        fn ($issues): string => implode(' ', array_map(
                            function (array $it): string {
                                $meta = $it['meta'];
                                ksort($meta, SORT_STRING);
                                return '[' . $it['path'] . ' ' . $it['code'] . ' '
                                    . json_encode((object) $meta, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE)
                                    . ']';
                            },
                            $issues->toJsonList())));
                }

                echo Binding::load($argv[3])->run(function (): string {
                    $out = [];
                %s
                    $out[] = '--';
                %s
                    return implode("\\n", $out) . "\\n";
                });
                """.formatted(rows, rendered);
    }

    @Test
    void eachValueIsTheTypeTheJvmHoldsItAs(@TempDir Path into) throws Exception {
        TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), IssueMetaContract.MODULE);
        Generated binding = LibraryBinding.generated(library, into.resolve("php"), "Acme\\Shop");
        Path host = into.resolve("host.php");
        Files.writeString(host, host(), StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()));

        assertThat(said).isEqualTo(IssueMetaContract.answered(IssueMetaContract.DecimalWritten.STRING));
        assertThat(IssueMetaContract.typesIn(said)).isEqualTo(IssueMetaContract.metaTypes());
    }
}
