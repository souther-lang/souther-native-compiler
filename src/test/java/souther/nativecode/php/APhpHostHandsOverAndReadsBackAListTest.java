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
 * A PHP host hands the library a list as a PHP list and is handed one back the same way, wherever
 * a list stands: a field it builds a value with and reads back, what a behavior takes and answers,
 * and what a behavior the host implements is handed and answers inside a value.
 *
 * <p>The cart #58 ports is the model: an order's lines, and a priced cart the host prices, which
 * the library reads the lines of. An element is handed over as a value of its type is anywhere
 * else, so an optional element, a list of lists and the empty list need nothing of their own, and
 * an element whose run has ended, or another library's, is refused as it would be on its own.
 */
class APhpHostHandsOverAndReadsBackAListTest {

    private static final String CART = """
            module cart exposing ( OrderLine, Order, PricedCart, Notes, Grid, totalOf, echoed,
                                   checkout, noted )

            data OrderLine = { sku: String, quantity: Int, unitPrice: Int }
                invariant quantity >= 1

            data Order = { lines: List<OrderLine> }

            data PricedCart = { lines: List<OrderLine>, total: Int }
                invariant List.length(lines) >= 1

            data Notes = { said: List<Option<String>> }

            data Grid = { rows: List<List<Int>> }

            partial let sumFrom (acc: Int, xs: List<OrderLine>, i: Int): Int =
                match List.get(i, xs) with
                    | Some x -> sumFrom(acc + x.quantity * x.unitPrice, xs, i + 1)
                    | None -> acc

            behavior totalOf : (order: Order) -> Int
            let totalOf (order) = sumFrom(0, order.lines, 0)

            behavior echoed : (lines: List<OrderLine>) -> List<OrderLine>
            let echoed (lines) = lines

            behavior priceCart : (order: Order) -> PricedCart

            behavior checkout : (order: Order) -> Int
                depends on priceCart
            let checkout (order, priceCart) = {
                let priced = priceCart(order)
                sumFrom(0, priced.lines, 0) * 1000 + priced.total
            }

            behavior countNotes : (lines: List<OrderLine>) -> Int

            behavior noted : (order: Order) -> Int
                depends on countNotes
            let noted (order, countNotes) = countNotes(order.lines)
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Billing\\Binding;
            use Acme\\Billing\\Cart\\Behaviors;
            use Acme\\Billing\\Cart\\Grid;
            use Acme\\Billing\\Cart\\Injections;
            use Acme\\Billing\\Cart\\Notes;
            use Acme\\Billing\\Cart\\Order;
            use Acme\\Billing\\Cart\\OrderLine;
            use Acme\\Billing\\Cart\\PricedCart;
            use Raoh\\Issue;
            use Raoh\\Result;
            use Souther\\Runtime\\Expired;
            use Souther\\Runtime\\ForeignHandle;

            function issues(Result $result): string {
                return $result->fold(
                    fn ($value) => 'ok',
                    fn ($issues) => implode(' ', array_map(
                        fn (Issue $it) => '[' . ($it->path->toJsonPointer() ?: '/') . ' ' . $it->code . ']',
                        $issues->toArray())));
            }

            /** @param list<OrderLine> $lines */
            function said(array $lines): string {
                return implode(',', array_map(
                    fn (OrderLine $it): string => $it->sku() . ' x' . $it->quantity(), $lines));
            }

            $binding = Binding::load($argv[3]);

            $binding->run(function (): void {
                $apple = OrderLine::of('apple', 2, 150)->getOrThrow();
                $pear = OrderLine::of('pear', 3, 90)->getOrThrow();
                $order = Order::of([$apple, $pear])->getOrThrow();
                $lines = $order->lines();
                echo "lines: ", said($lines), ", a list ", var_export(array_is_list($lines), true), "\\n";
                echo "total: ", Behaviors::totalOf($order), "\\n";
                echo "echoed: ", said(Behaviors::echoed([$pear, $apple, $pear])), "\\n";
                echo "written: ", $order->encode(), "\\n";
                echo "read: ", said(Order::decode($order->encode())->getOrThrow()->lines()), "\\n";

                $empty = Order::of([])->getOrThrow();
                echo "empty: ", var_export($empty->lines(), true), ", total ",
                    Behaviors::totalOf($empty), ", priced ", issues(PricedCart::of([], 0)),
                    "\\n";

                $notes = Notes::of([null, 'gift wrap', null])->getOrThrow();
                echo "notes: ", var_export($notes->said(), true), " ", $notes->encode(), "\\n";
                $grid = Grid::of([[1, 2], [], [3]])->getOrThrow();
                echo "grid: ", json_encode($grid->rows()), "\\n";

                try {
                    Order::of([$apple, 'pear']);
                } catch (TypeError $refused) {
                    echo "not a line: ", $refused::class, "\\n";
                }
                try {
                    Order::of([1 => $apple]);
                } catch (InvalidArgumentException $refused) {
                    echo "not a list: ", $refused->getMessage(), "\\n";
                }
            });

            // The host prices the cart, and the library reads the lines of what it answered.
            $pricing = Injections::of(
                priceCart: fn (Order $order): PricedCart => PricedCart::of(
                    array_map(fn (OrderLine $it): OrderLine => OrderLine::of($it->sku(),
                        $it->quantity() * 2, $it->unitPrice())->getOrThrow(), $order->lines()),
                    7)->getOrThrow(),
                countNotes: fn (array $lines): int => count($lines) * 10 + strlen(said($lines)));
            echo "checkout: ", $binding->run(fn (): int => Behaviors::checkout(
                Order::of([OrderLine::of('apple', 1, 150)->getOrThrow()])->getOrThrow()),
                $pricing), "\\n";
            echo "noted: ", $binding->run(fn (): int => Behaviors::noted(
                Order::of([OrderLine::of('fig', 1, 1)->getOrThrow()])->getOrThrow()),
                $pricing), "\\n";

            // An empty cart priced by the host is not a priced cart, and the host is told so.
            echo "unpriced: ", $binding->run(fn (): string => issues(PricedCart::of([], 0))), "\\n";

            // A value of an outer run outlives a list of an inner one, and goes in it.
            echo "outer line: ", $binding->run(function () use ($binding): int {
                $apple = OrderLine::of('apple', 1, 150)->getOrThrow();
                return $binding->run(fn (): int =>
                    Behaviors::totalOf(Order::of([$apple, $apple])->getOrThrow()));
            }), "\\n";

            $kept = $binding->run(fn (): OrderLine => OrderLine::of('apple', 1, 150)->getOrThrow());
            try {
                $binding->run(fn () => Order::of([$kept]));
            } catch (Expired $expired) {
                echo "expired line: ", $expired->getMessage(), "\\n";
            }
            $again = Binding::load($argv[4]);
            try {
                $binding->run(function () use ($again) {
                    $line = OrderLine::of('apple', 1, 150)->getOrThrow();
                    return $again->run(fn () => Order::of([$line]));
                });
            } catch (ForeignHandle $foreign) {
                echo "foreign line: ", $foreign->getMessage(), "\\n";
            }
            """;

    private static final String ANSWERED = """
            lines: apple x2,pear x3, a list true
            total: 570
            echoed: pear x3,apple x2,pear x3
            written: {"lines":[{"sku":"apple","quantity":2,"unitPrice":150},{"sku":"pear","quantity":3,"unitPrice":90}]}
            read: apple x2,pear x3
            empty: array (
            ), total 0, priced [/ invariant_violation]
            notes: array (
              0 => NULL,
              1 => 'gift wrap',
              2 => NULL,
            ) {"said":[null,"gift wrap",null]}
            grid: [[1,2],[],[3]]
            not a line: TypeError
            not a list: an array handed to a Souther library as a list has keys other than 0, 1, 2 and on
            checkout: 300007
            noted: 16
            unpriced: [/ invariant_violation]
            outer line: 300
            expired line: a value was used after the run it was made in ended
            foreign line: a value one library made was handed to another
            """;

    /** Where the runtime package stands, with what Composer installed for it. */
    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void aPhpListCrossesBothWaysWhereverAListStands(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(CART)), into.resolve("native"));
        PhpBindings.Generated binding =
                PhpBindings.generate(library, into.resolve("php"), "Acme\\Billing");
        NativeCompiler.Library again =
                NativeCompiler.library(CheckedProgram.of(List.of(CART)), into.resolve("again"));
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString(), again.library().toString()));

        assertThat(said).isEqualTo(ANSWERED);
    }

    /**
     * PHP types a list as an array, and the docblock says what it is a list of, so PHPStan checks
     * the element where PHP cannot.
     */
    @Test
    void aListIsTypedAsAListOfItsElementForPhpStan(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(CART)), into.resolve("native"));
        PhpBindings.Generated binding =
                PhpBindings.generate(library, into.resolve("php"), "Acme\\Billing");
        Path cart = binding.root().resolve("Cart");

        assertThat(Files.readString(cart.resolve("Order.php")))
                .contains("@param list<\\Acme\\Billing\\Cart\\OrderLine> $lines")
                .contains("public static function of(array $lines)")
                .contains("@return list<\\Acme\\Billing\\Cart\\OrderLine>")
                .contains("public function lines(): array");
        assertThat(Files.readString(cart.resolve("Notes.php")))
                .contains("@return list<?string>");
        assertThat(Files.readString(cart.resolve("Grid.php")))
                .contains("@return list<list<int>>");
        assertThat(Files.readString(cart.resolve("Behaviors.php")))
                .contains("@param list<\\Acme\\Billing\\Cart\\OrderLine> $lines")
                .contains("@return list<\\Acme\\Billing\\Cart\\OrderLine>");
        assertThat(Files.readString(cart.resolve("Injections.php")))
                .contains("callable(list<\\Acme\\Billing\\Cart\\OrderLine>): int");
        for (Path file : binding.files()) {
            if (file.toString().endsWith(".php")) {
                assertThat(Php.compiles(file)).as("%s", file).isTrue();
            }
        }
    }
}
