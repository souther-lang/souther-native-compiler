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
     * @param class-string<Binding> $binding the binding the implementations were written against,
     *        whose classes they take and answer
     * @param array<string, \Closure> $implementations by the declared name of the behavior each
     *        implements
     */
    protected function __construct(
        private readonly string $binding,
        private readonly array $implementations,
    ) {
    }

    /**
     * @internal
     * @return array<string, Implemented>
     */
    public function implemented(): array
    {
        $implemented = [];
        foreach ($this->implementations as $behavior => $implementation) {
            $implemented[$behavior] = Implemented::by($this->binding, $behavior, $implementation);
        }
        return $implemented;
    }
}
