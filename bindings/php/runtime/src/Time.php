<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A Souther `Time` as PHP holds one: a time of day, held to the second, as its hour, minute and
 * second. It is written as `LocalTime.toString` writes it, `:ss` only where the second is not
 * nought, which is the text the library takes for it.
 *
 * Refused where the numbers name no time of day.
 */
final class Time implements \Stringable
{
    public function __construct(
        public readonly int $hour,
        public readonly int $minute,
        public readonly int $second = 0,
    ) {
        if (!Calendar::isTime($hour, $minute, $second)) {
            throw new \InvalidArgumentException("{$hour}:{$minute}:{$second} is no time of day");
        }
    }

    public function __toString(): string
    {
        return Calendar::timeText($this->hour, $this->minute, $this->second);
    }
}
