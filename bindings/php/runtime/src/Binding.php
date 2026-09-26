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
    public const PROTOCOL = 9;

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
        4 => 'a type\'s decode as a raoh-php decoder over a PHP value, composed like a JVM type\'s '
            . 'decoder() (Session::decoder)',
        5 => 'a binding\'s functions take no session and ask for the innermost run going, and a run\'s '
            . 'body is called with nothing (Binding::innermostOf, Binding::run)',
        6 => 'a decoder writes a PHP value with every array as an object and hands it to the library\'s '
            . 'reading of a host\'s value, which reads an array keyed by its indices as a list '
            . '(Session::decoder)',
        7 => 'a behavior is called with the capabilities of what it was bound to, in the order it '
            . 'requires them, and nothing is registered: what a run is handed is what the behaviors '
            . 'it calls are constructed from (Bound, Implemented, InjectionSlot, Session)',
        8 => 'a tuple is a PHP list of its members, an optional of what may be null holds a Some, '
            . 'and a function value is a Closure: one the library answered is called through '
            . 'Session::callable, and one PHP hands over is made through the FunctionSlot the '
            . 'binding keeps for its type (Binding::hosting, FunctionSlot, Some)',
        9 => 'a Decimal is its integer and its scale, handed over and read back through the '
            . 'runtime (Decimal, Session::decimal, Session::amount)',
    ];

    /**
     * @param array<string, InjectionSlot> $slots by the declared name of the behavior each is for
     * @param array<string, FunctionSlot> $functions by what the generated binding calls the
     *        function type each is for
     * @param array<string, array{?string, list<string>}> $constructions by the declared name of
     *        each behavior a host constructs, whether or not it calls it by name: what makes a
     *        capability of it, where something may require it, and the declared name of each
     *        behavior it requires, in order
     * @param list<string> $injected the declared name of every behavior a host implements, as the
     *        library says, whether or not this binding has a way to adapt an implementation of it
     */
    protected function __construct(
        private readonly NativeLibrary $library,
        private readonly array $slots,
        private readonly array $functions,
        private readonly array $constructions,
        private readonly array $injected,
    ) {
    }

    /**
     * @internal This binding, as it was loaded for `$library`: what the generated class holds for
     * each library it was loaded for.
     *
     * What adapts an implementation to the classes a binding generated is the binding's, and one
     * library may be loaded by bindings generated under two namespaces; so an implementation reaches
     * the adapter of the binding it was written against through this, and never through the
     * library, which the bindings share.
     */
    abstract public static function in(NativeLibrary $library): static;

    /**
     * @internal The session every function of the binding is called in: the innermost run going
     * of a library it was loaded for ({@see innermostOf()}).
     */
    abstract public static function session(): Session;

    /** @internal What `$behavior`, which a host implements, is adapted to this binding through. */
    public function slot(string $behavior): InjectionSlot
    {
        return $this->slots[$behavior]
            ?? throw new \InvalidArgumentException("this binding implements no {$behavior}");
    }

    /** @internal What a closure of the function type `$type` is handed to the library through. */
    public function hosting(string $type): FunctionSlot
    {
        return $this->functions[$type]
            ?? throw new \InvalidArgumentException("this binding hands over no function of {$type}");
    }

    /**
     * @internal What `$behavior` is called with where it is called through this binding's
     * `Behaviors` in `$session`'s run: the capabilities of what it requires, constructed from what
     * the run was handed ({@see run()}), or null where it requires nothing.
     *
     * Each implementation the run was handed keeps the binding it was written against, so a
     * behavior called through one binding in another's run is handed the other's implementations
     * through the other's adapters.
     */
    public function requirementsOf(Session $session, string $behavior): ?\FFI\CData
    {
        return $this->constructedAs($session, $behavior)->requirements($session);
    }

    private function constructedAs(Session $session, string $behavior): Bound
    {
        return $session->constructedAs(static::class . "\0" . $behavior,
            function () use ($session, $behavior): Bound {
                [$bind, $requires] = $this->constructions[$behavior]
                    ?? throw new \InvalidArgumentException("this binding constructs no {$behavior}");
                $handed = [];
                // Which of the two a requirement is, is what the library says, and not whether this
                // binding could adapt an implementation of it: one it could not is one the run was
                // handed nothing for.
                foreach ($requires as $required) {
                    $handed[] = in_array($required, $this->injected, true)
                        ? $session->injected($required)
                        : $this->constructedAs($session, $required);
                }
                return Bound::of($bind, ...$handed);
            });
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
     * The body is handed nothing. What it calls of the binding finds this run itself, as the
     * innermost one going ({@see innermostOf()}).
     *
     * @template T
     * @param callable(): T $body
     * @return T
     */
    public function run(callable $body, Injections ...$injections): mixed
    {
        $ffi = $this->library->ffi();
        $mark = $ffi->souther_mark();
        $session = $this->library->open(array_merge(...array_map(
            static fn (Injections $set): array => $set->implemented(), $injections)));
        try {
            return $body();
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

    /**
     * @internal The session every function of a generated binding is called in: the innermost run
     * going on this fiber of any library the binding was loaded for.
     *
     * A computation is refused through any session but the innermost run's of its library
     * ({@see Session::call()}), and a library's runs are on one fiber at a time, so there is one
     * session a function could be called in and nothing for a caller to choose. Where the binding
     * was loaded for two libraries and both have a run going, the one opened later is inside the
     * other and is the one meant.
     *
     * @param array<int, self> $bindings every binding the generated class was loaded as
     */
    protected static function innermostOf(array $bindings): Session
    {
        $innermost = null;
        foreach ($bindings as $binding) {
            $here = $binding->library->innermostHere();
            if ($here !== null && ($innermost === null || $here->openedAfter($innermost))) {
                $innermost = $here;
            }
        }
        if ($innermost === null) {
            throw new OutsideAnyRun('a function of a Souther binding was called outside any run of'
                . ' its library; call it inside Binding::run');
        }
        return $innermost;
    }
}
