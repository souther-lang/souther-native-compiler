<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * The row's run ended without an answer: the implementation threw, a behavior was reached with
 * nothing handed for it, or the computation stopped. Not an answer that does not keep the row,
 * which is a verdict on an answer; here there was no answer to hold to the row.
 */
final readonly class RowEnded implements RowOutcome
{
    /**
     * @param \Throwable $reason what a call of a behavior would have thrown for the same end: the
     *        exception the implementation threw, itself, an `UnboundInjection`, or a `SoutherAbort`
     *        naming the status
     */
    public function __construct(public \Throwable $reason)
    {
    }
}
