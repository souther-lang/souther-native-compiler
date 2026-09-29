<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * @internal What `Date`, `Time`, `DateTime` and `Instant` share: the calendar they count days in,
 * the range a `java.time` type holds, and the text `java.time` writes for each, which is what the
 * library takes and what it writes back.
 *
 * The library takes a temporal as the text that names it and ends the process on text that names
 * none, so a value is held as its numbers, checked where it is made, and handed over as that text.
 */
final class Calendar
{
    private const MIN_YEAR = -999_999_999;
    private const MAX_YEAR = 999_999_999;

    /**
     * The first and the last second an `Instant` holds, as `java.time.Instant` has them:
     * -1000000000-01-01T00:00:00Z to +1000000000-12-31T23:59:59.999999999Z.
     */
    private const MIN_MOMENT = -31_557_014_167_219_200;
    private const MAX_MOMENT = 31_556_889_864_403_199;

    private const SECONDS_PER_DAY = 86_400;

    private function __construct()
    {
    }

    public static function isDay(int $year, int $month, int $day): bool
    {
        return $year >= self::MIN_YEAR && $year <= self::MAX_YEAR && $month >= 1 && $month <= 12
            && $day >= 1 && $day <= self::monthLength($year, $month);
    }

    public static function isTime(int $hour, int $minute, int $second): bool
    {
        return $hour >= 0 && $hour <= 23 && $minute >= 0 && $minute <= 59 && $second >= 0
            && $second <= 59;
    }

    private static function monthLength(int $year, int $month): int
    {
        return match ($month) {
            1, 3, 5, 7, 8, 10, 12 => 31,
            4, 6, 9, 11 => 30,
            default => self::isLeap($year) ? 29 : 28,
        };
    }

    private static function isLeap(int $year): bool
    {
        return self::floorMod($year, 4) === 0
            && (self::floorMod($year, 100) !== 0 || self::floorMod($year, 400) === 0);
    }

    private static function floorDiv(int $a, int $b): int
    {
        $quotient = intdiv($a, $b);
        return ($a % $b !== 0 && ($a < 0) !== ($b < 0)) ? $quotient - 1 : $quotient;
    }

    private static function floorMod(int $a, int $b): int
    {
        return $a - self::floorDiv($a, $b) * $b;
    }

    /**
     * The days from 1970-01-01 to a day of the proleptic Gregorian calendar (Howard Hinnant's
     * days_from_civil), as the library's runtime counts them.
     */
    private static function daysFromCivil(int $year, int $month, int $day): int
    {
        if ($month <= 2) {
            $year--;
        }
        $era = self::floorDiv($year, 400);
        $yearOfEra = $year - $era * 400;
        $monthFromMarch = $month > 2 ? $month - 3 : $month + 9;
        $dayOfYear = intdiv(153 * $monthFromMarch + 2, 5) + $day - 1;
        $dayOfEra = $yearOfEra * 365 + intdiv($yearOfEra, 4) - intdiv($yearOfEra, 100) + $dayOfYear;
        return $era * 146_097 + $dayOfEra - 719_468;
    }

    /**
     * The year, the month and the day a count of days from 1970-01-01 names.
     *
     * @return array{int, int, int}
     */
    private static function civilFromDays(int $days): array
    {
        $days += 719_468;
        $era = self::floorDiv($days, 146_097);
        $dayOfEra = $days - $era * 146_097;
        $yearOfEra = intdiv($dayOfEra - intdiv($dayOfEra, 1_460) + intdiv($dayOfEra, 36_524)
            - intdiv($dayOfEra, 146_096), 365);
        $dayOfYear = $dayOfEra - (365 * $yearOfEra + intdiv($yearOfEra, 4) - intdiv($yearOfEra, 100));
        $monthFromMarch = intdiv(5 * $dayOfYear + 2, 153);
        $day = $dayOfYear - intdiv(153 * $monthFromMarch + 2, 5) + 1;
        $month = $monthFromMarch < 10 ? $monthFromMarch + 3 : $monthFromMarch - 9;
        $year = $yearOfEra + $era * 400;
        return [$month <= 2 ? $year + 1 : $year, $month, $day];
    }

    /**
     * A date as `LocalDate.toString` writes it: a year of at least four digits, and a sign past
     * 9999 and below nought.
     */
    public static function dateText(int $year, int $month, int $day): string
    {
        $sign = $year < 0 ? '-' : ($year > 9999 ? '+' : '');
        return sprintf('%s%04d-%02d-%02d', $sign, abs($year), $month, $day);
    }

    /** A time held to the second as `LocalTime.toString` writes it: `:ss` only where it is not nought. */
    public static function timeText(int $hour, int $minute, int $second): string
    {
        return $second === 0 ? sprintf('%02d:%02d', $hour, $minute)
            : sprintf('%02d:%02d:%02d', $hour, $minute, $second);
    }

    public static function isMoment(int $second, int $nano): bool
    {
        return $nano >= 0 && $nano < 1_000_000_000 && $second >= self::MIN_MOMENT
            && $second <= self::MAX_MOMENT;
    }

    /**
     * A moment as `Instant.toString` writes it: in UTC, with a fraction of three, six or nine digits
     * only where there is one.
     */
    public static function instantText(int $second, int $nano): string
    {
        [$year, $month, $day] = self::civilFromDays(self::floorDiv($second, self::SECONDS_PER_DAY));
        $ofDay = self::floorMod($second, self::SECONDS_PER_DAY);
        $written = self::dateText($year, $month, $day)
            . sprintf('T%02d:%02d:%02d', intdiv($ofDay, 3600), intdiv($ofDay % 3600, 60), $ofDay % 60);
        return $written . match (true) {
            $nano === 0 => '',
            $nano % 1_000_000 === 0 => sprintf('.%03d', intdiv($nano, 1_000_000)),
            $nano % 1_000 === 0 => sprintf('.%06d', intdiv($nano, 1_000)),
            default => sprintf('.%09d', $nano),
        } . 'Z';
    }

    /** The `Date` the library wrote, which is what `LocalDate.toString` writes. */
    public static function writtenDate(string $text): Date
    {
        if (preg_match('/\A' . self::DATE . '\z/', $text, $read) !== 1) {
            throw self::unread('Date', 'LocalDate', $text);
        }
        return self::date($read, 1);
    }

    /** The `Time` the library wrote, which is what `LocalTime.toString` writes of one held to the second. */
    public static function writtenTime(string $text): Time
    {
        if (preg_match('/\A' . self::TIME . '\z/', $text, $read) !== 1) {
            throw self::unread('Time', 'LocalTime', $text);
        }
        return self::time($read, 1);
    }

    /** The `DateTime` the library wrote, which is what `LocalDateTime.toString` writes. */
    public static function writtenDateTime(string $text): DateTime
    {
        if (preg_match('/\A' . self::DATE . 'T' . self::TIME . '\z/', $text, $read) !== 1) {
            throw self::unread('DateTime', 'LocalDateTime', $text);
        }
        return new DateTime(self::date($read, 1), self::time($read, 5));
    }

    /** The `Instant` the library wrote, which is what `Instant.toString` writes: in UTC. */
    public static function writtenInstant(string $text): Instant
    {
        if (preg_match('/\A' . self::DATE . 'T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?Z\z/', $text,
                $read) !== 1) {
            throw self::unread('Instant', 'Instant', $text);
        }
        // Read as numbers, not as a Date: an Instant reaches a year past what a Date holds.
        $year = $read[1] === '-' ? -(int) $read[2] : (int) $read[2];
        $second = self::daysFromCivil($year, (int) $read[3], (int) $read[4]) * self::SECONDS_PER_DAY
            + (int) $read[5] * 3600 + (int) $read[6] * 60 + (int) $read[7];
        $fraction = $read[8] ?? '';
        return new Instant($second, (int) str_pad($fraction, 9, '0'));
    }

    /** A date as `LocalDate.toString` writes it: its sign, year, month and day. */
    private const DATE = '([+-]?)(\d{4,})-(\d{2})-(\d{2})';

    /** A time of day as `LocalTime.toString` writes one held to the second. */
    private const TIME = '(\d{2}):(\d{2})(?::(\d{2}))?';

    /**
     * The `Date` of what `DATE` matched, its sign at `$sign` and its year, month and day after it.
     *
     * @param array<int, string> $read
     */
    private static function date(array $read, int $sign): Date
    {
        $year = (int) $read[$sign + 1];
        return new Date($read[$sign] === '-' ? -$year : $year, (int) $read[$sign + 2],
            (int) $read[$sign + 3]);
    }

    /**
     * The `Time` of what `TIME` matched, its hour at `$hour` and its minute and second after it.
     *
     * @param array<int, string> $read
     */
    private static function time(array $read, int $hour): Time
    {
        $second = $read[$hour + 2] ?? '';
        return new Time((int) $read[$hour], (int) $read[$hour + 1], $second === '' ? 0 : (int) $second);
    }

    private static function unread(string $type, string $java, string $text): \LogicException
    {
        return new \LogicException(
            "the library writes a {$type} as {$java}.toString does, and wrote '{$text}'");
    }
}
