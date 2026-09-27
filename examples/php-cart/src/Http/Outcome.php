<?php

declare(strict_types=1);

namespace App\Http;

/**
 * What a request came to, and whether what was written while answering it is kept.
 *
 * A route says which, where it knows what the domain answered. That the domain answered is not that
 * its answer is to be kept: an injected behavior may write before the command is refused (`loadCart`
 * makes the user's cart row), and a refused command keeps nothing it wrote on the way. A handler
 * answers an `Outcome` and not a `Response`, so no route commits without saying so.
 */
final readonly class Outcome
{
    private function __construct(public Response $response, public bool $kept)
    {
    }

    /** The command succeeded, and what it wrote is kept. */
    public static function commit(Response $response): self
    {
        return new self($response, true);
    }

    /** The command was refused, or wrote nothing worth keeping, and what it wrote is dropped. */
    public static function rollback(Response $response): self
    {
        return new self($response, false);
    }
}
