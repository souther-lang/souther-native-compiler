package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.time.DateTimeException;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.temporal.ChronoUnit;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;
import java.util.function.Supplier;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every kernel and comparison over a {@code Date}, a {@code Time}, a {@code DateTime} and an
 * {@code Instant}, held to what {@code java.time} answers for the same program.
 *
 * <p>The rows are the oracle for what a program states, as they are for the other kernels: a row
 * that ran and kept its answer is one the JVM answered, and the native run of it is held to that
 * answer. The rows are few and chosen for where the two carriers could part; what covers the
 * calendar is {@link #theCalendarIsWhatJavaTimeAnswersOverAThousandChosenAndRandomCases}, which
 * asks the object the same question of {@code java.time} over the ends of every range, the days
 * either side of a leap day and a seeded run of the rest, and holds the two answers alike.
 */
class ATemporalAnswersWhatTheJvmAnswersTest {

    private static final String TEMPORALS = """
            module temporals

            behavior addedDays : (n: Int, d: Date) -> Date
            let addedDays (n, d) = Date.addDays(n, d)

            behavior addedMonths : (n: Int, d: Date) -> Date
            let addedMonths (n, d) = Date.addMonths(n, d)

            behavior addedYears : (n: Int, d: Date) -> Date
            let addedYears (n, d) = Date.addYears(n, d)

            behavior daysBetween : (a: Date, b: Date) -> Int
            let daysBetween (a, b) = Date.daysBetween(a, b)

            behavior yearOf : (d: Date) -> Int
            let yearOf (d) = Date.year(d)

            behavior monthOf : (d: Date) -> Int
            let monthOf (d) = Date.month(d)

            behavior dayOf : (d: Date) -> Int
            let dayOf (d) = Date.day(d)

            behavior dateFromParts : (y: Int, m: Int, d: Int) -> Int
            let dateFromParts (y, m, d) = match Date.fromParts(y, m, d) with
                | Date as day -> Date.year(day) * 10000 + Date.month(day) * 100 + Date.day(day)
                | NotADate -> -1

            behavior timeFromParts : (h: Int, m: Int, s: Int) -> Int
            let timeFromParts (h, m, s) = match Time.fromParts(h, m, s) with
                | Time as t -> Time.hour(t) * 10000 + Time.minute(t) * 100 + Time.second(t)
                | NotATime -> -1

            behavior hourOf : (t: Time) -> Int
            let hourOf (t) = Time.hour(t)

            behavior minuteOf : (t: Time) -> Int
            let minuteOf (t) = Time.minute(t)

            behavior secondOf : (t: Time) -> Int
            let secondOf (t) = Time.second(t)

            behavior addedMinutes : (n: Int, dt: DateTime) -> DateTime
            let addedMinutes (n, dt) = DateTime.addMinutes(n, dt)

            behavior addedHours : (n: Int, dt: DateTime) -> DateTime
            let addedHours (n, dt) = DateTime.addHours(n, dt)

            behavior addedDaysTo : (n: Int, dt: DateTime) -> DateTime
            let addedDaysTo (n, dt) = DateTime.addDays(n, dt)

            behavior minutesBetween : (a: DateTime, b: DateTime) -> Int
            let minutesBetween (a, b) = DateTime.minutesBetween(a, b)

            behavior dateOf : (dt: DateTime) -> Date
            let dateOf (dt) = DateTime.toDate(dt)

            behavior timeOf : (dt: DateTime) -> Time
            let timeOf (dt) = DateTime.toTime(dt)

            behavior joined : (d: Date, t: Time) -> DateTime
            let joined (d, t) = DateTime.fromDateAndTime(d, t)

            behavior sameDate : (a: Date, b: Date) -> Bool
            let sameDate (a, b) = a == b

            behavior sameTime : (a: Time, b: Time) -> Bool
            let sameTime (a, b) = a == b

            behavior sameDateTime : (a: DateTime, b: DateTime) -> Bool
            let sameDateTime (a, b) = a == b

            behavior sameInstant : (a: Instant, b: Instant) -> Bool
            let sameInstant (a, b) = a == b

            behavior dateBelow : (a: Date, b: Date) -> Bool
            let dateBelow (a, b) = a < b

            behavior timeBelow : (a: Time, b: Time) -> Bool
            let timeBelow (a, b) = a < b

            behavior dateTimeBelow : (a: DateTime, b: DateTime) -> Bool
            let dateTimeBelow (a, b) = a < b

            behavior instantBelow : (a: Instant, b: Instant) -> Bool
            let instantBelow (a, b) = a < b

            behavior instantAtMost : (a: Instant, b: Instant) -> Bool
            let instantAtMost (a, b) = a <= b

            behavior dateThrough : (d: Date) -> Date
            let dateThrough (d) = d

            behavior timeThrough : (t: Time) -> Time
            let timeThrough (t) = t

            behavior dateTimeThrough : (dt: DateTime) -> DateTime
            let dateTimeThrough (dt) = dt

            behavior instantThrough : (i: Instant) -> Instant
            let instantThrough (i) = i

            behavior sortedDates : (xs: List<Date>) -> List<Date>
            let sortedDates (xs) = List.sort(xs)

            behavior sortedInstants : (xs: List<Instant>) -> List<Instant>
            let sortedInstants (xs) = List.sort(xs)

            behavior rentDay : (a: Int) -> Date
            let rentDay (a) = Date("2026-07-25")

            behavior opening : (a: Int) -> Time
            let opening (a) = Time("09:00:00")

            behavior meeting : (a: Int) -> DateTime
            let meeting (a) = DateTime("2026-07-25T09:00")

            behavior launch : (a: Int) -> Instant
            let launch (a) = Instant("2026-07-25T00:00:00.5Z")

            behavior spelledOtherwise : (a: Int) -> Bool
            let spelledOtherwise (a) =
                Date("+010000-01-01") == Date("+10000-01-01")
                    && Time("09:30:00.") == Time("09:30")
                    && DateTime("2026-07-01t09:30") == DateTime("2026-07-01T09:30")
                    && DateTime("2026-07-01T09:30:00.") == DateTime("2026-07-01T09:30")
                    && Instant("2026-07-01T00:00:00.Z") == Instant("2026-07-01T00:00:00Z")

            example spelledOtherwise
                | "spellings java.time reads and a boundary does not" : (0) -> true

            example rentDay
                | "a literal" : (0) -> Date("2026-07-25")

            example opening
                | "written to the second, held as the time it names" : (0) -> Time("09:00")

            example meeting
                | "a literal" : (0) -> DateTime("2026-07-25T09:00")

            example launch
                | "a fraction of a second" : (0) -> Instant("2026-07-25T00:00:00.500Z")

            example addedDays
                | "forward across a month" : (30, Date("2026-01-31")) -> Date("2026-03-02")
                | "backward across a year" : (-365, Date("2026-01-01")) -> Date("2025-01-01")
                | "across a leap day" : (1, Date("2024-02-28")) -> Date("2024-02-29")
                | "before the epoch" : (-1, Date("1970-01-01")) -> Date("1969-12-31")
                | "to the first date" : (-1, Date("-999999999-01-02")) -> Date("-999999999-01-01")
                | "to the last date" : (1, Date("+999999999-12-30")) -> Date("+999999999-12-31")

            example addedMonths
                | "the last day of a shorter month" : (1, Date("2026-01-31")) -> Date("2026-02-28")
                | "a leap February" : (1, Date("2024-01-31")) -> Date("2024-02-29")
                | "backward across a year" : (-13, Date("2026-01-31")) -> Date("2024-12-31")
                | "none" : (0, Date("2026-01-31")) -> Date("2026-01-31")
                | "a year and a month" : (13, Date("2026-12-31")) -> Date("2028-01-31")

            example addedYears
                | "the twenty-ninth of February" : (1, Date("2024-02-29")) -> Date("2025-02-28")
                | "four years on" : (4, Date("2024-02-29")) -> Date("2028-02-29")
                | "before year nought" : (-2030, Date("2026-07-25")) -> Date("-0004-07-25")

            example daysBetween
                | "forward" : (Date("2026-07-25"), Date("2026-08-25")) -> 31
                | "backward" : (Date("2026-08-25"), Date("2026-07-25")) -> -31
                | "the same day" : (Date("2026-07-25"), Date("2026-07-25")) -> 0
                | "across a leap day" : (Date("2024-02-28"), Date("2024-03-01")) -> 2

            example yearOf
                | "a year" : (Date("2026-07-25")) -> 2026
                | "before year nought" : (Date("-0001-12-31")) -> -1

            example monthOf
                | "a month" : (Date("2026-07-25")) -> 7

            example dayOf
                | "a day" : (Date("2026-07-25")) -> 25

            example dateFromParts
                | "a day" : (2026, 7, 25) -> 20260725
                | "the last of February in a leap year" : (2024, 2, 29) -> 20240229
                | "the twenty-ninth of February in another" : (2026, 2, 29) -> -1
                | "month thirteen" : (2026, 13, 1) -> -1
                | "day nought" : (2026, 1, 0) -> -1
                | "a year no date holds" : (1000000000, 1, 1) -> -1

            example timeFromParts
                | "a time" : (9, 30, 5) -> 93005
                | "the last second" : (23, 59, 59) -> 235959
                | "hour twenty-four" : (24, 0, 0) -> -1
                | "minute sixty" : (0, 60, 0) -> -1
                | "the leap second" : (23, 59, 60) -> -1

            example hourOf
                | "an hour" : (Time("09:30:05")) -> 9

            example minuteOf
                | "a minute" : (Time("09:30:05")) -> 30

            example secondOf
                | "a second" : (Time("09:30:05")) -> 5

            example addedMinutes
                | "across midnight" : (90, DateTime("2026-07-25T23:00")) -> DateTime("2026-07-26T00:30")
                | "backward across a year" : (-1, DateTime("2026-01-01T00:00")) -> DateTime("2025-12-31T23:59")

            example addedHours
                | "across a day" : (25, DateTime("2026-07-25T09:30:15")) -> DateTime("2026-07-26T10:30:15")

            example addedDaysTo
                | "a leap day" : (1, DateTime("2024-02-28T12:00")) -> DateTime("2024-02-29T12:00")

            example minutesBetween
                | "forward" : (DateTime("2026-07-25T09:00"), DateTime("2026-07-25T10:30")) -> 90
                | "backward" : (DateTime("2026-07-25T10:30"), DateTime("2026-07-25T09:00")) -> -90
                | "across a day" : (DateTime("2026-07-25T23:59"), DateTime("2026-07-26T00:01")) -> 2
                | "a part of a minute forward" : (DateTime("2026-07-25T09:00:00"), DateTime("2026-07-25T09:00:59")) -> 0
                | "a part of a minute backward" : (DateTime("2026-07-25T09:00:59"), DateTime("2026-07-25T09:00:00")) -> 0
                | "just over a minute backward" : (DateTime("2026-07-25T09:01:01"), DateTime("2026-07-25T09:00:00")) -> -1

            example dateOf
                | "the day" : (DateTime("2026-07-25T09:30")) -> Date("2026-07-25")
                | "before the epoch" : (DateTime("1969-12-31T23:59:59")) -> Date("1969-12-31")

            example timeOf
                | "the time" : (DateTime("2026-07-25T09:30:15")) -> Time("09:30:15")
                | "before the epoch" : (DateTime("1969-12-31T23:59:59")) -> Time("23:59:59")

            example joined
                | "a date and a time" : (Date("2026-07-25"), Time("09:30")) -> DateTime("2026-07-25T09:30")

            example sameDate
                | "made apart" : (Date("2026-07-25"), Date("2026-07-25")) -> true
                | "a day apart" : (Date("2026-07-25"), Date("2026-07-26")) -> false

            example sameTime
                | "written with and without its seconds" : (Time("09:30"), Time("09:30:00")) -> true
                | "a second apart" : (Time("09:30"), Time("09:30:01")) -> false

            example sameDateTime
                | "written with and without its seconds" : (DateTime("2026-07-25T09:30"), DateTime("2026-07-25T09:30:00")) -> true
                | "a second apart" : (DateTime("2026-07-25T09:30"), DateTime("2026-07-25T09:30:01")) -> false

            example sameInstant
                | "written with and without its zeros" : (Instant("2026-07-25T00:00:00.5Z"), Instant("2026-07-25T00:00:00.500Z")) -> true
                | "a nanosecond apart" : (Instant("2026-07-25T00:00:00Z"), Instant("2026-07-25T00:00:00.000000001Z")) -> false

            example dateBelow
                | "before" : (Date("2026-07-25"), Date("2026-07-26")) -> true
                | "the same" : (Date("2026-07-25"), Date("2026-07-25")) -> false
                | "before year nought" : (Date("-0001-01-01"), Date("0000-01-01")) -> true
                | "past year 9999" : (Date("9999-12-31"), Date("+10000-01-01")) -> true

            example timeBelow
                | "before" : (Time("09:30"), Time("09:30:01")) -> true
                | "after" : (Time("23:59:59"), Time("00:00")) -> false

            example dateTimeBelow
                | "before" : (DateTime("2026-07-25T23:59:59"), DateTime("2026-07-26T00:00")) -> true
                | "before the epoch" : (DateTime("1969-12-31T23:59:59"), DateTime("1970-01-01T00:00")) -> true

            example instantBelow
                | "a nanosecond" : (Instant("2026-07-25T00:00:00.000000001Z"), Instant("2026-07-25T00:00:00.000000002Z")) -> true
                | "a second" : (Instant("2026-07-25T00:00:00.999999999Z"), Instant("2026-07-25T00:00:01Z")) -> true
                | "before the epoch" : (Instant("1969-12-31T23:59:59.999999999Z"), Instant("1970-01-01T00:00:00Z")) -> true
                | "the same" : (Instant("2026-07-25T00:00:00Z"), Instant("2026-07-25T00:00:00Z")) -> false

            example instantAtMost
                | "the same" : (Instant("2026-07-25T00:00:00Z"), Instant("2026-07-25T00:00:00Z")) -> true
                | "after" : (Instant("2026-07-25T00:00:01Z"), Instant("2026-07-25T00:00:00.999999999Z")) -> false

            example dateThrough
                | "written as it is" : (Date("2026-07-25")) -> Date("2026-07-25")
                | "the last date" : (Date("+999999999-12-31")) -> Date("+999999999-12-31")
                | "year nought" : (Date("0000-01-01")) -> Date("0000-01-01")

            example timeThrough
                | "written to the second" : (Time("09:30:00")) -> Time("09:30")
                | "the last second" : (Time("23:59:59")) -> Time("23:59:59")

            example dateTimeThrough
                | "written to the second" : (DateTime("2026-07-25T09:30:00")) -> DateTime("2026-07-25T09:30")

            example instantThrough
                | "written as it is" : (Instant("2026-07-25T00:00:00Z")) -> Instant("2026-07-25T00:00:00Z")
                | "the last moment" : (Instant("+1000000000-12-31T23:59:59.999999999Z")) -> Instant("+1000000000-12-31T23:59:59.999999999Z")
                | "the first moment" : (Instant("-1000000000-01-01T00:00:00Z")) -> Instant("-1000000000-01-01T00:00:00Z")

            example sortedDates
                | "chronologically" : ([Date("2026-07-25"), Date("-0001-01-01"), Date("2026-01-31")]) -> [Date("-0001-01-01"), Date("2026-01-31"), Date("2026-07-25")]

            example sortedInstants
                | "to the nanosecond" : ([Instant("2026-07-25T00:00:00.000000002Z"), Instant("2026-07-25T00:00:00.000000001Z"), Instant("2026-07-25T00:00:00Z")]) -> [Instant("2026-07-25T00:00:00Z"), Instant("2026-07-25T00:00:00.000000001Z"), Instant("2026-07-25T00:00:00.000000002Z")]
            """;

    @Test
    void everyTemporalRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(TEMPORALS);
    }

    /**
     * A shift off the end of what a temporal holds ends the run, for the one reason the operation's
     * contract names (spec §a-shift-off-the-end-of-a-temporal-aborts): a count too large to add is
     * not told apart from a day past the end.
     */
    @Test
    void aShiftOffTheEndOfATemporalEndsTheRun() throws Exception {
        Asked temporals = new Asked(TEMPORALS);
        RunOutcome noPlace = new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE);
        ObservedValue lastDate = date("+999999999-12-31");
        ObservedValue firstDate = date("-999999999-01-01");
        assertThat(temporals.outcome("addedDays", integer(1), lastDate)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedDays", integer(-1), firstDate)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedDays", integer(Long.MAX_VALUE), lastDate)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedMonths", integer(1), lastDate)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedMonths", integer(Long.MIN_VALUE), firstDate)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedYears", integer(1), lastDate)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedYears", integer(-1), firstDate)).isEqualTo(noPlace);
        ObservedValue lastMoment = dateTime("+999999999-12-31T23:59:59");
        ObservedValue firstMoment = dateTime("-999999999-01-01T00:00");
        assertThat(temporals.outcome("addedMinutes", integer(1), lastMoment)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedMinutes", integer(-1), firstMoment)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedHours", integer(Long.MAX_VALUE), lastMoment)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedDaysTo", integer(1), lastMoment)).isEqualTo(noPlace);
        assertThat(temporals.outcome("addedDaysTo", integer(Long.MIN_VALUE), firstMoment)).isEqualTo(noPlace);
        // And the last step that lands, either side of the end.
        assertThat(temporals.outcome("addedDays", integer(-1), lastDate))
                .isEqualTo(answered(date("+999999999-12-30")));
        assertThat(temporals.outcome("addedDaysTo", integer(0), lastMoment))
                .isEqualTo(answered(lastMoment));
    }

    /**
     * A value a host makes of the text that names it is the value, and what it is read back as is
     * the text the boundary writes: to the second for a time, and in UTC for an instant.
     */
    @Test
    void aTemporalHandedOverIsTheValueItsTextNames() throws Exception {
        Asked temporals = new Asked(TEMPORALS);
        assertThat(temporals.outcome("dateThrough", date("2026-07-25")))
                .isEqualTo(answered(date("2026-07-25")));
        assertThat(temporals.outcome("timeThrough", time("09:30:00")))
                .isEqualTo(answered(time("09:30")));
        assertThat(temporals.outcome("dateTimeThrough", dateTime("2026-07-25T09:30:05")))
                .isEqualTo(answered(dateTime("2026-07-25T09:30:05")));
        assertThat(temporals.outcome("instantThrough", instant("2026-07-25T09:30:00+09:00")))
                .isEqualTo(answered(instant("2026-07-25T00:30:00Z")));
        assertThat(temporals.outcome("instantThrough", instant("2026-07-25T00:30:00.120Z")))
                .isEqualTo(answered(instant("2026-07-25T00:30:00.120Z")));
        assertThat(temporals.outcome("sameInstant", instant("2026-07-25T09:30:00+09:00"),
                instant("2026-07-25T00:30:00Z"))).isEqualTo(answered(new ObservedValue.Bool(true)));
    }

    /**
     * The whole of the calendar, against {@code java.time}: each behavior asked what {@code java.time}
     * is asked, over the ends of every range, the days either side of a leap day, and a seeded run
     * of numbers in between. A calendar is where a carrier that is right on the days a test thinks of
     * parts from the one that is right on the rest.
     */
    @Test
    void theCalendarIsWhatJavaTimeAnswersOverAThousandChosenAndRandomCases() throws Exception {
        Asked temporals = new Asked(TEMPORALS);
        Random random = new Random(89);
        List<LocalDate> dates = new ArrayList<>(List.of(
                LocalDate.MIN, LocalDate.MIN.plusDays(1), LocalDate.MAX, LocalDate.MAX.minusDays(1),
                LocalDate.of(0, 1, 1), LocalDate.of(-1, 12, 31), LocalDate.of(1970, 1, 1),
                LocalDate.of(1969, 12, 31), LocalDate.of(2024, 2, 29), LocalDate.of(2024, 2, 28),
                LocalDate.of(2023, 2, 28), LocalDate.of(1900, 2, 28), LocalDate.of(2000, 2, 29),
                LocalDate.of(9999, 12, 31), LocalDate.of(10000, 1, 1), LocalDate.of(-10000, 1, 1),
                LocalDate.of(2026, 1, 31), LocalDate.of(2026, 7, 25)));
        for (int i = 0; i < 60; i++) {
            dates.add(LocalDate.ofEpochDay(
                    random.nextLong(LocalDate.MIN.toEpochDay(), LocalDate.MAX.toEpochDay() + 1)));
            dates.add(LocalDate.ofEpochDay(random.nextLong(-800_000, 800_000)));
        }
        long[] counts = {0, 1, -1, 28, 29, 30, 31, 365, 366, -366, 1461, 12, -12, 13, 1_000_000,
                -1_000_000, 999_999_999, -999_999_999, 1_999_999_998L, Integer.MAX_VALUE,
                Long.MAX_VALUE, Long.MIN_VALUE, 365_241_780_471L, -365_243_219_162L};

        int cases = 0;
        for (LocalDate date : dates) {
            ObservedValue given = date(date.toString());
            assertThat(temporals.outcome("yearOf", given)).as(date.toString())
                    .isEqualTo(answered(integer(date.getYear())));
            assertThat(temporals.outcome("monthOf", given)).as(date.toString())
                    .isEqualTo(answered(integer(date.getMonthValue())));
            assertThat(temporals.outcome("dayOf", given)).as(date.toString())
                    .isEqualTo(answered(integer(date.getDayOfMonth())));
            assertThat(temporals.outcome("dateThrough", given)).as(date.toString())
                    .isEqualTo(answered(given));
            for (int at = 0; at < 4; at++) {
                long count = counts[random.nextInt(counts.length)];
                cases += shifted(temporals, "addedDays", count, given, () -> date.plusDays(count));
                cases += shifted(temporals, "addedMonths", count, given, () -> date.plusMonths(count));
                cases += shifted(temporals, "addedYears", count, given, () -> date.plusYears(count));
            }
            LocalDate other = dates.get(random.nextInt(dates.size()));
            ObservedValue against = date(other.toString());
            assertThat(temporals.outcome("daysBetween", given, against)).as(date + " to " + other)
                    .isEqualTo(answered(integer(ChronoUnit.DAYS.between(date, other))));
            assertThat(temporals.outcome("dateBelow", given, against)).as(date + " < " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(date.isBefore(other))));
            assertThat(temporals.outcome("sameDate", given, against)).as(date + " == " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(date.equals(other))));
            cases += 5;
        }

        List<LocalDateTime> moments = new ArrayList<>(List.of(
                LocalDateTime.MIN.withNano(0), LocalDateTime.MAX.withNano(0),
                LocalDateTime.of(1970, 1, 1, 0, 0), LocalDateTime.of(1969, 12, 31, 23, 59, 59),
                LocalDateTime.of(2024, 2, 29, 23, 59, 59), LocalDateTime.of(2026, 7, 25, 9, 30)));
        for (int i = 0; i < 80; i++) {
            moments.add(LocalDateTime.ofEpochSecond(
                    random.nextLong(LocalDateTime.MIN.toEpochSecond(java.time.ZoneOffset.UTC),
                            LocalDateTime.MAX.toEpochSecond(java.time.ZoneOffset.UTC)),
                    0, java.time.ZoneOffset.UTC));
            moments.add(LocalDateTime.ofEpochSecond(random.nextLong(-3_000_000_000L, 5_000_000_000L),
                    0, java.time.ZoneOffset.UTC));
        }
        long[] smalls = {0, 1, -1, 59, 60, 61, -61, 1439, 1440, 1441, 86_400, -86_400, 1_000_000,
                -1_000_000_000L, Long.MAX_VALUE, Long.MIN_VALUE, Long.MAX_VALUE / 60, Long.MIN_VALUE / 60};
        for (LocalDateTime moment : moments) {
            ObservedValue given = dateTime(moment.toString());
            assertThat(temporals.outcome("dateTimeThrough", given)).as(moment.toString())
                    .isEqualTo(answered(given));
            assertThat(temporals.outcome("dateOf", given)).as(moment.toString())
                    .isEqualTo(answered(date(moment.toLocalDate().toString())));
            assertThat(temporals.outcome("timeOf", given)).as(moment.toString())
                    .isEqualTo(answered(time(moment.toLocalTime().toString())));
            assertThat(temporals.outcome("joined", date(moment.toLocalDate().toString()),
                    time(moment.toLocalTime().toString()))).as(moment.toString())
                    .isEqualTo(answered(given));
            for (int at = 0; at < 3; at++) {
                long count = smalls[random.nextInt(smalls.length)];
                cases += shifted(temporals, "addedMinutes", count, given, () -> moment.plusMinutes(count));
                cases += shifted(temporals, "addedHours", count, given, () -> moment.plusHours(count));
                cases += shifted(temporals, "addedDaysTo", count, given, () -> moment.plusDays(count));
            }
            LocalDateTime other = moments.get(random.nextInt(moments.size()));
            ObservedValue against = dateTime(other.toString());
            assertThat(temporals.outcome("minutesBetween", given, against)).as(moment + " to " + other)
                    .isEqualTo(answered(integer(ChronoUnit.MINUTES.between(moment, other))));
            assertThat(temporals.outcome("dateTimeBelow", given, against)).as(moment + " < " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(moment.isBefore(other))));
            assertThat(temporals.outcome("sameDateTime", given, against)).as(moment + " == " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(moment.equals(other))));
            cases += 8;
        }

        for (int i = 0; i < 120; i++) {
            LocalTime clock = LocalTime.ofSecondOfDay(random.nextInt(86_400));
            LocalTime other = LocalTime.ofSecondOfDay(random.nextInt(86_400));
            assertThat(temporals.outcome("hourOf", time(clock.toString()))).as(clock.toString())
                    .isEqualTo(answered(integer(clock.getHour())));
            assertThat(temporals.outcome("minuteOf", time(clock.toString()))).as(clock.toString())
                    .isEqualTo(answered(integer(clock.getMinute())));
            assertThat(temporals.outcome("secondOf", time(clock.toString()))).as(clock.toString())
                    .isEqualTo(answered(integer(clock.getSecond())));
            assertThat(temporals.outcome("timeThrough", time(clock.toString()))).as(clock.toString())
                    .isEqualTo(answered(time(clock.toString())));
            assertThat(temporals.outcome("timeBelow", time(clock.toString()), time(other.toString())))
                    .as(clock + " < " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(clock.isBefore(other))));
            assertThat(temporals.outcome("sameTime", time(clock.toString()), time(other.toString())))
                    .as(clock + " == " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(clock.equals(other))));
            cases += 6;
        }

        long[] partsOf = {-1, 0, 1, 2, 12, 13, 24, 28, 29, 30, 31, 32, 59, 60, 61, 1999, 2000, 2023, 2024,
                999_999_999, 1_000_000_000, -999_999_999, -1_000_000_000, 2_147_483_648L,
                Long.MAX_VALUE, Long.MIN_VALUE};
        for (int i = 0; i < 400; i++) {
            long year = partsOf[random.nextInt(partsOf.length)];
            long month = partsOf[random.nextInt(partsOf.length)];
            long day = partsOf[random.nextInt(partsOf.length)];
            long expected;
            try {
                LocalDate named = LocalDate.of(Math.toIntExact(year), Math.toIntExact(month),
                        Math.toIntExact(day));
                expected = named.getYear() * 10000L + named.getMonthValue() * 100L + named.getDayOfMonth();
            } catch (DateTimeException | ArithmeticException notADate) {
                expected = -1;
            }
            assertThat(temporals.outcome("dateFromParts", integer(year), integer(month), integer(day)))
                    .as(year + "-" + month + "-" + day).isEqualTo(answered(integer(expected)));
            long hour = partsOf[random.nextInt(partsOf.length)];
            long minute = partsOf[random.nextInt(partsOf.length)];
            long second = partsOf[random.nextInt(partsOf.length)];
            try {
                LocalTime named = LocalTime.of(Math.toIntExact(hour), Math.toIntExact(minute),
                        Math.toIntExact(second));
                expected = named.getHour() * 10000L + named.getMinute() * 100L + named.getSecond();
            } catch (DateTimeException | ArithmeticException notATime) {
                expected = -1;
            }
            assertThat(temporals.outcome("timeFromParts", integer(hour), integer(minute), integer(second)))
                    .as(hour + ":" + minute + ":" + second).isEqualTo(answered(integer(expected)));
            cases += 2;
        }

        List<Instant> instants = new ArrayList<>(List.of(
                Instant.MIN, Instant.MAX, Instant.EPOCH, Instant.EPOCH.minusNanos(1),
                Instant.parse("2026-07-25T00:00:00.5Z"), Instant.parse("2026-07-25T00:00:00.000001Z"),
                Instant.parse("2026-07-25T00:00:00.123456789Z")));
        for (int i = 0; i < 100; i++) {
            instants.add(Instant.ofEpochSecond(
                    random.nextLong(Instant.MIN.getEpochSecond(), Instant.MAX.getEpochSecond()),
                    random.nextInt(4) == 0 ? random.nextInt(1_000_000_000) : 0));
            instants.add(Instant.ofEpochSecond(random.nextLong(-3_000_000_000L, 5_000_000_000L),
                    random.nextInt(3) == 0 ? random.nextInt(1_000_000) * 1000L : random.nextInt(1_000_000_000)));
        }
        for (Instant moment : instants) {
            Instant other = instants.get(random.nextInt(instants.size()));
            assertThat(temporals.outcome("instantThrough", instant(moment.toString())))
                    .as(moment.toString()).isEqualTo(answered(instant(moment.toString())));
            assertThat(temporals.outcome("instantBelow", instant(moment.toString()), instant(other.toString())))
                    .as(moment + " < " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(moment.isBefore(other))));
            assertThat(temporals.outcome("instantAtMost", instant(moment.toString()), instant(other.toString())))
                    .as(moment + " <= " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(!moment.isAfter(other))));
            assertThat(temporals.outcome("sameInstant", instant(moment.toString()), instant(other.toString())))
                    .as(moment + " == " + other)
                    .isEqualTo(answered(new ObservedValue.Bool(moment.equals(other))));
            cases += 4;
        }
        assertThat(cases).isGreaterThan(1000);
    }

    /**
     * One shift asked of the object and of {@code java.time}: the date it lands on, or that the run
     * ends because it lands nowhere. {@code java.time} refuses in two ways, by an exception of its
     * own past what a date holds and by an arithmetic one past what a count of days holds, and the
     * language says one abort for both.
     */
    private static int shifted(Asked temporals, String behavior, long count, ObservedValue given,
                               Supplier<Object> expected) throws Exception {
        RunOutcome wanted;
        try {
            wanted = answered(switch (expected.get()) {
                case LocalDate landed -> date(landed.toString());
                case LocalDateTime landed -> dateTime(landed.toString());
                default -> throw new AssertionError("a shift lands on a date or on a date-time");
            });
        } catch (DateTimeException | ArithmeticException off) {
            wanted = new RunOutcome.Aborted(AbortKind.REQUIRED_FORM_HAS_NO_PLACE);
        }
        assertThat(temporals.outcome(behavior, integer(count), given))
                .as(behavior + " " + count + " " + given)
                .isEqualTo(wanted);
        return 1;
    }

    /**
     * The value as the checker observes it, which is what an answer is compared as: a date as
     * {@code LocalDate.toString} writes it, a clock always to the second, and an instant in UTC.
     */
    private static ObservedValue date(String written) {
        return new ObservedValue.Temporal(LocalDate.parse(written).toString());
    }

    private static ObservedValue time(String written) {
        return new ObservedValue.Temporal(
                souther.compiler.numeric.Times.written(LocalTime.parse(written)));
    }

    private static ObservedValue dateTime(String written) {
        return new ObservedValue.Temporal(
                souther.compiler.numeric.DateTimes.written(LocalDateTime.parse(written)));
    }

    private static ObservedValue instant(String written) {
        return new ObservedValue.Temporal(Instant.parse(written).toString());
    }

    private static ObservedValue integer(long value) {
        return new ObservedValue.Integer(value);
    }

    private static RunOutcome answered(ObservedValue value) {
        return new RunOutcome.Answered(value);
    }

    /** A program checked once and asked as many questions as a test has. */
    private static final class Asked {

        private final CheckedProgram program;
        private final Running running;

        Asked(String source) {
            this.program = Checked.of(List.of(source));
            this.running = Running.of(program);
        }

        RunOutcome outcome(String behavior, ObservedValue... handed) throws Exception {
            CheckedModule module = program.modules().getFirst();
            CheckedBehavior reached = module.behaviors().stream()
                    .filter(it -> it.name().name().equals(behavior))
                    .findFirst()
                    .orElseThrow(() -> new AssertionError("no behavior " + behavior));
            return running.answeredOrEnded(module, reached, List.of(handed));
        }
    }
}
