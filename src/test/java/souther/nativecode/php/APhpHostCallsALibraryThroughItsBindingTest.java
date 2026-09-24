package souther.nativecode.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A PHP application calls a library through the binding generated for it, and never through FFI
 * itself: it builds values, calls behaviors, reads the answer's fields and case, reads external
 * JSON and writes a value back, implements a behavior the model asks a host for, and is refused
 * where it holds a value past its run.
 */
class APhpHostCallsALibraryThroughItsBindingTest {

    private static final String SHOP = """
            module shop exposing ( Money, Line, Order, Free, Paid, Owed, Settled, Outcome,
                                   settle, owing, stillOwing : Int, discounted, squared,
                                   standardPrice )

            data Money = Int
                invariant notNegative = value >= 0

            data Line = { price: Money, quantity: Int, note: String? }
                invariant some = quantity > 0

            data Order = { line: Line, placed: Bool }

            data Free
            data Paid = { amount: Money }
            data Waived = { reason: String }
            data Owed = { amount: Money, overdue: Bool }
            data Settled = Free | Paid | Waived
            data Outcome = Settled | Owed

            behavior settle : (line: Line, paid: Int) -> Outcome
            let settle (line, paid) = {
                let due = line.price.value * line.quantity
                if due == 0 then Free
                else if paid >= due then Paid { amount = Money(due) }
                else Owed { amount = Money(due - paid), overdue = paid == 0 }
            }

            behavior owing : (outcome: Outcome) -> Int
            let owing (outcome) = match outcome with
                | Owed as o -> o.amount.value
                | Settled -> 0

            behavior stillOwing = settle >-> owing

            behavior discountFor : (line: Line) -> Int

            behavior discounted : (line: Line) -> Int
                depends on discountFor
            let discounted (line, discountFor) = line.price.value * line.quantity - discountFor(line)

            behavior squared : (n: Int) -> Int
            let squared (n) = n * n

            let standardPrice = Money(3)
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Billing\\Binding;
            use Acme\\Billing\\Shop\\Behaviors;
            use Acme\\Billing\\Shop\\Free;
            use Acme\\Billing\\Shop\\Injections;
            use Acme\\Billing\\Shop\\Line;
            use Acme\\Billing\\Shop\\Money;
            use Acme\\Billing\\Shop\\Order;
            use Acme\\Billing\\Shop\\Outcome;
            use Acme\\Billing\\Shop\\OutcomeCodec;
            use Acme\\Billing\\Shop\\Owed;
            use Acme\\Billing\\Shop\\Paid;
            use Acme\\Billing\\Shop\\Settled;
            use Acme\\Billing\\Shop\\Values;
            use Raoh\\Issue;
            use Raoh\\Result;
            use Souther\\Runtime\\Expired;
            use Souther\\Runtime\\ForeignHandle;
            use Souther\\Runtime\\NotTheInnermostRun;
            use Souther\\Runtime\\Session;
            use Souther\\Runtime\\SoutherAbort;
            use Souther\\Runtime\\UnboundInjection;

            function issues(Result $result): string {
                return $result->fold(
                    fn ($value) => 'ok',
                    fn ($issues) => implode(' ', array_map(
                        fn (Issue $it) => '[' . ($it->path->toJsonPointer() ?: '/') . ' ' . $it->code . ']',
                        $issues->toArray())));
            }

            $binding = Binding::load($argv[3]);

            $binding->run(function (Session $session): void {
                $money = Money::of($session, 3)->getOrThrow();
                echo "money: ", $money->value(), ", standard ", Values::standardPrice($session)->value(), "\\n";
                echo "below: ", issues(Money::of($session, -1)), "\\n";
                echo "none: ", issues(Line::of($session, $money, 0)), "\\n";

                $line = Line::of($session, $money, 2, 'gift wrap')->getOrThrow();
                echo "line: ", $line->quantity(), " at ", $line->price()->value(), ", ", $line->note(), "\\n";
                echo "bare: ", var_export(Line::of($session, $money, 2)->getOrThrow()->note(), true), "\\n";
                echo "written: ", $line->encode(), "\\n";

                $owed = Behaviors::settle($session, $line, 2);
                echo "owed: ", $owed::class, ", amount ", $owed->amount()->value(),
                    ", overdue ", var_export($owed->overdue(), true),
                    ", an outcome ", var_export($owed instanceof Outcome, true),
                    ", settled ", var_export($owed instanceof Settled, true), "\\n";
                $paid = Behaviors::settle($session, $line, 6);
                echo "paid: ", $paid::class, ", settled ", var_export($paid instanceof Settled, true),
                    ", an outcome ", var_export($paid instanceof Outcome, true), "\\n";
                echo "owing: ", Behaviors::owing($session, $owed), " and ", Behaviors::owing($session, $paid), "\\n";
                echo "still owing: ", Behaviors::stillOwing($session, input1: 2, input0: $line), "\\n";

                $said = OutcomeCodec::encode($owed);
                echo "outcome: ", $said, "\\n";
                $read = OutcomeCodec::decode($session, $said)->getOrThrow();
                echo "read back: ", $read::class, ", amount ", $read->amount()->value(), "\\n";
                echo "free: ", Free::of($session)->getOrThrow()->encode(), "\\n";

                echo "order: ", issues(Order::decode($session,
                    '{"line": {"price": 4, "quantity": 5}, "placed": true}')), "\\n";
                echo "nested: ", issues(Order::decode($session,
                    '{"line": {"price": -1, "quantity": 5}, "placed": true}')), "\\n";
                echo "not json: ", issues(Line::decode($session, '{"price"')), "\\n";

                $composed = Line::of($session, $money, 1, "cafe\\u{0301}")->getOrThrow()->note();
                echo "normalized: ", bin2hex($composed), "\\n";

                try {
                    Behaviors::squared($session, 4000000000);
                } catch (SoutherAbort $abort) {
                    echo "aborted: ", $abort->status, "\\n";
                }
                try {
                    Behaviors::discounted($session, $line);
                } catch (UnboundInjection $unbound) {
                    echo "unbound: ", $unbound::class, "\\n";
                }
            });

            $discounts = Injections::of(discountFor: fn (Session $session, Line $line): int => $line->quantity());
            echo "discounted: ", $binding->run(
                fn (Session $session): int => Behaviors::discounted($session,
                    Line::of($session, Money::of($session, 3)->getOrThrow(), 2)->getOrThrow()),
                $discounts), "\\n";

            $down = new LogicException('the price list is down');
            try {
                $binding->run(
                    fn (Session $session): int => Behaviors::discounted($session,
                        Line::of($session, Money::of($session, 3)->getOrThrow(), 2)->getOrThrow()),
                    Injections::of(discountFor: function (Session $session, Line $line) use ($down): int {
                        throw $down;
                    }));
            } catch (LogicException $caught) {
                echo "thrown: ", $caught === $down ? 'the same one' : 'another', "\\n";
            }

            echo "nested runs: ", $binding->run(function (Session $outer) use ($binding): int {
                $money = Money::of($outer, 5)->getOrThrow();
                return $binding->run(fn (Session $inner): int =>
                    Behaviors::owing($inner, Behaviors::settle($inner,
                        Line::of($inner, $money, 1)->getOrThrow(), 1)));
            }), "\\n";

            $kept = $binding->run(fn (Session $session): Money => Money::of($session, 5)->getOrThrow());
            try {
                $kept->value();
            } catch (Expired $expired) {
                echo "expired: ", $expired->getMessage(), "\\n";
            }
            $again = Binding::load($argv[4]);
            try {
                $binding->run(fn (Session $here) => $again->run(fn (Session $there) =>
                    Line::of($there, Money::of($here, 3)->getOrThrow(), 1)));
            } catch (ForeignHandle $foreign) {
                echo "foreign: ", $foreign->getMessage(), "\\n";
            }
            echo "one binding: ", var_export(Binding::load($argv[3]) === $binding, true), "\\n";
            $linked = dirname($argv[4]) . '/linked-' . basename($argv[3]);
            link($argv[3], $linked);
            echo "one file: ", var_export(Binding::load($linked) === $binding, true), "\\n";

            // A computation started through an outer session while an inner run is going would
            // make a value in the inner run's part of the arena, which that run drops.
            $binding->run(function (Session $outer) use ($binding): void {
                try {
                    $binding->run(fn (Session $inner) => Money::of($outer, 5));
                } catch (NotTheInnermostRun $refused) {
                    echo "outer in inner: ", $refused::class, "\\n";
                }
                // A field read out of an outer value is a value the outer one holds, which lives as
                // long, whichever run it was read in.
                $line = Line::of($outer, Money::of($outer, 3)->getOrThrow(), 1)->getOrThrow();
                $price = $binding->run(fn (Session $inner): Money => $line->price());
                echo "read in inner: still ", $price->value(), "\\n";
            });

            $keptSession = $binding->run(fn (Session $session): Session => $session);
            try {
                Money::of($keptSession, 1);
            } catch (Expired $expired) {
                echo "session expired: ", $expired->getMessage(), "\\n";
            }
            """;

    private static final String ANSWERED = """
            money: 3, standard 3
            below: [/ invariant_violation]
            none: [/ invariant_violation]
            line: 2 at 3, gift wrap
            bare: NULL
            written: {"price":3,"quantity":2,"note":"gift wrap"}
            owed: Acme\\Billing\\Shop\\Owed, amount 4, overdue false, an outcome true, settled false
            paid: Acme\\Billing\\Shop\\Paid, settled true, an outcome true
            owing: 4 and 0
            still owing: 4
            outcome: {"type":"Owed","amount":4,"overdue":false}
            read back: Acme\\Billing\\Shop\\Owed, amount 4
            free: {}
            order: ok
            nested: [/line/price invariant_violation]
            not json: [/ invalid_format]
            normalized: 636166c3a9
            aborted: REQUIRED_FORM_HAS_NO_PLACE
            unbound: Souther\\Runtime\\UnboundInjection
            discounted: 4
            thrown: the same one
            nested runs: 4
            expired: a value was used after the run it was made in ended
            foreign: a value one library made was handed to another
            one binding: true
            one file: true
            outer in inner: Souther\\Runtime\\NotTheInnermostRun
            read in inner: still 3
            session expired: a session was used after its run ended
            """;

    /** Where the runtime package stands, with what Composer installed for it. */
    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void aPhpApplicationUsesTheModelThroughItsGeneratedBinding(@TempDir Path into) throws Exception {
        assertThat(RUNTIME.resolve("vendor").resolve("autoload.php"))
                .as("the dependencies of %s, which the build installs with Composer before the"
                        + " tests", RUNTIME)
                .exists();
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(SHOP)), into.resolve("native"));
        PhpBindings.Generated binding =
                PhpBindings.generate(library, into.resolve("php"), "Acme\\Billing");
        // The same model built a second time, which is a second library to PHP.
        NativeCompiler.Library again =
                NativeCompiler.library(CheckedProgram.of(List.of(SHOP)), into.resolve("again"));
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = said(List.of("php", "-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString(), again.library().toString()));

        assertThat(said).isEqualTo(ANSWERED);
    }

    /**
     * Under {@code ffi.enable=preload} a request cannot declare a library, so a preload script
     * declares it from what {@code Binding::preloadHeader} writes, and a request asks for it by the
     * scope and the library's path: the same library a load of that file would be.
     */
    @Test
    void aPreloadedLibraryIsCalledAsALoadedOneIs(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(SHOP)), into.resolve("native"));
        PhpBindings.Generated binding =
                PhpBindings.generate(library, into.resolve("php"), "Acme\\Billing");
        String requires = "require '" + RUNTIME.toAbsolutePath().resolve("vendor")
                .resolve("autoload.php") + "';\nrequire '" + binding.root().resolve("autoload.php")
                + "';\n";
        Path header = into.resolve("preloaded.h");
        Path preload = into.resolve("preload.php");
        Files.writeString(preload, "<?php\n" + requires
                + "file_put_contents('" + header + "', \\Acme\\Billing\\Binding::preloadHeader("
                + "'souther_shop', '" + library.library() + "'));\n"
                + "FFI::load('" + header + "');\n", StandardCharsets.UTF_8);
        Path host = into.resolve("host.php");
        Files.writeString(host, "<?php\n" + requires + """
                use Acme\\Billing\\Binding;
                use Acme\\Billing\\Shop\\Behaviors;
                use Acme\\Billing\\Shop\\Line;
                use Acme\\Billing\\Shop\\Money;
                use Souther\\Runtime\\Session;

                $binding = Binding::preloaded('souther_shop', $argv[1]);
                echo $binding->run(fn (Session $session): int => Behaviors::owing($session,
                    Behaviors::settle($session, Line::of($session,
                        Money::of($session, 3)->getOrThrow(), 2)->getOrThrow(), 2))), "\\n";
                """, StandardCharsets.UTF_8);

        assertThat(said(List.of("php", "-d", "opcache.enable_cli=1",
                "-d", "opcache.preload=" + preload, "-d", "ffi.enable=preload",
                host.toString(), library.library().toString()))).isEqualTo("4\n");
    }

    private static String said(List<String> command) throws Exception {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError(command.get(0) + " failed: " + said);
        }
        return said;
    }
}
