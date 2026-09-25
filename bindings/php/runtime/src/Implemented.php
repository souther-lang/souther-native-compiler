<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI\CData;

/**
 * An implementation a host wrote of a behavior it implements, standing where the behavior is
 * required.
 *
 * @internal
 */
final class Implemented implements Requirement
{
    /** @var array<int, Hosted> by the library it was handed to */
    private array $hosted = [];

    private function __construct(
        private readonly string $behavior,
        private readonly \Closure $implementation,
    ) {
    }

    /**
     * `$implementation` of `$behavior`, by its declared name: called with the session of the
     * innermost run going and what the behavior takes, and answering what it answers.
     */
    public static function by(string $behavior, \Closure $implementation): self
    {
        return new self($behavior, $implementation);
    }

    public function capability(Session $session): CData
    {
        $library = $session->library();
        return ($this->hosted[spl_object_id($library)]
            ??= $library->slot($this->behavior)->implement($this->implementation))->capability();
    }
}
