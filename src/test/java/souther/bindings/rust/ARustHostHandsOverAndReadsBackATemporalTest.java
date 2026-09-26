package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A {@code Date}, a {@code Time}, a {@code DateTime} and an {@code Instant} are the runtime crate's
 * types to a Rust host, held as their numbers: checked where they are made, so what the library
 * would end the process on is refused before it is handed over, and handed over and read back as
 * the text {@code java.time} writes for them. A member of a union is one as well.
 */
class ARustHostHandsOverAndReadsBackATemporalTest {

    private static final String CALENDAR = """
            module cal exposing ( Booking, Missing, shifted, parsed, clockOf, later, seen )

            data Missing

            data Booking = { day: Date, start: Time, at: DateTime, seen: Instant }

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
            """;

    private static final String HOST = """
            use calendar::cal::{self, Booking, DateOrMissing};
            use calendar::{Date, DateTime, Instant, Library, Time};

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        let leap = Date::new(2024, 2, 28).unwrap();
                        println!("shifted: {}", cal::shifted(run, leap, 1).unwrap());
                        println!("shifted back: {}", cal::shifted(run, Date::new(1, 1, 1).unwrap(), -1).unwrap());
                        for (y, m, d) in [(2023, 2, 29), (2024, 2, 29)] {
                            match cal::parsed(run, y, m, d).unwrap() {
                                DateOrMissing::Date(day) => println!("parsed: {day:?}"),
                                DateOrMissing::Missing(_) => println!("parsed: missing"),
                            }
                        }
                        let at = DateTime::new(leap, Time::new(9, 30, 15).unwrap());
                        println!("clock: {}", cal::clockOf(run, at).unwrap());
                        let early = Instant::new(-1, 500_000_000).unwrap();
                        let late = Instant::new(1_700_000_000, 123_000).unwrap();
                        println!("later: {} {}", cal::later(run, early, late).unwrap(),
                            cal::later(run, late, early).unwrap());
                        println!("seen: {}", cal::seen(run, late).unwrap());
                        let booking = Booking::new(run, leap, Time::new(7, 5, 0).unwrap(), at, early)
                            .unwrap().into_result().unwrap();
                        println!("booking: {} {} {} {}", booking.day(), booking.start(), booking.at(),
                            booking.seen().nano());
                        println!("encoded: {}", booking.encode());
                    })
                    .unwrap();
                println!("refused: {}", Date::new(2023, 2, 29).unwrap_err());
            }
            """;

    @Test
    void aTemporalIsTheRuntimesTypeHeldAsItsNumbers(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(CALENDAR)), into.resolve("native"));
        RustBindings.Generated binding = RustHost.generated(library, into.resolve("binding"),
                "calendar");

        String said = RustHost.ran(into, binding, "calendar", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                shifted: 2024-02-29
                shifted back: 0000-12-31
                parsed: missing
                parsed: Date { year: 2024, month: 2, day: 29 }
                clock: 09:30:15
                later: true false
                seen: 2023-11-14T22:13:20.000123Z
                booking: 2024-02-28 07:05 2024-02-28T09:30:15 500000000
                encoded: {"day":"2024-02-28","start":"07:05","at":"2024-02-28T09:30:15","seen":"1969-12-31T23:59:59.500Z"}
                refused: 2023-2-29 is no day a Date holds
                """);
    }
}
