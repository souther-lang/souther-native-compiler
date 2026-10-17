<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * There was nothing to run: the compile could not read the row ({@see RowKind::Unreproducible}),
 * or the library has no way to hold an answer to what it states ({@see RowKind::NotComparable}).
 * Nothing was called.
 */
final readonly class RowNotRun implements RowOutcome
{
    /** @param string $why why there was nothing to run, as {@see Row::$why} says it */
    public function __construct(public string $why)
    {
    }
}
