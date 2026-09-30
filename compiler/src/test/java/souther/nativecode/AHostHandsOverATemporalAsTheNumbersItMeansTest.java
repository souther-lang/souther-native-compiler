package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A host makes a {@code Date}, a {@code Time}, a {@code DateTime} or an {@code Instant} of the
 * numbers it means and reads those numbers back, each through a word of its own: the manifest says
 * a {@code date} where it says a {@code string} for text, so a host handed one where the other was
 * meant has been handed something else, and a C compiler that reads only the header refuses it.
 *
 * <p>The numbers are what the type is defined by (spec §primitives): a date's year, month and day, a
 * time's hour, minute and second, a date-time both, and an instant's second from the epoch and the
 * nanosecond within it. Numbers that name no value are answered as that, and the process goes on
 * (souther-native-compiler#137).
 */
class AHostHandsOverATemporalAsTheNumbersItMeansTest {

    private static final String CALENDAR = """
            module cal exposing ( shifted, parsed, clockOf, later, seen, Missing )

            data Missing

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

    private static final String HARNESS = """
            #include <inttypes.h>
            #include <stdio.h>
            #include "souther.h"

            static void date(souther_date at) {
                int64_t year, month, day;
                souther_date_parts(at, &year, &month, &day);
                printf("%" PRId64 "-%" PRId64 "-%" PRId64, year, month, day);
            }

            static void clock(souther_time at) {
                int64_t hour, minute, second;
                souther_time_parts(at, &hour, &minute, &second);
                printf("%" PRId64 ":%" PRId64 ":%" PRId64, hour, minute, second);
            }

            static souther_instant moment(int64_t second, int64_t nano) {
                souther_instant made = NULL;
                souther_instant_of_parts(second, nano, &made);
                return made;
            }

            int main(void) {
                int64_t scope = souther_scope_open();

                souther_date day = NULL;
                uint8_t made = souther_date_of_parts(2026, 1, 31, &day);
                souther_date moved = NULL;
                souther_status status = souther@_m_cal_b_shifted(NULL, day, 30, &moved);
                printf("shifted: made %u status %u ", made, status);
                date(moved);
                printf(" from ");
                date(day);
                printf("\\n");

                souther_value named = NULL;
                status = souther@_m_cal_b_parsed(NULL, 2024, 2, 29, &named);
                souther_value none = NULL;
                souther_status refused = souther@_m_cal_b_parsed(NULL, 2026, 2, 29, &none);
                printf("parsed: status %u case %u ", status,
                       souther@_m_cal_b_parsed_answer_case(named));
                souther_date named_day = NULL;
                souther_case_date_read(named, &named_day);
                date(named_day);
                printf(", status %u case %u\\n", refused, souther@_m_cal_b_parsed_answer_case(none));

                souther_value made_case = souther_case_date_make(day);
                printf("made: case %u ", souther@_m_cal_b_parsed_answer_case(made_case));
                souther_date made_day = NULL;
                souther_case_date_read(made_case, &made_day);
                date(made_day);
                printf("\\n");

                souther_datetime at = NULL;
                souther_datetime_of_parts(2026, 1, 31, 9, 30, 5, &at);
                souther_time of_day = NULL;
                status = souther@_m_cal_b_clockOf(NULL, at, &of_day);
                printf("clock: status %u ", status);
                clock(of_day);
                int64_t parts[6];
                souther_datetime_parts(at, &parts[0], &parts[1], &parts[2], &parts[3], &parts[4],
                                       &parts[5]);
                printf(", %" PRId64 "-%" PRId64 "-%" PRId64 " %" PRId64 ":%" PRId64 ":%" PRId64 "\\n",
                       parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]);

                uint8_t later = 9;
                status = souther@_m_cal_b_later(NULL, moment(1769821800, 0),
                                                moment(1769821800, 1), &later);
                uint8_t same = 9;
                souther@_m_cal_b_later(NULL, moment(1769821800, 0), moment(1769821800, 0), &same);
                souther_instant instant = NULL;
                souther@_m_cal_b_seen(NULL, moment(-1, 120000000), &instant);
                int64_t second, nano;
                souther_instant_parts(instant, &second, &nano);
                printf("later: status %u %u %u, %" PRId64 " %" PRId64 "\\n", status, later, same,
                       second, nano);

                souther_date no_day = NULL;
                souther_time no_time = NULL;
                souther_datetime no_at = NULL;
                souther_instant no_moment = NULL;
                printf("refused: %u %u %u %u %u %u\\n",
                       souther_date_of_parts(2026, 2, 30, &no_day),
                       souther_date_of_parts(2026, -1, 1, &no_day),
                       souther_time_of_parts(24, 0, 0, &no_time),
                       souther_datetime_of_parts(2026, 1, 31, 9, 60, 0, &no_at),
                       souther_instant_of_parts(0, 1000000000, &no_moment),
                       souther_instant_of_parts(0, -1, &no_moment));

                souther_scope_close(scope);
                return 0;
            }
            """;

    @Test
    void aCProgramIncludingOnlyTheHeaderMakesATemporalOfItsNumbersAndReadsThemBack(
            @TempDir Path into) throws Exception {
        assertThat(HARNESS).doesNotContain("__asm__").doesNotContain("extern");
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(CALENDAR)), into);
        Path source = into.resolve("host.c");
        Files.writeString(source, Running.spelt(HARNESS), StandardCharsets.UTF_8);
        Path executable = into.resolve("host");

        said(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), source.toString(),
                "-I", library.header().getParent().toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));

        assertThat(said(List.of(executable.toString()))).isEqualTo("""
                shifted: made 1 status 0 2026-3-2 from 2026-1-31
                parsed: status 0 case 0 2024-2-29, status 0 case 1
                made: case 0 2026-1-31
                clock: status 0 9:30:5, 2026-1-31 9:30:5
                later: status 0 1 0, -1 120000000
                refused: 0 0 0 0 0 0
                """);
    }

    private static String said(List<String> command) throws IOException, InterruptedException {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError(command.get(0) + " failed: " + said);
        }
        return said;
    }
}
