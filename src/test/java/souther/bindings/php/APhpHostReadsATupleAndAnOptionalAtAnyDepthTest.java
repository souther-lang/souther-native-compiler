package souther.bindings.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A PHP host is handed a tuple as a PHP list of its members, a list of tuples as a list of those,
 * and an optional of an optional as null, or a {@code Some} of what the inner one is: an optional
 * whose value may be null holds it in a {@code Some}, so holding nothing is told apart at each
 * depth.
 */
class APhpHostReadsATupleAndAnOptionalAtAnyDepthTest {

    private static final String SHAPE = """
            module shape exposing ( Line, pair, noted, blank, beyond, pairs )

            data Line = { quantity: Int, note: String? }

            let pair: (Int, Bool) = (3, true)

            let noted = List.get(0, [Line { quantity = 1, note = "gift" }.note])

            let blank = List.get(0, [Line { quantity = 1, note = None }.note])

            let beyond = List.get(1, [Line { quantity = 1, note = None }.note])

            let pairs = [(1, "a"), (2, "b")]
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Shape\\Binding;
            use Acme\\Shape\\Shape\\Values;
            use Souther\\Runtime\\Some;

            function said(mixed $it): string {
                return match (true) {
                    $it instanceof Some => 'Some(' . said($it->value) . ')',
                    default => var_export($it, true),
                };
            }

            Binding::load($argv[3])->run(function (): void {
                echo "pair: ", json_encode(Values::pair()), "\\n";
                echo "noted: ", said(Values::noted()), "\\n";
                echo "blank: ", said(Values::blank()), "\\n";
                echo "beyond: ", said(Values::beyond()), "\\n";
                echo "pairs: ", json_encode(Values::pairs()), "\\n";
            });
            """;

    /** Where the runtime package stands, with what Composer installed for it. */
    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void aTupleAndAnOptionalAtAnyDepthAreReadAsPhpHoldsThem(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHAPE)), into.resolve("native"));
        PhpBindings.Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Shape");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()));

        assertThat(said).isEqualTo("""
                pair: [3,true]
                noted: Some('gift')
                blank: Some(NULL)
                beyond: NULL
                pairs: [[1,"a"],[2,"b"]]
                """);
    }

    /**
     * What a docblock says of each is what PHP holds: a tuple is an array of its members at their
     * places, and an optional of an optional a {@code Some} of the inner one or null.
     */
    @Test
    void eachIsTypedAsWhatPhpHolds(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHAPE)), into.resolve("native"));
        PhpBindings.Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Shape");

        assertThat(Files.readString(binding.root().resolve("Shape").resolve("Values.php")))
                .contains("@return array{0: int, 1: bool}", "public static function pair(): array",
                        "@return \\Souther\\Runtime\\Some<?string>|null",
                        "public static function noted(): ?\\Souther\\Runtime\\Some",
                        "@return list<array{0: int, 1: string}>");
        for (Path file : binding.files()) {
            if (file.toString().endsWith(".php")) {
                assertThat(Php.compiles(file)).as("%s", file).isTrue();
            }
        }
    }
}
