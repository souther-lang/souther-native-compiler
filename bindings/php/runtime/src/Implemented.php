<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI\CData;

/**
 * An implementation a host wrote of a behavior it implements, standing where the behavior is
 * required, with the binding it was written against.
 *
 * The binding is part of it and not of where it is used: what the library hands the implementation
 * is made into the classes of that binding, and what it answers is read back out of them, by that
 * binding's adapter. A library loaded by two bindings holds two adapters for one behavior, and the
 * one an implementation is called through is its own binding's.
 *
 * @internal
 */
final class Implemented implements Requirement
{
    /** @var array<int, Hosted> by the library it was handed to */
    private array $hosted = [];

    /** @param class-string<Binding> $binding */
    private function __construct(
        private readonly string $binding,
        private readonly string $behavior,
        private readonly \Closure $implementation,
    ) {
    }

    /**
     * `$implementation` of `$behavior`, by its declared name, written against `$binding`: called in
     * the innermost run going with what the behavior takes, and answering what it answers, as that
     * binding's classes.
     *
     * @param class-string<Binding> $binding
     */
    public static function by(string $binding, string $behavior, \Closure $implementation): self
    {
        return new self($binding, $behavior, $implementation);
    }

    public function capability(Session $session): CData
    {
        $library = $session->library();
        return ($this->hosted[spl_object_id($library)] ??= ($this->binding)::in($library)
            ->slot($this->behavior)->implement($this->implementation))->capability();
    }
}
