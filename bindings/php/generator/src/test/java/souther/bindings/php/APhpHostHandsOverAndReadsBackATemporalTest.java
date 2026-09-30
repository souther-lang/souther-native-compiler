package souther.bindings.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.Generated;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;
import souther.nativecode.Repository;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A PHP host hands the library a {@code Date}, a {@code Time}, a {@code DateTime} and an
 * {@code Instant} as the runtime's classes of those names, held as their numbers and handed over as
 * the text java.time writes for them, and is handed one back the same way, wherever it stands: what
 * a behavior takes and answers, a field, an optional, an element of a list, a case of a union, and
 * what a behavior the host implements is handed and answers.
 *
 * <p>The library ends the process on text that names no temporal, so what the runtime writes is
 * held here at the ends of each range, where the text is least like the common case: a year with a
 * sign, a year of more than four digits, the first and the last moment an {@code Instant} holds.
 */
class APhpHostHandsOverAndReadsBackATemporalTest {

    private static final String CALENDAR = """
            module cal exposing ( Booking, Due, Missing, shifted, parsed, clockOf, later, seen, days,
                                  remind )

            data Missing

            data Booking = { day: Date, start: Time, at: DateTime, seen: Instant }

            data Due = { on: Date? }

            behavior shifted : (day: Date, n: Int) -> Date
            let shifted (day, n) = Date.addDays(n, day)

            behavior parsed : (y: Int, m: Int, d: Int) -> Date | Missing
            let parsed (y, m, d) = match Date.fromParts(y, m, d) with
                | Date as day -> day
                | NotADate -> Missing

            behavior clockOf : (at: DateTime) -> Time
            let clockOf (at) = DateTime.toTime(at)

            behavior later : (a: Instant, b: Instant) -> Bool
            let later (a, b) = a < b

            behavior seen : (at: Instant) -> Instant
            let seen (at) = at

            behavior days : (xs: List<Date>) -> List<Date>
            let days (xs) = xs

            behavior deadlineFor : (day: Date) -> Date

            behavior remind : (day: Date) -> Date
                depends on deadlineFor
            let remind (day, deadlineFor) = Date.addDays(-1, deadlineFor(day))
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Calendar\\Binding;
            use Acme\\Calendar\\Cal\\Behaviors;
            use Acme\\Calendar\\Cal\\Booking;
            use Acme\\Calendar\\Cal\\Due;
            use Acme\\Calendar\\Cal\\Injections;
            use Acme\\Calendar\\Cal\\Missing;
            use Souther\\Runtime\\Date;
            use Souther\\Runtime\\DateTime;
            use Souther\\Runtime\\Instant;
            use Souther\\Runtime\\Time;

            $binding = Binding::load($argv[3]);

            $binding->run(function (): void {
                $leap = new Date(2024, 2, 28);
                echo "shifted: ", Behaviors::shifted($leap, 1), "\\n";
                echo "shifted back: ", Behaviors::shifted(new Date(1, 1, 1), -1), "\\n";
                foreach ([[2023, 2, 29], [2024, 2, 29]] as [$y, $m, $d]) {
                    $parsed = Behaviors::parsed($y, $m, $d);
                    echo "parsed: ", $parsed instanceof Missing ? 'missing' : $parsed, "\\n";
                }
                $at = new DateTime($leap, new Time(9, 30, 15));
                echo "clock: ", Behaviors::clockOf($at), "\\n";
                $early = new Instant(-1, 500_000_000);
                $late = new Instant(1_700_000_000, 123_000);
                echo "later: ", var_export(Behaviors::later($early, $late), true), " ",
                    var_export(Behaviors::later($late, $early), true), "\\n";
                echo "seen: ", Behaviors::seen($late), "\\n";
                $booking = Booking::of($leap, new Time(7, 5), $at, $early)->getOrThrow();
                echo "booking: ", $booking->day(), " ", $booking->start(), " ", $booking->at(), " ",
                    $booking->seen()->nano, "\\n";
                echo "encoded: ", $booking->encode(), "\\n";
                echo "decoded: ", Booking::decode($booking->encode())->getOrThrow()->at(), "\\n";
                echo "days: ", implode(' ', Behaviors::days(
                    [$leap, new Date(-5, 1, 1), new Date(10000, 12, 31)])), "\\n";
                $none = Due::of(null)->getOrThrow();
                $some = Due::of($leap)->getOrThrow();
                echo "due: ", var_export($none->on(), true), " ", $some->on(), "\\n";
                echo "years: ", Behaviors::shifted(new Date(-999_999_999, 1, 1), 0), " ",
                    Behaviors::shifted(new Date(999_999_999, 12, 31), 0), "\\n";
                echo "moments: ", Behaviors::seen(new Instant(-31_557_014_167_219_200)), " ",
                    Behaviors::seen(new Instant(31_556_889_864_403_199, 999_999_999)), "\\n";
                echo "fractions: ", Behaviors::seen(new Instant(0, 100_000_000)), " ",
                    Behaviors::seen(new Instant(0, 1_000)), " ", Behaviors::seen(new Instant(0, 1)), "\\n";
            });

            $deadlines = Injections::of(deadlineFor: fn (Date $day): Date => new Date($day->year, $day->month, 28));
            echo "remind: ", $binding->run(fn (): Date => Behaviors::remind(new Date(2024, 2, 3)), $deadlines), "\\n";

            echo "of: ", Date::of(new \\DateTimeImmutable('2024-02-29 23:30', new \\DateTimeZone('Asia/Tokyo'))), " ",
                Instant::of(new \\DateTimeImmutable('1970-01-01T00:00:01.5Z')), " ",
                (new Instant(1, 500_000_000))->toDateTime()->format('Y-m-d\\TH:i:s.u\\Z'), "\\n";
            foreach ([fn () => new Date(2023, 2, 29), fn () => new Time(24, 0),
                    fn () => new Instant(0, 1_000_000_000), fn () => new Date(1_000_000_000, 1, 1)] as $make) {
                try {
                    $make();
                } catch (InvalidArgumentException $refused) {
                    echo "refused: ", $refused->getMessage(), "\\n";
                }
            }
            """;

    private static final String ANSWERED = """
            shifted: 2024-02-29
            shifted back: 0000-12-31
            parsed: missing
            parsed: 2024-02-29
            clock: 09:30:15
            later: true false
            seen: 2023-11-14T22:13:20.000123Z
            booking: 2024-02-28 07:05 2024-02-28T09:30:15 500000000
            encoded: {"day":"2024-02-28","start":"07:05","at":"2024-02-28T09:30:15","seen":"1969-12-31T23:59:59.500Z"}
            decoded: 2024-02-28T09:30:15
            days: 2024-02-28 -0005-01-01 +10000-12-31
            due: NULL 2024-02-28
            years: -999999999-01-01 +999999999-12-31
            moments: -1000000000-01-01T00:00:00Z +1000000000-12-31T23:59:59.999999999Z
            fractions: 1970-01-01T00:00:00.100Z 1970-01-01T00:00:00.000001Z 1970-01-01T00:00:00.000000001Z
            remind: 2024-02-27
            of: 2024-02-29 1970-01-01T00:00:01.500Z 1970-01-01T00:00:01.500000Z
            refused: 2023-2-29 is no day a Date holds
            refused: 24:0:0 is no time of day
            refused: 0 seconds and 1000000000 nanoseconds is no moment an Instant holds
            refused: 1000000000-1-1 is no day a Date holds
            """;

    /** Where the runtime package stands, with what Composer installed for it. */
    private static final Path RUNTIME = Repository.file("bindings", "php", "runtime");

    @Test
    void aTemporalCrossesAsTheRuntimesClassWhereverItStands(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(CALENDAR)), into.resolve("native"));
        Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme\\Calendar");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        String said = Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()));

        assertThat(said).isEqualTo(ANSWERED);
        assertThat(Php.compiles(binding.files().stream()
                .filter(file -> file.toString().endsWith(".php")).toList()))
                .allSatisfy((file, compiles) -> assertThat(compiles).as("%s", file).isTrue());
    }
}
