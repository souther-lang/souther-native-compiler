<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * @internal What `Date`, `Time`, `DateTime` and `Instant` share: the calendar they count days in,
 * the range a `java.time` type holds, and the text `java.time` writes for each.
 *
 * A value is held as the numbers it means and handed to the library as those numbers, which is how
 * the library takes one and answers one (souther-native-compiler#137). It is checked where it is
 * made, so that a value a PHP program has is always one: the library decides the same again where
 * it is handed one, and a refusal from it is this binding and the library disagreeing. The text is
 * for a PHP program to show, and never crosses to the library.
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
}
