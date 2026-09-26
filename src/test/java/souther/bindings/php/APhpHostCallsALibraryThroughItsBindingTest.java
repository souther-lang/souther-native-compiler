package souther.bindings.php;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;

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
                                   settle, owing, stillOwing : Int, charge, chargedFree, discounted,
                                   squared, standardPrice, quantityOf, flaggedOf, labelOf,
                                   doubledQuantity )

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

            behavior charge : (paid: Int) -> Owed | Settled
            let charge (paid) =
                if paid > 1 then Waived { reason = "goodwill" }
                else if paid > 0 then Free
                else Owed { amount = Money(1), overdue = true }

            behavior chooseCharge : (paid: Int) -> Owed | Settled

            behavior chargedFree : (paid: Int) -> Bool
                depends on chooseCharge
            let chargedFree (paid, chooseCharge) = match chooseCharge(paid) with
                | Free -> true
                | Paid -> false
                | Waived -> false
                | Owed -> false

            behavior discountFor : (line: Line) -> Int

            behavior discounted : (line: Line) -> Int
                depends on discountFor
            let discounted (line, discountFor) = line.price.value * line.quantity - discountFor(line)

            behavior squared : (n: Int) -> Int
            let squared (n) = n * n

            let standardPrice = Money(3)

            behavior quantityOf : (paid: Int) -> Int | Free
            let quantityOf (paid) = if paid > 0 then paid else Free

            behavior flaggedOf : (paid: Int) -> Bool | Free
            let flaggedOf (paid) = if paid > 0 then paid > 1 else Free

            behavior labelOf : (paid: Int) -> String | Free
            let labelOf (paid) = if paid > 0 then "paid" else Free

            behavior chooseQuantity : (paid: Int) -> Int | Free

            behavior doubledQuantity : (paid: Int) -> Int
                depends on chooseQuantity
            let doubledQuantity (paid, chooseQuantity) = match chooseQuantity(paid) with
                | Int as n -> n * 2
                | Free -> -1
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
            use Souther\\Runtime\\OutsideAnyRun;
            use Souther\\Runtime\\RunOnAnotherFiber;
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

            $binding->run(function (): void {
                $money = Money::of(3)->getOrThrow();
                echo "money: ", $money->value(), ", standard ", Values::standardPrice()->value(), "\\n";
                echo "below: ", issues(Money::of(-1)), "\\n";
                echo "none: ", issues(Line::of($money, 0)), "\\n";

                $line = Line::of($money, 2, 'gift wrap')->getOrThrow();
                echo "line: ", $line->quantity(), " at ", $line->price()->value(), ", ", $line->note(), "\\n";
                echo "bare: ", var_export(Line::of($money, 2)->getOrThrow()->note(), true), "\\n";
                echo "written: ", $line->encode(), "\\n";

                $owed = Behaviors::settle($line, 2);
                echo "owed: ", $owed::class, ", amount ", $owed->amount()->value(),
                    ", overdue ", var_export($owed->overdue(), true),
                    ", an outcome ", var_export($owed instanceof Outcome, true),
                    ", settled ", var_export($owed instanceof Settled, true), "\\n";
                $paid = Behaviors::settle($line, 6);
                echo "paid: ", $paid::class, ", settled ", var_export($paid instanceof Settled, true),
                    ", an outcome ", var_export($paid instanceof Outcome, true), "\\n";
                echo "owing: ", Behaviors::owing($owed), " and ", Behaviors::owing($paid), "\\n";
                echo "still owing: ", Behaviors::stillOwing(input1: 2, input0: $line), "\\n";

                $owes = Behaviors::charge(0);
                $free = Behaviors::charge(1);
                $waived = Behaviors::charge(2);
                echo "charged: ", $owes::class, " ", $owes->amount()->value(), ", ", $free::class,
                    ", ", $waived::class, " settled ", var_export($waived instanceof Settled, true), "\\n";

                $said = OutcomeCodec::encode($owed);
                echo "outcome: ", $said, "\\n";
                $read = OutcomeCodec::decode($said)->getOrThrow();
                echo "read back: ", $read::class, ", amount ", $read->amount()->value(), "\\n";
                echo "free: ", Free::of()->getOrThrow()->encode(), "\\n";

                echo "order: ", issues(Order::decode('{"line": {"price": 4, "quantity": 5}, "placed": true}')), "\\n";
                echo "nested: ", issues(Order::decode('{"line": {"price": -1, "quantity": 5}, "placed": true}')), "\\n";
                echo "not json: ", issues(Line::decode('{"price"')), "\\n";

                $composed = Line::of($money, 1, "cafe\\u{0301}")->getOrThrow()->note();
                echo "normalized: ", bin2hex($composed), "\\n";

                $none = Behaviors::quantityOf(0);
                echo "quantity: ", var_export(Behaviors::quantityOf(5), true), ", ", $none::class,
                    ", ", var_export(Behaviors::flaggedOf(2), true), " ",
                    var_export(Behaviors::flaggedOf(1), true), ", ",
                    var_export(Behaviors::labelOf(1), true), ", ", Behaviors::labelOf(0)::class, "\\n";

                try {
                    Behaviors::squared(4000000000);
                } catch (SoutherAbort $abort) {
                    echo "aborted: ", $abort->status, "\\n";
                }
                try {
                    Behaviors::discounted($line);
                } catch (UnboundInjection $unbound) {
                    echo "unbound: ", $unbound::class, "\\n";
                }
            });

            $discounts = Injections::of(discountFor: fn (Line $line): int => $line->quantity());
            echo "discounted: ", $binding->run(
                fn (): int => Behaviors::discounted(Line::of(Money::of(3)->getOrThrow(), 2)->getOrThrow()),
                $discounts), "\\n";

            $charging = Injections::of(chooseCharge: fn (int $paid): Owed|Settled =>
                $paid > 0 ? Free::of()->getOrThrow()
                    : Owed::of(Money::of(1)->getOrThrow(), true)->getOrThrow());
            echo "charged free: ", var_export($binding->run(
                fn (): bool => Behaviors::chargedFree(1), $charging), true),
                " and ", var_export($binding->run(
                fn (): bool => Behaviors::chargedFree(0), $charging), true), "\\n";

            $quantities = Injections::of(chooseQuantity: fn (int $paid): int|Free =>
                $paid > 0 ? $paid + 1 : Free::of()->getOrThrow());
            echo "doubled quantity: ", $binding->run(
                fn (): int => Behaviors::doubledQuantity(3), $quantities), " and ", $binding->run(
                fn (): int => Behaviors::doubledQuantity(0), $quantities), "\\n";

            $down = new LogicException('the price list is down');
            try {
                $binding->run(
                    fn (): int => Behaviors::discounted(Line::of(Money::of(3)->getOrThrow(), 2)->getOrThrow()),
                    Injections::of(discountFor: function (Line $line) use ($down): int {
                        throw $down;
                    }));
            } catch (LogicException $caught) {
                echo "thrown: ", $caught === $down ? 'the same one' : 'another', "\\n";
            }

            echo "nested runs: ", $binding->run(function () use ($binding): int {
                $money = Money::of(5)->getOrThrow();
                return $binding->run(fn (): int =>
                    Behaviors::owing(Behaviors::settle(Line::of($money, 1)->getOrThrow(), 1)));
            }), "\\n";

            // A fiber suspended inside a run holds the library's runs until it ends them: another
            // fiber's run on top would be ended in some other order.
            $first = new Fiber(fn () => $binding->run(function (): int {
                $money = Money::of(6)->getOrThrow();
                $handed = Fiber::suspend($money);
                return $money->value() + $handed;
            }));
            $suspended = $first->start();
            $second = new Fiber(fn () => $binding->run(fn (): int => 1));
            try {
                $second->start();
            } catch (RunOnAnotherFiber $refused) {
                echo "second fiber: ", $refused::class, "\\n";
            }
            try {
                (new Fiber(fn () => $suspended->value()))->start();
            } catch (RunOnAnotherFiber $refused) {
                echo "value on another fiber: ", $refused::class, "\\n";
            }
            // A run going on another fiber is not one a call here is in.
            try {
                (new Fiber(fn () => Money::of(1)))->start();
            } catch (OutsideAnyRun $outside) {
                echo "call on another fiber: ", $outside::class, "\\n";
            }
            $first->resume(1);
            echo "first fiber: ", $first->getReturn(), ", then ",
                $binding->run(fn (): int => Money::of(2)->getOrThrow()->value()),
                "\\n";

            $kept = $binding->run(fn (): Money => Money::of(5)->getOrThrow());
            try {
                $kept->value();
            } catch (Expired $expired) {
                echo "expired: ", $expired->getMessage(), "\\n";
            }
            $again = Binding::load($argv[4]);
            try {
                $binding->run(function () use ($again) {
                    $money = Money::of(3)->getOrThrow();
                    return $again->run(fn () => Line::of($money, 1));
                });
            } catch (ForeignHandle $foreign) {
                echo "foreign: ", $foreign->getMessage(), "\\n";
            }
            echo "one binding: ", var_export(Binding::load($argv[3]) === $binding, true), "\\n";
            $linked = dirname($argv[4]) . '/linked-' . basename($argv[3]);
            link($argv[3], $linked);
            echo "one file: ", var_export(Binding::load($linked) === $binding, true), "\\n";

            // A field read out of an outer value is a value the outer one holds, which lives as long,
            // whichever run it was read in.
            $binding->run(function () use ($binding): void {
                $line = Line::of(Money::of(3)->getOrThrow(), 1)->getOrThrow();
                $price = $binding->run(fn (): Money => $line->price());
                echo "read in inner: still ", $price->value(), "\\n";
            });

            try {
                Money::of(1);
            } catch (OutsideAnyRun $outside) {
                echo "outside any run: ", $outside::class, "\\n";
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
            charged: Acme\\Billing\\Shop\\Owed 1, Acme\\Billing\\Shop\\Free, Acme\\Billing\\Shop\\SettledValue settled true
            outcome: {"type":"Owed","amount":4,"overdue":false}
            read back: Acme\\Billing\\Shop\\Owed, amount 4
            free: {}
            order: ok
            nested: [/line/price invariant_violation]
            not json: [/ invalid_format]
            normalized: 636166c3a9
            quantity: 5, Acme\\Billing\\Shop\\Free, true false, 'paid', Acme\\Billing\\Shop\\Free
            aborted: REQUIRED_FORM_HAS_NO_PLACE
            unbound: Souther\\Runtime\\UnboundInjection
            discounted: 4
            charged free: true and false
            doubled quantity: 8 and -1
            thrown: the same one
            nested runs: 4
            second fiber: Souther\\Runtime\\RunOnAnotherFiber
            value on another fiber: Souther\\Runtime\\RunOnAnotherFiber
            call on another fiber: Souther\\Runtime\\OutsideAnyRun
            first fiber: 7, then 2
            expired: a value was used after the run it was made in ended
            foreign: a value one library made was handed to another
            one binding: true
            one file: true
            read in inner: still 3
            outside any run: Souther\\Runtime\\OutsideAnyRun
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
                NativeCompiler.library(Checked.of(List.of(SHOP)), into.resolve("native"));
        PhpBindings.Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Billing");
        // The same model built a second time, which is a second library to PHP.
        NativeCompiler.Library again =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into.resolve("again"));
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
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
                NativeCompiler.library(Checked.of(List.of(SHOP)), into.resolve("native"));
        PhpBindings.Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Billing");
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

                $binding = Binding::preloaded('souther_shop', $argv[1]);
                echo $binding->run(fn (): int => Behaviors::owing(Behaviors::settle(
                    Line::of(Money::of(3)->getOrThrow(), 2)->getOrThrow(), 2))), "\\n";
                """, StandardCharsets.UTF_8);

        assertThat(Php.ran(List.of("-d", "opcache.enable_cli=1",
                "-d", "opcache.preload=" + preload, "-d", "ffi.enable=preload",
                host.toString(), library.library().toString()))).isEqualTo("4\n");
    }
}
