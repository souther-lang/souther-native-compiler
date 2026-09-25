package souther.nativecode.php;

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
 * A PHP host holds a behavior the way the JVM backend's host does: it implements a behavior the
 * model asks a host for as a class extending the one generated for it, binds a behavior requiring
 * it to an instance, and calls {@code apply}.
 *
 * <p>What binding one checks is what PHP's types check: a missing implementation is a
 * {@code TypeError} at {@code bind}, and never an {@code UnboundInjection} when the behavior is
 * called. What is bound is registered for the length of one call in the caller's run, so the value
 * {@code apply} answers is the run's, and what the run registered is registered again after it.
 */
class APhpHostBindsABehaviorToWhatItRequiresTest {

    private static final String CATALOG = """
            module catalog exposing ( Price, priceOf )

            data Price = Int
                invariant notNegative = value >= 0

            behavior priceOf : (sku: String) -> Price
            """;

    /** A second behavior called `priceOf`, which a composition requires beside the catalog's. */
    private static final String WHOLESALE = """
            module wholesale exposing ( priceOf )

            import catalog ( Price )

            behavior priceOf : (price: Price) -> Int
            """;

    private static final String SHOP = """
            module shop exposing ( Line, quote, total, both, twice, priced : Int, resold : Int,
                                   lineOf )

            import catalog ( Price, priceOf )
            import wholesale

            behavior discountFor : (sku: String) -> Int

            behavior quote : (sku: String, count: Int) -> Int
                depends on priceOf, discountFor
            let quote (sku, count, priceOf, discountFor) =
                priceOf(sku).value * count - discountFor(sku)

            behavior total : (sku: String) -> Int
                depends on quote
            let total (sku, quote) = quote(sku, 2) + 1

            behavior both : (sku: String) -> Int
                depends on quote, priceOf
            let both (sku, quote, priceOf) = quote(sku, 1) + priceOf(sku).value

            behavior twice : (n: Int) -> Int
            let twice (n) = n * 2

            behavior valued : (price: Price) -> Int
            let valued (price) = price.value * 10

            behavior priced = priceOf >-> valued

            behavior resold = catalog.priceOf >-> wholesale.priceOf

            data Line = { sku: String, amount: Int }

            behavior lineOf : (sku: String) -> Line
                depends on priceOf
            let lineOf (sku, priceOf) = Line { sku = sku, amount = priceOf(sku).value }
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Billing\\Binding;
            use Acme\\Billing\\Catalog\\Injections as CatalogInjections;
            use Acme\\Billing\\Catalog\\Price;
            use Acme\\Billing\\Catalog\\PriceOf;
            use Acme\\Billing\\Shop\\Behaviors;
            use Acme\\Billing\\Shop\\Both;
            use Acme\\Billing\\Shop\\DiscountFor;
            use Acme\\Billing\\Shop\\Injections as ShopInjections;
            use Acme\\Billing\\Shop\\Priced;
            use Acme\\Billing\\Shop\\Quote;
            use Acme\\Billing\\Shop\\Total;
            use Acme\\Billing\\Shop\\Twice;
            use Acme\\Billing\\Shop\\Line;
            use Acme\\Billing\\Shop\\LineOf;
            use Acme\\Billing\\Shop\\Resold;
            use Acme\\Billing\\Wholesale\\PriceOf as WholesalePriceOf;
            use Souther\\Runtime\\Expired;
            use Souther\\Runtime\\NotTheInnermostRun;
            use Souther\\Runtime\\Session;
            use Souther\\Runtime\\UnboundInjection;

            /** A price list with a dependency of its own, as a container would wire one. */
            final class ListedPrice extends PriceOf
            {
                public function __construct(private readonly int $each)
                {
                }

                public function apply(Session $session, string $sku): Price
                {
                    return Price::of($session, $this->each * strlen($sku))->getOrThrow();
                }
            }

            final class Off extends DiscountFor
            {
                public function __construct(private readonly int $by)
                {
                }

                public function apply(Session $session, string $sku): int
                {
                    return $this->by;
                }
            }

            final class MarkedUp extends WholesalePriceOf
            {
                public function apply(Session $session, Price $price): int
                {
                    return $price->value() + 1;
                }
            }

            $binding = Binding::load($argv[3]);
            $prices = new ListedPrice(3);
            $quote = Quote::bind($prices, new Off(1));

            echo "quote: ", $binding->run(fn (Session $s): int => $quote->apply($s, 'ab', 2)), "\\n";
            echo "total: ", $binding->run(fn (Session $s): int => Total::bind($quote)->apply($s, 'ab')), "\\n";
            echo "both: ", $binding->run(fn (Session $s): int => Both::bind($quote, $prices)->apply($s, 'ab')), "\\n";
            echo "twice: ", $binding->run(fn (Session $s): int => Twice::of()->apply($s, 4)), "\\n";
            echo "priced: ", $binding->run(fn (Session $s): int => Priced::bind($prices)->apply($s, 'abc')), "\\n";

            // Two requirements of one name, from two modules, taken by their places.
            echo "resold: ", $binding->run(fn (Session $s): int =>
                Resold::bind(dependency0: $prices, dependency1: new MarkedUp())->apply($s, 'ab')), "\\n";

            // What apply answers is a value of the caller's run: read after apply has put back what
            // it registered, and refused once the run has ended.
            echo "line: ", $binding->run(function (Session $s) use ($prices): string {
                $line = LineOf::bind($prices)->apply($s, 'abc');
                return $line->sku() . ' at ' . $line->amount();
            }), "\\n";
            $kept = $binding->run(fn (Session $s): Line => LineOf::bind($prices)->apply($s, 'abc'));
            try {
                $kept->amount();
            } catch (Expired $expired) {
                echo "line after its run: ", $expired->getMessage(), "\\n";
            }

            // What an implementation answers is a value of the caller's run too.
            echo "held: ", $binding->run(function (Session $s) use ($prices): int {
                $price = new class ($prices) extends PriceOf {
                    public function __construct(private readonly PriceOf $listed)
                    {
                    }

                    public function apply(Session $session, string $sku): Price
                    {
                        return $this->listed->apply($session, $sku . $sku);
                    }
                };
                $answered = $price->apply($s, 'ab');
                return Priced::bind($price)->apply($s, 'ab') + $answered->value();
            }), "\\n";

            // Registered for the call and not after it: the run's own are registered again.
            echo "after: ", $binding->run(function (Session $s) use ($quote): string {
                $bound = $quote->apply($s, 'ab', 1);
                $own = Behaviors::quote($s, 'ab', 1);
                return "{$bound} then {$own}";
            }, CatalogInjections::of(priceOf: fn (Session $s, string $sku): Price =>
                Price::of($s, 100)->getOrThrow()),
                ShopInjections::of(discountFor: fn (Session $s, string $sku): int => 0)), "\\n";
            try {
                $binding->run(function (Session $s) use ($quote): int {
                    $quote->apply($s, 'ab', 1);
                    return Behaviors::quote($s, 'ab', 1);
                });
            } catch (UnboundInjection $unbound) {
                echo "unbound after: ", $unbound::class, "\\n";
            }

            // An implementation is called in the innermost run, and may call a bound behavior in it.
            $nested = Quote::bind($prices, new class ($quote) extends DiscountFor {
                public function __construct(private readonly Quote $inner)
                {
                }

                public function apply(Session $session, string $sku): int
                {
                    return $this->inner->apply($session, $sku, 1);
                }
            });
            echo "nested: ", $binding->run(fn (Session $s): int => $nested->apply($s, 'ab', 2)), "\\n";

            try {
                Quote::bind($prices);
            } catch (TypeError $missing) {
                echo "missing: ", $missing::class, "\\n";
            }
            try {
                Quote::bind(new Off(1), $prices);
            } catch (TypeError $wrong) {
                echo "wrong: ", $wrong::class, "\\n";
            }
            try {
                Both::bind($quote, new ListedPrice(5));
            } catch (InvalidArgumentException $two) {
                echo "two: ", $two->getMessage(), "\\n";
            }
            echo "same twice: ", $binding->run(fn (Session $s): int =>
                Both::bind(Quote::bind($prices, new Off(0)), $prices)->apply($s, 'a')), "\\n";

            $binding->run(function (Session $outer) use ($binding): void {
                try {
                    $binding->run(fn (Session $inner): int => Twice::of()->apply($outer, 1));
                } catch (NotTheInnermostRun $refused) {
                    echo "outer in inner: ", $refused::class, "\\n";
                }
            });
            """;

    private static final String ANSWERED = """
            quote: 11
            total: 12
            both: 11
            twice: 8
            priced: 90
            resold: 7
            line: abc at 9
            line after its run: a value was used after the run it was made in ended
            held: 132
            after: 5 then 100
            unbound after: Souther\\Runtime\\UnboundInjection
            nested: 7
            missing: ArgumentCountError
            wrong: TypeError
            two: catalog.priceOf is bound to two implementations, and the library calls one implementation of it at a time
            same twice: 6
            outer in inner: Souther\\Runtime\\NotTheInnermostRun
            """;

    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void aHostBindsABehaviorToClassesImplementingWhatItRequires(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(
                CheckedProgram.of(List.of(CATALOG, WHOLESALE, SHOP)), into.resolve("native"));
        PhpBindings.Generated binding =
                PhpBindings.generate(library, into.resolve("php"), "Acme\\Billing");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        assertThat(Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()))).isEqualTo(ANSWERED);
    }
}
