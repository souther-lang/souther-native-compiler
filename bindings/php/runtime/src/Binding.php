<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A library a binding was generated for, and the runs a host makes of it. The generated binding
 * extends this with how to load its library and what each behavior a host implements is adapted
 * through.
 */
abstract class Binding
{
    /**
     * Which version of what a generated binding calls of this runtime this is. It moves when a
     * binding generated before would call something this does not have, or call it as something it
     * is not; a binding says which it was generated for and refuses to load over any other.
     */
    public const PROTOCOL = 1;

    /**
     * @param array<string, InjectionSlot> $slots by the declared name of the behavior each is for
     */
    protected function __construct(
        private readonly NativeLibrary $library,
        private readonly array $slots,
    ) {
    }

    /**
     * Runs `$body` in a session of its own, with `$injections` registered for its length.
     *
     * The arena is marked before and put back after, so every value made in the run is refused once
     * it ends, and anything that has to outlive it leaves as its external form (`encode()`). What
     * was registered before is registered again after, so runs nest.
     *
     * @template T
     * @param callable(Session): T $body
     * @return T
     */
    public function run(callable $body, Injections ...$injections): mixed
    {
        $ffi = $this->library->ffi();
        $mark = $ffi->souther_mark();
        $session = $this->library->open();
        $entered = [];
        try {
            foreach ($injections as $set) {
                foreach ($set->implementations() as $behavior => $implementation) {
                    $slot = $this->slots[$behavior]
                        ?? throw new \InvalidArgumentException(
                            "the library asks no host to implement {$behavior}");
                    $entered[] = [$slot, $slot->enter($implementation)];
                }
            }
            return $body($session);
        } finally {
            foreach (array_reverse($entered) as [$slot, $replaced]) {
                $slot->leave($replaced);
            }
            $this->library->close($session);
            $ffi->souther_reset($mark);
        }
    }

    /** @internal */
    public function library(): NativeLibrary
    {
        return $this->library;
    }
}
