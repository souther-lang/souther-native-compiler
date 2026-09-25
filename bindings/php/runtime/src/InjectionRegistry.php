<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * What a binding registers implementations through, for the length of a call: a run's, or one of
 * a behavior bound to its implementations ({@see Bound}).
 *
 * Apart from the arena, which only a run marks and resets: a bound behavior is called inside a run
 * and answers values of that run, so what it registers has to end with the call and nothing else.
 *
 * @internal
 */
final class InjectionRegistry
{
    /**
     * @param array<string, InjectionSlot> $slots by the declared name of the behavior each is for
     */
    public function __construct(private readonly array $slots)
    {
    }

    /**
     * Runs `$body` with each of `$registered` registered, in order, and what was registered before
     * registered again after, so that calls nest.
     *
     * @template T
     * @param callable(): T $body
     * @param array<string, \Closure> ...$registered each by the declared name of the behavior its
     *        implementations implement
     * @return T
     */
    public function around(callable $body, array ...$registered): mixed
    {
        $entered = [];
        try {
            foreach ($registered as $implementations) {
                foreach ($implementations as $behavior => $implementation) {
                    $slot = $this->slots[$behavior]
                        ?? throw new \InvalidArgumentException(
                            "the library asks no host to implement {$behavior}");
                    $entered[] = [$slot, $slot->enter($implementation)];
                }
            }
            return $body();
        } finally {
            foreach (array_reverse($entered) as [$slot, $replaced]) {
                $slot->leave($replaced);
            }
        }
    }
}
