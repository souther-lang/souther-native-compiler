package souther.bindings.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;
import souther.nativecode.Generated;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;
import souther.nativecode.Repository;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;
import tools.jackson.databind.node.ArrayNode;
import tools.jackson.databind.node.ObjectNode;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A PHP host holds its own implementation of a behavior it implements to the behavior's {@code
 * example} rows: the class generated for the behavior lists the rows, as a PHPUnit data provider
 * would hand them to a test, and evaluates one of them against an instance in the run the host
 * opened.
 *
 * <p>Whether an answer keeps a row is the library's to say, and the rows are chosen where a
 * comparison of the external form would say otherwise: a {@code Decimal} stated at one scale and
 * answered at another, and a case stated with nothing under it and answered with something. Beside
 * those, a row written with no name, one whose answer is owed, which an answer neither keeps nor
 * fails to keep, and an implementation that throws, whose row ended without an answer and did not
 * fail to be kept. What the host prints of each is what the outcome is and the answer's external
 * form, which is there to be read and compared with nothing.
 */
class APhpHostHoldsItsImplementationToABehaviorsRowsTest {

    private static final String SHELF = """
            module shelf exposing ( Label, Priced, Unpriced, labelOf, stockOf )

            data Priced = { amount: Decimal }
            data Unpriced
            data Label = Priced | Unpriced

            behavior labelOf : (sku: Int) -> Label
            behavior stockOf : (sku: Int) -> Int

            example labelOf
                | "a price at the scale it was written" : (1) -> Priced { amount = 1.50m }
                | (2) -> Unpriced
                | "priced, whatever it costs" : (3) -> Priced
                | "not labelled yet" : (4) -> <?>

            example stockOf
                | "none on the shelf" : (1) -> 0
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Shelf\\Binding;
            use Acme\\Shelf\\Shelf\\Label;
            use Acme\\Shelf\\Shelf\\LabelOf;
            use Acme\\Shelf\\Shelf\\Priced;
            use Acme\\Shelf\\Shelf\\StockOf;
            use Acme\\Shelf\\Shelf\\Unpriced;
            use Souther\\Runtime\\Decimal;
            use Souther\\Runtime\\OutsideAnyRun;
            use Souther\\Runtime\\Row;
            use Souther\\Runtime\\RowAnswered;
            use Souther\\Runtime\\RowEnded;
            use Souther\\Runtime\\RowKept;
            use Souther\\Runtime\\RowNotKept;
            use Souther\\Runtime\\RowNotRun;
            use Souther\\Runtime\\RowOutcome;

            /** A shelf answering a price for each sku it holds one for, and no price for the rest. */
            final class Shelved extends LabelOf
            {
                /** @param array<int, ?Decimal> $prices */
                public function __construct(private readonly array $prices)
                {
                }

                public function apply(int $sku): Label
                {
                    $price = $this->prices[$sku] ?? null;
                    return $price === null ? Unpriced::of()->getOrThrow()
                        : Priced::of($price)->getOrThrow();
                }
            }

            final class Missing extends LabelOf
            {
                public static ?\\RuntimeException $thrown = null;

                public function apply(int $sku): Label
                {
                    throw self::$thrown = new \\RuntimeException("no shelf holds {$sku}");
                }
            }

            function said(Row $row, RowOutcome $outcome): string
            {
                $what = match (true) {
                    $outcome instanceof RowKept => "kept {$outcome->answer}",
                    $outcome instanceof RowNotKept => "not kept {$outcome->answer}",
                    $outcome instanceof RowAnswered => "answered {$outcome->answer}",
                    $outcome instanceof RowEnded => 'ended ' . $outcome->reason::class . ' '
                        . ($outcome->reason === Missing::$thrown ? 'as thrown' : 'otherwise'),
                    $outcome instanceof RowNotRun => "not run: {$outcome->why}",
                };
                return "{$row->at} {$row->shown}: {$what}";
            }

            $binding = Binding::load($argv[3]);

            foreach (LabelOf::examples() as $key => [$row]) {
                echo "row '{$key}': {$row->kind->name}, ",
                    $row->name === null ? 'unnamed' : "named '{$row->name}'", "\\n";
            }

            $hosts = [
                'right' => new Shelved([1 => new Decimal('15', 1), 3 => new Decimal('999', 2),
                    4 => new Decimal('4', 0)]),
                'wrong' => new Shelved([1 => new Decimal('151', 2), 2 => new Decimal('2', 0)]),
                'missing' => new Missing(),
            ];
            foreach ($hosts as $name => $host) {
                foreach (LabelOf::examples() as [$row]) {
                    echo "{$name} ", said($row, $binding->run(fn (): RowOutcome =>
                        LabelOf::evaluate($host, $row))), "\\n";
                }
            }

            // A row is evaluated in a run, and against the behavior it is a row of.
            $first = LabelOf::examples()['a price at the scale it was written'][0];
            try {
                LabelOf::evaluate($hosts['right'], $first);
            } catch (OutsideAnyRun $outside) {
                echo "outside any run: ", $outside::class, "\\n";
            }
            try {
                $binding->run(fn (): RowOutcome => StockOf::evaluate(new class extends StockOf {
                    public function apply(int $sku): int
                    {
                        return 0;
                    }
                }, $first));
            } catch (\\InvalidArgumentException $other) {
                echo "another behavior's row: ", $other::class, "\\n";
            }
            """;

    private static final String HELD = """
            row 'a price at the scale it was written': Asserted, named 'a price at the scale it was written'
            row 'row 2': Asserted, unnamed
            row 'priced, whatever it costs': Asserted, named 'priced, whatever it costs'
            row 'not labelled yet': Owed, named 'not labelled yet'
            right 0 a price at the scale it was written: kept {"type":"Priced","amount":1.5}
            right 1 row 2: kept {"type":"Unpriced"}
            right 2 priced, whatever it costs: kept {"type":"Priced","amount":9.99}
            right 3 not labelled yet: answered {"type":"Priced","amount":4}
            wrong 0 a price at the scale it was written: not kept {"type":"Priced","amount":1.51}
            wrong 1 row 2: not kept {"type":"Priced","amount":2}
            wrong 2 priced, whatever it costs: not kept {"type":"Unpriced"}
            wrong 3 not labelled yet: answered {"type":"Unpriced"}
            missing 0 a price at the scale it was written: ended RuntimeException as thrown
            missing 1 row 2: ended RuntimeException as thrown
            missing 2 priced, whatever it costs: ended RuntimeException as thrown
            missing 3 not labelled yet: ended RuntimeException as thrown
            outside any run: Souther\\Runtime\\OutsideAnyRun
            another behavior's row: InvalidArgumentException
            """;

    private static final Path RUNTIME = Repository.file("bindings", "php", "runtime");

    private static final JsonMapper JSON = JsonMapper.builder().build();

    @Test
    void aHostsImplementationIsHeldToEachRowAsTheLibraryHoldsIt(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHELF)), into.resolve("native"));
        Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Shelf");

        assertThat(ran(into, binding, library, HOST)).isEqualTo(HELD);
    }

    /**
     * A row with nothing to run is listed all the same, and evaluating it answers why and calls
     * nothing: one the compile could not read, and one stating what the library has no way to hold
     * an answer to. Neither is a row this model has, so the manifest says so of two it does.
     */
    @Test
    void aRowWithNothingToRunIsListedAndSaysWhy(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHELF)), into.resolve("native"));
        ObjectNode manifest = (ObjectNode) JSON.readTree(library.manifest().toFile());
        for (JsonNode injection : manifest.get("modules").get(0).get("injections")) {
            if (injection.get("name").stringValue().equals("labelOf")) {
                ArrayNode examples = (ArrayNode) injection.get("examples");
                ((ObjectNode) examples.get(0)).set("row", JSON.readTree(
                        "{\"asserted\": {\"unavailable\": {\"reason\": \"no_comparison\","
                                + " \"path\": []}}}"));
                ((ObjectNode) examples.get(2)).set("row", JSON.readTree(
                        "{\"unreproducible\": \"the row's input is not a value\"}"));
            }
        }
        Path changed = into.resolve("changed.json");
        Files.writeString(changed, JSON.writeValueAsString(manifest), StandardCharsets.UTF_8);
        Generated binding = LibraryBinding.generated(changed, library.declarations(),
                into.resolve("php"), "Acme\\Shelf");

        assertThat(ran(into, binding, library, """
                <?php
                declare(strict_types=1);

                require $argv[1] . '/vendor/autoload.php';
                require $argv[2] . '/autoload.php';

                use Acme\\Shelf\\Binding;
                use Acme\\Shelf\\Shelf\\Label;
                use Acme\\Shelf\\Shelf\\LabelOf;
                use Souther\\Runtime\\RowNotRun;
                use Souther\\Runtime\\RowOutcome;

                $binding = Binding::load($argv[3]);
                $never = new class extends LabelOf {
                    public function apply(int $sku): Label
                    {
                        throw new \\LogicException('called for a row with nothing to run');
                    }
                };
                foreach (LabelOf::examples() as [$row]) {
                    if ($row->why === null) {
                        continue;
                    }
                    $outcome = $binding->run(fn (): RowOutcome => LabelOf::evaluate($never, $row));
                    echo "{$row->shown}: {$row->kind->name}, ", $outcome instanceof RowNotRun
                        ? "not run: {$outcome->why}" : 'run', "\\n";
                }
                """)).isEqualTo("""
                a price at the scale it was written: NotComparable, not run: the library has no way to compare an answer with what the row states
                priced, whatever it costs: Unreproducible, not run: the row's input is not a value
                """);
    }

    private static String ran(Path into, Generated binding, NativeCompiler.Library library,
                              String host) throws Exception {
        Path script = into.resolve("host.php");
        Files.writeString(script, host, StandardCharsets.UTF_8);
        return Php.ran(List.of("-d", "ffi.enable=1", script.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()));
    }
}
