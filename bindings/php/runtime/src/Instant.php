<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A Souther `Instant` as PHP holds one: a moment, as the second from 1970-01-01T00:00:00Z and the
 * nanosecond within it, in the range `java.time.Instant` holds. It is written as `Instant.toString`
 * writes it, in UTC with a fraction only where there is one, which is for PHP to show: the library
 * is handed its numbers.
 *
 * Refused where the nanosecond is not below a second, or the moment is past what an `Instant`
 * holds.
 */
final class Instant implements \Stringable
{
    public function __construct(public readonly int $second, public readonly int $nano = 0)
    {
        if (!Calendar::isMoment($second, $nano)) {
            throw new \InvalidArgumentException(
                "{$second} seconds and {$nano} nanoseconds is no moment an Instant holds");
        }
    }

    /** The moment `$at` is, which is the same in any zone, to the microsecond PHP holds. */
    public static function of(\DateTimeInterface $at): self
    {
        return new self($at->getTimestamp(), (int) $at->format('u') * 1_000);
    }

    /** This moment in UTC, to the microsecond, which is as near as PHP holds one. */
    public function toDateTime(): \DateTimeImmutable
    {
        return (new \DateTimeImmutable('@' . $this->second))
            ->setMicrosecond(intdiv($this->nano, 1_000));
    }

    public function __toString(): string
    {
        return Calendar::instantText($this->second, $this->nano);
    }
}
