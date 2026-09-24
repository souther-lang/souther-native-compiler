<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * Implementations a host hands a run for behaviors of one module, generated as a class per module
 * with one parameter for each behavior it asks a host to implement.
 */
abstract class Injections
{
    /**
     * @param array<string, \Closure> $implementations by the declared name of the behavior each
     *        implements
     */
    protected function __construct(private readonly array $implementations)
    {
    }

    /**
     * @internal
     * @return array<string, \Closure>
     */
    public function implementations(): array
    {
        return $this->implementations;
    }
}
