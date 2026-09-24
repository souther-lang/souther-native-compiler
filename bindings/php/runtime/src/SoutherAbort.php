<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A Souther computation that ended without a value: a division by zero, an `ensures` that did not
 * hold, and every other abort the library numbers.
 *
 * Thrown and not answered as an `Err`. An `Err` carries issues about input a caller handed over,
 * and an abort is not that: it is the computation itself stopping.
 */
final class SoutherAbort extends \RuntimeException implements SoutherFailure
{
    public function __construct(public readonly string $status, int $number)
    {
        parent::__construct("the computation ended without a value: {$status}", $number);
    }
}
