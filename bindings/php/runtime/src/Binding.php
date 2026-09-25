<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A library a binding was generated for, and the runs a host makes of it. The generated binding
 * extends this with how to load its library, what each behavior a host implements is adapted
 * through, and what each behavior is constructed from.
 */
abstract class Binding
{
    /**
     * Which version of what a generated binding calls of this runtime this is. It moves when a
     * binding generated before would call something this does not have, or call it as something it
     * is not; a binding says which it was generated for and refuses to load over any other.
     */
    public const PROTOCOL = 4;

    /**
     * What each version of the protocol moved, by its number, oldest first. The versions before the
     * first here are in the history of this file.
     *
     * A change that moves the protocol adds its line at the end under the next number, and moves
     * {@see PROTOCOL} to it, which a test holds to the last key here. Two branches each moving to
     * the same number add two different lines at one place, which a merge stops at; two edits of
     * one constant to the same number merge without a word.
     */
    public const MOVES = [
        2 => 'a list is handed over and read back as a PHP list, packed into columns and unpacked '
            . 'into elements (Session::list, Session::elements)',
        3 => 'a class per behavior, bound to what it requires: registering implementations for a '
            . 'call is InjectionRegistry\'s, apart from the run\'s arena (Bound, InjectionRegistry)',
        4 => 'a behavior is called with the capabilities of what it was bound to, in the order it '
            . 'requires them, and nothing is registered: what a run is handed is what the behaviors '
            . 'it calls are constructed from (Bound, Implemented, InjectionSlot, Session)',
    ];

    /**
     * @param array<string, InjectionSlot> $slots by the declared name of the behavior each is for
     * @param array<string, array{?string, list<string>}> $constructions by the declared name of
     *        each published behavior: what makes a capability of it, where something may require
     *        it, and the declared name of each behavior it requires, in order
     */
    protected function __construct(
        private readonly NativeLibrary $library,
        array $slots,
        array $constructions,
    ) {
        $library->adopt($slots, $constructions);
    }

    /**
     * Runs `$body` in a session of its own, handed `$injections`.
     *
     * The arena is marked before and put back after, so every value made in the run is refused once
     * it ends, and anything that has to outlive it leaves as its external form (`encode()`).
     *
     * A behavior called through `Behaviors` in the run is constructed from `$injections`: each
     * behavior a host implements that it requires, at any depth, is the one implementation handed
     * here for it, and nothing where none is. What a behavior is bound to otherwise is its own
     * (the class generated for it), and holds two implementations of one behavior where binding
     * said so.
     *
     * @template T
     * @param callable(Session): T $body
     * @return T
     */
    public function run(callable $body, Injections ...$injections): mixed
    {
        $ffi = $this->library->ffi();
        $mark = $ffi->souther_mark();
        $session = $this->library->open(array_merge(...array_map(
            static fn (Injections $set): array => $set->implementations(), $injections)));
        try {
            return $body($session);
        } finally {
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
