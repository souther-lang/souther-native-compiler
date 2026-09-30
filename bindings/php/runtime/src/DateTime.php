<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A Souther `DateTime` as PHP holds one: a date and a time of day, in no zone. It is written as
 * `LocalDateTime.toString` writes it, which is for PHP to show: the library is handed its numbers.
 */
final class DateTime implements \Stringable
{
    public function __construct(public readonly Date $date, public readonly Time $time)
    {
    }

    public function __toString(): string
    {
        return $this->date . 'T' . $this->time;
    }
}
