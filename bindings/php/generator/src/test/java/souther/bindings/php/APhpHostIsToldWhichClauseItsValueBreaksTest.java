package souther.bindings.php;

import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;
import souther.nativecode.Php;
import souther.nativecode.Repository;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value a PHP host builds with {@code of} and a clause refuses comes back as the issue the type's
 * {@code decode} answers for the same value: the constraint a clause is, with what Raoh's own
 * constraint says of it, or the type and the clause. So which rule the value broke reaches a host
 * whichever way it built the value, and of a type with two clauses it says which.
 */
class APhpHostIsToldWhichClauseItsValueBreaksTest {

    private static final String MEMBERS = """
            module members exposing ( MemberId, Title, Line )

            data MemberId = String
                invariant shaped = String.matches("M[0-9]{5}", value)

            data Title = String
                invariant named = String.length(String.trim(value)) >= 1

            data Line = { quantity: Int, price: Int }
                invariant counted = quantity >= 1
                invariant priced = price >= 0
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Shop\\Binding;
            use Acme\\Shop\\Members\\Line;
            use Acme\\Shop\\Members\\MemberId;
            use Acme\\Shop\\Members\\Title;
            use Raoh\\Issue;
            use Raoh\\Result;

            function said(Result $result): string {
                return $result->fold(
                    fn (mixed $value): string => 'not refused',
                    fn ($issues): string => implode(' ', array_map(
                        function (Issue $it): string {
                            $meta = $it->meta;
                            ksort($meta, SORT_STRING);
                            $entries = [];
                            foreach ($meta as $name => $value) {
                                $entries[] = $name . '=' . $value;
                            }
                            return '[' . $it->path->toJsonPointer() . ' ' . $it->code . ' '
                                . implode(' ', $entries) . ']';
                        },
                        $issues->toArray())));
            }

            function compared(string $what, Result $built, Result $read): void {
                echo $what, ': ', said($built), ' ', said($built) === said($read) ? 'true' : 'false',
                    "\\n";
            }

            Binding::load($argv[3])->run(function (): void {
                compared('shaped', MemberId::of('x1'), MemberId::decode('"x1"'));
                compared('named', Title::of('  '), Title::decode('"  "'));
                compared('counted', Line::of(0, 1), Line::decode('{"quantity":0,"price":1}'));
                compared('priced', Line::of(1, -1), Line::decode('{"quantity":1,"price":-1}'));
                echo 'held: ', MemberId::of('M12345')->getOrThrow()->encode(), "\\n";
            });
            """;

    /** Where the runtime package stands, with what Composer installed for it. */
    private static final Path RUNTIME = Repository.file("bindings", "php", "runtime");

    @Test
    void aConstructionIsRefusedWithTheIssueItsDecoderAnswers(@TempDir Path into) throws Exception {
        TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), MEMBERS);
        Generated binding = LibraryBinding.generated(library, into.resolve("php"), "Acme\\Shop");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()));

        assertThat(said).isEqualTo("""
                shaped: [ invalid_format pattern=M[0-9]{5}] true
                named: [ invariant_violation clause=named module=members type=Title] true
                counted: [ invariant_violation clause=counted module=members type=Line] true
                priced: [ invariant_violation clause=priced module=members type=Line] true
                held: "M12345"
                """);
    }
}
