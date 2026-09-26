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
 * A host makes a {@code Date}, a {@code Time}, a {@code DateTime} or an {@code Instant} of the text
 * that names it and reads the text back, each through a word of its own: the manifest says a
 * {@code date} where it says a {@code string} for text, so a host handed one where the other was
 * meant has been handed something else, and a C compiler that reads only the header refuses it.
 *
 * <p>What the text is, is what a boundary writes: a time to the second without its seconds where
 * they are nought, an instant in UTC. A host may make an instant of an offset spelling, which names
 * the same moment; the text it is read back as is the moment's.
 */
class AHostHandsOverATemporalAsTheTextThatNamesItTest {

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
            #include <string.h>
            #include "souther.h"

            static souther_string made(const char *text) {
                return souther_string_of_utf8((const uint8_t *) text, (int64_t) strlen(text));
            }

            static void text(souther_string said) {
                printf("%.*s", (int) souther_string_length(said),
                       (const char *) souther_string_bytes(said));
            }

            int main(void) {
                int64_t mark = souther_mark();

                souther_date day = souther_date_of_iso(made("2026-01-31"));
                souther_date moved = NULL;
                souther_status status = souther@_m_cal_b_shifted(NULL, day, 30, &moved);
                printf("shifted: status %u ", status);
                text(souther_date_iso(moved));
                printf(" from ");
                text(souther_date_iso(day));
                printf("\\n");

                souther_value named = NULL;
                status = souther@_m_cal_b_parsed(NULL, 2024, 2, 29, &named);
                souther_value none = NULL;
                souther_status refused = souther@_m_cal_b_parsed(NULL, 2026, 2, 29, &none);
                printf("parsed: status %u case %u ", status,
                       souther@_m_cal_b_parsed_answer_case(named));
                text(souther_date_iso(souther_case_date_read(named)));
                printf(", status %u case %u\\n", refused, souther@_m_cal_b_parsed_answer_case(none));

                souther_value made_case = souther_case_date_make(day);
                printf("made: case %u ", souther@_m_cal_b_parsed_answer_case(made_case));
                text(souther_date_iso(souther_case_date_read(made_case)));
                printf("\\n");

                souther_time clock = NULL;
                status = souther@_m_cal_b_clockOf(
                        NULL, souther_datetime_of_iso(made("2026-01-31T09:30:00")), &clock);
                printf("clock: status %u ", status);
                text(souther_time_iso(clock));
                printf(", ");
                text(souther_time_iso(souther_time_of_iso(made("23:59:59"))));
                printf(", ");
                text(souther_datetime_iso(souther_datetime_of_iso(made("2026-01-31T09:30:05"))));
                printf("\\n");

                uint8_t later = 9;
                status = souther@_m_cal_b_later(NULL,
                        souther_instant_of_iso(made("2026-01-31T09:30:00+09:00")),
                        souther_instant_of_iso(made("2026-01-31T00:30:00.000000001Z")), &later);
                uint8_t same = 9;
                souther@_m_cal_b_later(NULL,
                        souther_instant_of_iso(made("2026-01-31T09:30:00+09:00")),
                        souther_instant_of_iso(made("2026-01-31T00:30:00Z")), &same);
                souther_instant instant = NULL;
                souther@_m_cal_b_seen(NULL,
                        souther_instant_of_iso(made("2026-01-31T09:30:00.120+09:00")), &instant);
                printf("later: status %u %u %u, ", status, later, same);
                text(souther_instant_iso(instant));
                printf("\\n");

                souther_reset(mark);
                return 0;
            }
            """;

    @Test
    void aCProgramIncludingOnlyTheHeaderMakesATemporalOfTheTextThatNamesItAndReadsItBack(
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
                shifted: status 0 2026-03-02 from 2026-01-31
                parsed: status 0 case 0 2024-02-29, status 0 case 1
                made: case 0 2026-01-31
                clock: status 0 09:30, 23:59:59, 2026-01-31T09:30:05
                later: status 0 1 0, 2026-01-31T00:30:00.120Z
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
