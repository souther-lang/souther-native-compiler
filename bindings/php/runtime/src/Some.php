<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * An optional holding a value, where what it holds may itself be null: an optional of an optional.
 *
 * An optional is null where it holds nothing and what it holds where it holds something, which is
 * one value too few where what it holds may be null: `Int??` holding an optional holding nothing and
 * `Int??` holding nothing would both be null. So an optional whose value may be null holds it in
 * this, and each depth of absence is its own.
 *
 * @template T
 */
final readonly class Some
{
    /** @param T $value */
    public function __construct(public mixed $value)
    {
    }
}
