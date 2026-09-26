package souther.bindings.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Documents;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A PHP host is handed a function value as a {@code Closure}, which it calls with PHP values and
 * which answers one; and hands over a closure of its own where a function value is taken, which
 * the library calls. What the closure throws comes back out of the call into the library that
 * reached it, a computation that ends without a value is thrown as one, and a function value is a
 * value of the run it was handed in.
 *
 * <p>The library is built from the document the driver's own tests of function values are held to
 * ({@link Documents#FUNCTIONS}): no source publishes a function value yet.
 */
class APhpHostCallsAndHandsOverAFunctionValueTest {

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Calling\\Binding;
            use Acme\\Calling\\M\\Values;
            use Souther\\Runtime\\Expired;
            use Souther\\Runtime\\Some;
            use Souther\\Runtime\\SoutherAbort;

            function said(mixed $it): string {
                return match (true) {
                    $it instanceof Some => 'Some(' . said($it->value) . ')',
                    default => var_export($it, true),
                };
            }

            $binding = Binding::load($argv[3]);

            $binding->run(function (): void {
                $bump = Values::bump();
                echo "bump: ", $bump(3), "\\n";
                $twice = Values::twice();
                echo "twice bump: ", $twice($bump, 1), "\\n";
                echo "twice hosted: ", $twice(fn (int $x): int => $x * 3, 2), "\\n";
                try {
                    $twice(function (int $x): int {
                        throw new DomainException("refused {$x}");
                    }, 2);
                } catch (DomainException $thrown) {
                    echo "thrown: ", $thrown->getMessage(), "\\n";
                }
                try {
                    Values::overflow()(1);
                } catch (SoutherAbort $ended) {
                    echo "overflow: ", $ended->status, "\\n";
                }
                echo "pairing: ", json_encode(Values::pairing()(-4)), "\\n";
                $deep = Values::deep();
                foreach ([1, 0, -1] as $x) {
                    echo "deep {$x}: ", said($deep($x)), "\\n";
                }
                $meet = Values::meet();
                echo "meet: ", said($meet([4, 7])), " ", said($meet([4, null])), "\\n";
                echo "lifted: ", Values::lifted()(10)(1), "\\n";
            });

            $kept = $binding->run(fn (): Closure => Values::bump());
            try {
                $binding->run(fn () => $kept(1));
            } catch (Expired $expired) {
                echo "expired: ", $expired->getMessage(), "\\n";
            }
            """;

    /** Where the runtime package stands, with what Composer installed for it. */
    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void aFunctionValueIsAClosureBothWays(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                Documents.library(Documents.FUNCTIONS, into.resolve("native"));
        PhpBindings.Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Calling");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()));

        assertThat(said).isEqualTo("""
                bump: 8
                twice bump: 11
                twice hosted: 18
                thrown: refused 2
                overflow: REQUIRED_FORM_HAS_NO_PLACE
                pairing: [-4,false]
                deep 1: Some(1)
                deep 0: Some(NULL)
                deep -1: NULL
                meet: 7 NULL
                lifted: 11
                expired: a value was used after the run it was made in ended
                """);
    }

    /**
     * A function value is typed as a closure of what it takes and answers, and the library calls a
     * closure PHP hands over through one slot for each function type, made once for the binding.
     */
    @Test
    void aFunctionValueIsTypedAsAClosureOfWhatItTakesAndAnswers(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library =
                Documents.library(Documents.FUNCTIONS, into.resolve("native"));
        PhpBindings.Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Calling");

        assertThat(Files.readString(binding.root().resolve("M").resolve("Values.php")))
                .contains("@return \\Closure(\\Closure(int): int, int): int",
                        "public static function twice(): \\Closure");
        assertThat(Files.readString(binding.root().resolve("Binding.php")))
                .containsOnlyOnce("new \\Souther\\Runtime\\FunctionSlot($library, "
                        + "'souther5_m_m_fn_f1_int_int_implementation'");
        for (Path file : binding.files()) {
            if (file.toString().endsWith(".php")) {
                assertThat(Php.compiles(file)).as("%s", file).isTrue();
            }
        }
    }
}
