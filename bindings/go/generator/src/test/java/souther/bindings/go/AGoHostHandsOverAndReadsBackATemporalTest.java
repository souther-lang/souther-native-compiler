package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Date, a Time, a DateTime and an Instant are the runtime's own types held as their numbers and
 * handed over as the text java.time writes for them, which the library reads.
 */
class AGoHostHandsOverAndReadsBackATemporalTest {

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
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

            	"example.com/calendar"
            	"example.com/calendar/cal"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func date(year int32, month, day uint8) souther.Date {
            	return made(souther.NewDate(year, month, day))
            }

            func clock(hour, minute, second uint8) souther.Time {
            	return made(souther.NewTime(hour, minute, second))
            }

            func main() {
            	library, err := calendar.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *calendar.Run) error {
            		leap := date(2024, 2, 28)
            		fmt.Printf("shifted: %s\\n", made(cal.Shifted(r, leap, 1)))
            		fmt.Printf("shifted back: %s\\n", made(cal.Shifted(r, date(1, 1, 1), -1)))
            		for _, ymd := range [][3]int64{{2023, 2, 29}, {2024, 2, 29}} {
            			switch it := made(cal.Parsed(r, ymd[0], ymd[1], ymd[2])).(type) {
            			case cal.DateOrMissingDate:
            				fmt.Printf("parsed: %s\\n", it.Value)
            			case cal.DateOrMissingMissing:
            				fmt.Printf("parsed: missing\\n")
            			}
            		}
            		at := souther.NewDateTime(leap, clock(9, 30, 15))
            		fmt.Printf("clock: %s\\n", made(cal.ClockOf(r, at)))
            		early := made(souther.NewInstant(-1, 500_000_000))
            		late := made(souther.NewInstant(1_700_000_000, 123_000))
            		fmt.Printf("later: %v %v\\n", made(cal.Later(r, early, late)), made(cal.Later(r, late, early)))
            		fmt.Printf("seen: %s\\n", made(cal.Seen(r, late)))
            		booking := made(cal.NewBooking(r, leap, clock(7, 5, 0), at, early))
            		fmt.Printf("booking: %s %s %s %d\\n", booking.Day(), booking.Start(), booking.At(), booking.Seen().Nano())
            		fmt.Printf("encoded: %s\\n", booking.Encode())
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            	_, err = souther.NewDate(2023, 2, 29)
            	fmt.Printf("refused: %v\\n", errors.Is(err, souther.ErrNotTemporal))
            }
            """;

    @Test
    void aTemporalIsTheRuntimesTypeHeldAsItsNumbers(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, CALENDAR, "example.com/calendar", HOST);

        assertThat(said).isEqualTo("""
                shifted: 2024-02-29
                shifted back: 0000-12-31
                parsed: missing
                parsed: 2024-02-29
                clock: 09:30:15
                later: true false
                seen: 2023-11-14T22:13:20.000123Z
                booking: 2024-02-28 07:05 2024-02-28T09:30:15 500000000
                encoded: {"day":"2024-02-28","start":"07:05","at":"2024-02-28T09:30:15","seen":"1969-12-31T23:59:59.500Z"}
                refused: true
                """);
    }
}
