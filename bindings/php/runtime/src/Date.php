<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A Souther `Date` as PHP holds one: a day of the proleptic ISO calendar, in a year a
 * `java.time.LocalDate` holds, as its year, month and day.
 *
 * It is not a `\DateTimeInterface`, which is a moment in a zone: a `Date` has neither. `of` takes
 * the day a `\DateTimeInterface` falls on in its own zone. It is written as `LocalDate.toString`
 * writes it, which is the text the library takes for it.
 *
 * Refused where the calendar has no such day, or the year is past what a `Date` holds.
 */
final class Date implements \Stringable
{
    public function __construct(
        public readonly int $year,
        public readonly int $month,
        public readonly int $day,
    ) {
        if (!Calendar::isDay($year, $month, $day)) {
            throw new \InvalidArgumentException("{$year}-{$month}-{$day} is no day a Date holds");
        }
    }

    /** The day `$at` falls on in its own zone. */
    public static function of(\DateTimeInterface $at): self
    {
        return new self((int) $at->format('Y'), (int) $at->format('n'), (int) $at->format('j'));
    }

    public function __toString(): string
    {
        return Calendar::dateText($this->year, $this->month, $this->day);
    }
}
