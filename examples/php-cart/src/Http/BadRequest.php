<?php

declare(strict_types=1);

namespace App\Http;

use Raoh\Issues;

/** A request that does not decode, with raoh-php's issues. The application answers it with a 400. */
final class BadRequest extends \RuntimeException
{
    public function __construct(public readonly Issues $issues)
    {
        parent::__construct('the request does not decode');
    }

    public static function of(Issues $issues): self
    {
        return new self($issues);
    }
}
