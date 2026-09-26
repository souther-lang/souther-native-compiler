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
 * A PHP host hands the library a {@code Decimal} as its integer and its scale, and is handed one
 * back the same way, wherever a {@code Decimal} stands: what a behavior takes and answers, a field,
 * an element of a list, an optional, a case of a union, and what a behavior the host implements is
 * handed and answers.
 *
 * <p>The scale crosses as it is, both ways, so {@code 1.50} comes back at scale 2; what a boundary
 * writes is the amount, so the same value is written {@code 1.5} in JSON.
 */
class APhpHostHandsOverAndReadsBackADecimalTest {

    private static final String PRICING = """
            module pricing exposing ( Priced, Discount, Free, taxed, total, rated, orNought, charged )

            data Priced = { amount: Decimal, note: String }

            data Discount = { rate: Decimal? }

            data Free

            behavior taxed : (amount: Decimal, rate: Decimal) -> Decimal
            let taxed (amount, rate) = Decimal.round(2, HALF_UP, amount * rate)

            behavior total : (xs: List<Decimal>) -> Decimal
            let total (xs) = List.sum(xs)

            behavior rated : (amount: Decimal) -> Decimal | Free
            let rated (amount) = if amount == 0m then Free else amount

            behavior orNought : (d: Discount) -> Decimal
            let orNought (d) = match d.rate with
                | Some x -> x
                | None -> 0m

            behavior rateFor : (amount: Decimal) -> Decimal

            behavior charged : (amount: Decimal) -> Decimal
                depends on rateFor
            let charged (amount, rateFor) = amount * rateFor(amount)
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Pricing\\Binding;
            use Acme\\Pricing\\Pricing\\Behaviors;
            use Acme\\Pricing\\Pricing\\Discount;
            use Acme\\Pricing\\Pricing\\Free;
            use Acme\\Pricing\\Pricing\\Injections;
            use Acme\\Pricing\\Pricing\\Priced;
            use Souther\\Runtime\\Decimal;

            function said(Decimal $d): string {
                return $d->unscaled . ' at ' . $d->scale;
            }

            $binding = Binding::load($argv[3]);

            $binding->run(function (): void {
                echo "taxed: ", said(Behaviors::taxed(new Decimal('1999', 2), new Decimal('75', 3))), "\\n";
                $priced = Priced::of(new Decimal('-150', 2), 'x')->getOrThrow();
                echo "priced: ", said($priced->amount()), ", written ", $priced->encode(), "\\n";
                echo "read: ", said(Priced::decode('{"amount":1e2,"note":"y"}')->getOrThrow()->amount()), "\\n";
                echo "total: ", said(Behaviors::total([new Decimal('15', 1), new Decimal('225', 2)])), "\\n";
                $free = Behaviors::rated(new Decimal('0', 5));
                $half = Behaviors::rated(new Decimal('5', 1));
                echo "rated: ", var_export($free instanceof Free, true), ", ",
                    $half instanceof Decimal ? said($half) : 'not a Decimal', "\\n";
                $none = Discount::of(null)->getOrThrow();
                $some = Discount::of(new Decimal('7', 3))->getOrThrow();
                echo "or nought: ", said(Behaviors::orNought($none)), ", ",
                    said(Behaviors::orNought($some)), ", read ", said($some->rate()), " ",
                    var_export($none->rate(), true), "\\n";
            });

            $rates = Injections::of(rateFor: fn (Decimal $amount): Decimal => new Decimal('5', 2));
            echo "charged: ", said($binding->run(
                fn (): Decimal => Behaviors::charged(new Decimal('200', 0)), $rates)), "\\n";

            echo "leading zeros: ", said(new Decimal('-007', 1)), "\\n";
            foreach ([['1.5', 0], ['', 0], ['7', 2147483648]] as [$unscaled, $scale]) {
                try {
                    new Decimal($unscaled, $scale);
                } catch (InvalidArgumentException $refused) {
                    echo "refused: ", $refused->getMessage(), "\\n";
                }
            }
            """;

    private static final String ANSWERED = """
            taxed: 150 at 2
            priced: -150 at 2, written {"amount":-1.5,"note":"x"}
            read: 1 at -2
            total: 375 at 2
            rated: true, 5 at 1
            or nought: 0 at 0, 7 at 3, read 7 at 3 NULL
            charged: 1000 at 2
            leading zeros: -7 at 1
            refused: a Decimal's integer is an optional '-' and ASCII digits, and not '1.5'
            refused: a Decimal's integer is an optional '-' and ASCII digits, and not ''
            refused: a Decimal's scale is a 32-bit number, and not 2147483648
            """;

    /** Where the runtime package stands, with what Composer installed for it. */
    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void aDecimalCrossesAsItsIntegerAndItsScaleWhereverItStands(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(PRICING)), into.resolve("native"));
        PhpBindings.Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Pricing");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()));

        assertThat(said).isEqualTo(ANSWERED);
        for (Path file : binding.files()) {
            if (file.toString().endsWith(".php")) {
                assertThat(Php.compiles(file)).as("%s", file).isTrue();
            }
        }
    }
}
