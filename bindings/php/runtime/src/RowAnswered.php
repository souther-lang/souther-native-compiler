<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * What the implementation answered a row whose answer is owed (`<?>`). The row states nothing an
 * answer could keep, so this is neither kept nor not kept: it is what a person reads to write the
 * answer the row owes.
 */
final readonly class RowAnswered implements RowOutcome
{
    /** @param string $answer what the implementation answered, in the language's external form */
    public function __construct(public string $answer)
    {
    }
}
