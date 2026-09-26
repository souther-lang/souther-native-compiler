package souther.bindings.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * One library loaded by two bindings generated for it under two namespaces. The library is one, as
 * {@code NativeLibrary} says, and what adapts an implementation to the classes a binding generated
 * is each binding's own: an implementation is called through the adapter of the binding it was
 * written against, whichever binding was loaded last and whichever binding's function the call
 * went through.
 *
 * <p>What the implementations answer is a value of a declared type, so an implementation handed
 * across to the other binding's adapter would answer a class that adapter does not take.
 */
class APhpLibraryLoadedByTwoBindingsCallsEachOnesOwnTest {

    private static final String CATALOG = """
            module catalog exposing ( Price, priceOf )

            data Price = Int
                invariant notNegative = value >= 0

            behavior priceOf : (sku: String) -> Price
            """;

    private static final String SHOP = """
            module shop exposing ( quote )

            import catalog ( Price, priceOf )

            behavior quote : (sku: String) -> Int
                depends on priceOf
            let quote (sku, priceOf) = priceOf(sku).value * 2
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';
            require $argv[3] . '/autoload.php';

            final class ListedByA extends \\A\\Catalog\\PriceOf
            {
                public function apply(string $sku): \\A\\Catalog\\Price
                {
                    return \\A\\Catalog\\Price::of(3 * strlen($sku))->getOrThrow();
                }
            }

            final class ListedByB extends \\B\\Catalog\\PriceOf
            {
                public function apply(string $sku): \\B\\Catalog\\Price
                {
                    return \\B\\Catalog\\Price::of(5 * strlen($sku))->getOrThrow();
                }
            }

            $a = \\A\\Binding::load($argv[4]);
            $b = \\B\\Binding::load($argv[4]);

            echo "A bound: ", $a->run(fn (): int => \\A\\Shop\\Quote::bind(new ListedByA())('ab')), "\\n";
            echo "B bound: ", $b->run(fn (): int => \\B\\Shop\\Quote::bind(new ListedByB())('ab')), "\\n";

            $handedToA = \\A\\Catalog\\Injections::of(
                priceOf: fn (string $sku): \\A\\Catalog\\Price => \\A\\Catalog\\Price::of(7)->getOrThrow());
            echo "A handed: ", $a->run(fn (): int => \\A\\Shop\\Behaviors::quote('ab'), $handedToA), "\\n";

            // A function of B's binding in a run A's binding was handed an implementation for: the
            // implementation is still A's, and answers through A's adapter.
            echo "B in A's run: ", $a->run(fn (): int => \\B\\Shop\\Behaviors::quote('ab'), $handedToA),
                "\\n";

            // Both at once in one run: each bound behavior calls its own.
            echo "both: ", $a->run(fn (): string =>
                \\A\\Shop\\Quote::bind(new ListedByA())('ab') . ' '
                . \\B\\Shop\\Quote::bind(new ListedByB())('ab')), "\\n";
            """;

    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void eachBindingsImplementationIsCalledThroughItsOwnAdapter(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(
                CheckedProgram.of(List.of(CATALOG, SHOP)), into.resolve("native"));
        PhpBindings.Generated a = LibraryBinding.generated(library, into.resolve("a"), "A");
        PhpBindings.Generated b = LibraryBinding.generated(library, into.resolve("b"), "B");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        assertThat(Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), a.root().toString(), b.root().toString(),
                library.library().toString()))).isEqualTo("""
                        A bound: 12
                        B bound: 20
                        A handed: 14
                        B in A's run: 14
                        both: 12 20
                        """);
    }
}
