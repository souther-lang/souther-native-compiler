<?php

declare(strict_types=1);

namespace Souther\Runtime;

/** The implementation's answer keeps the row, as the language's equality says. */
final readonly class RowKept implements RowOutcome
{
    /** @param string $answer what the implementation answered, in the language's external form, for a report */
    public function __construct(public string $answer)
    {
    }
}
