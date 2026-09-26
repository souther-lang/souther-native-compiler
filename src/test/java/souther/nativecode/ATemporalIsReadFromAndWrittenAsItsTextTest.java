package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A {@code Date}, a {@code Time}, a {@code DateTime} and an {@code Instant} cross a boundary as the
 * ISO 8601 text that names them: read from it by the grammar of each type (Raoh's), and written
 * as {@code toString} of the {@code java.time} class the language names for it (spec §primitives).
 *
 * <p>The rows are where reading and writing part from each other, which is the whole of what a
 * boundary decides: a clock written to the second and read back without its seconds, an instant
 * read from an offset and written in UTC, a fraction that a type held to the second would have to
 * drop, and the leap second no moment names.
 */
class ATemporalIsReadFromAndWrittenAsItsTextTest {

    private static final String WIRE = """
            module wire exposing ( Booking )

            data Booking = { day: Date, start: Time, at: DateTime, seen: Instant }
            """;

    private static String booking(String day, String start, String at, String seen) {
        return "{\"day\":" + day + ",\"start\":" + start + ",\"at\":" + at + ",\"seen\":" + seen + "}";
    }

    private static String ok(String day, String start, String at, String seen) {
        return booking("\"" + day + "\"", "\"" + start + "\"", "\"" + at + "\"", "\"" + seen + "\"");
    }

    private static final Decoding ROWS = new Decoding()
            .type("wire", "Booking")
            .row("plain", "Booking", ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00Z"))
            .row("seconds written out", "Booking",
                    ok("2026-07-25", "09:30:00", "2026-07-25T09:30:00", "2026-07-25T00:00:00Z"))
            .row("seconds kept", "Booking",
                    ok("2026-07-25", "09:30:05", "2026-07-25T09:30:05", "2026-07-25T00:00:05Z"))
            .row("fraction of nought", "Booking",
                    ok("2026-07-25", "09:30:00.000", "2026-07-25T09:30:00.000000000", "2026-07-25T00:00:00.0Z"))
            .row("offset", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T09:30:00+09:00"))
            .row("offset to the second", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00-01:30:15"))
            .row("nanoseconds", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00.123456789Z"))
            .row("milliseconds", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00.5Z"))
            .row("microseconds", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00.000120Z"))
            .row("end of the day", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T24:00:00Z"))
            .row("years", "Booking",
                    ok("+10000-01-01", "09:30", "-0001-12-31T23:59:59", "-1000000000-01-01T00:00:00Z"))
            .row("the last of each", "Booking",
                    ok("+999999999-12-31", "23:59:59", "+999999999-12-31T23:59:59",
                            "+1000000000-12-31T23:59:59.999999999Z"))
            .row("a fraction in a time", "Booking",
                    ok("2026-07-25", "09:30:00.5", "2026-07-25T09:30", "2026-07-25T00:00:00Z"))
            .row("a fraction in a date-time", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30:00.000000001", "2026-07-25T00:00:00Z"))
            .row("a leap second", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2016-12-31T23:59:60Z"))
            .row("a leap second at an offset", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2016-12-31T23:59:60+09:00"))
            .row("no day", "Booking",
                    ok("2026-02-30", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00Z"))
            .row("no time", "Booking",
                    ok("2026-07-25", "24:00", "2026-07-25T09:30", "2026-07-25T00:00:00Z"))
            .row("no moment past the last", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "+1000000001-01-01T00:00:00Z"))
            .row("no zone", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00"))
            .row("no offset past eighteen hours", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00+18:01"))
            .row("a year the way nobody writes it", "Booking",
                    ok("+2026-07-25", "09:30", "2026-07-25T09:30", "2026-07-25T00:00:00Z"))
            .row("a lower case", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25t09:30", "2026-07-25T00:00:00z"))
            .row("a date-time with no time", "Booking",
                    ok("2026-07-25", "09:30", "2026-07-25", "2026-07-25T00:00:00Z"))
            .row("every one wrong", "Booking", booking("1", "true", "null", "[]"))
            .row("every one absent", "Booking", "{}");

    @Test
    void aBoundaryReadsTheTextEachTypeIsWrittenAsAndWritesItBackAsTheLanguageWritesIt() throws Exception {
        String said = AValueIsReadFromTheFormItIsWrittenInTest.run(
                Checked.of(List.of(WIRE)), ROWS.harness());

        assertThat(said).isEqualTo("""
                plain: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-25T00:00:00Z"}
                seconds written out: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-25T00:00:00Z"}
                seconds kept: value {"day":"2026-07-25","start":"09:30:05","at":"2026-07-25T09:30:05","seen":"2026-07-25T00:00:05Z"}
                fraction of nought: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-25T00:00:00Z"}
                offset: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-25T00:30:00Z"}
                offset to the second: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-25T01:30:15Z"}
                nanoseconds: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-25T00:00:00.123456789Z"}
                milliseconds: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-25T00:00:00.500Z"}
                microseconds: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-25T00:00:00.000120Z"}
                end of the day: value {"day":"2026-07-25","start":"09:30","at":"2026-07-25T09:30","seen":"2026-07-26T00:00:00Z"}
                years: value {"day":"+10000-01-01","start":"09:30","at":"-0001-12-31T23:59:59","seen":"-1000000000-01-01T00:00:00Z"}
                the last of each: value {"day":"+999999999-12-31","start":"23:59:59","at":"+999999999-12-31T23:59:59","seen":"+1000000000-12-31T23:59:59.999999999Z"}
                a fraction in a time: issues [@/start invalid_format]
                a fraction in a date-time: issues [@/at invalid_format]
                a leap second: issues [@/seen invalid_format]
                a leap second at an offset: issues [@/seen invalid_format]
                no day: issues [@/day invalid_format]
                no time: issues [@/start invalid_format]
                no moment past the last: issues [@/seen invalid_format]
                no zone: issues [@/seen invalid_format]
                no offset past eighteen hours: issues [@/seen invalid_format]
                a year the way nobody writes it: issues [@/day invalid_format]
                a lower case: issues [@/at invalid_format] [@/seen invalid_format]
                a date-time with no time: issues [@/at invalid_format]
                every one wrong: issues [@/day type_mismatch actual=number expected=String] [@/start type_mismatch actual=boolean expected=String] [@/at type_mismatch actual=null expected=String] [@/seen type_mismatch actual=array expected=String]
                every one absent: issues [@/day missing_field actual=nothing expected=a field] [@/start missing_field actual=nothing expected=a field] [@/at missing_field actual=nothing expected=a field] [@/seen missing_field actual=nothing expected=a field]
                """);
    }
}
