<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * What a behavior was bound to, as binding it said: the objects implementing the behaviors a host
 * implements that it requires itself, and what each behavior it requires that is constructed in
 * turn was bound to.
 *
 * Kept as that and not as one table. The requirement set is what a behavior is constructed with,
 * and on the JVM each behavior constructed in turn holds its own. The library cannot yet: it
 * registers one implementation of a behavior at a time, whoever calls it (#72). So what is
 * registered around a call is flattened out of this, and where that would take two different
 * implementations of one behavior, binding is refused rather than letting whichever was registered
 * last answer for both. How the library is handed what was bound is decided in the flattening, and
 * nothing a host writes changes with it.
 *
 * @internal
 */
final class Bound
{
    /** @var array<string, \Closure> */
    private readonly array $registered;

    /**
     * @param array<string, object> $implementers by the declared name of the behavior each
     *        implements, each an instance of the class generated for it
     * @param list<self> $constructed what each behavior this requires that is constructed in turn
     *        was bound to
     */
    private function __construct(
        private readonly array $implementers,
        private readonly array $constructed,
    ) {
        $this->registered = array_map(
            static fn (object $implementer): \Closure => $implementer->apply(...),
            $this->flattened());
    }

    /**
     * What is bound to `$implementers`, and to what each of `$constructed` was bound to.
     *
     * @param array<string, object> $implementers by the declared name of the behavior each implements
     */
    public static function of(array $implementers, self ...$constructed): self
    {
        return new self($implementers, array_values($constructed));
    }

    /**
     * Runs `$body` with what this is bound to registered, in the run `$session` is for.
     *
     * @template T
     * @param callable(): T $body
     * @return T
     */
    public function around(Session $session, callable $body): mixed
    {
        return $session->withInjections($body, $this->registered);
    }

    /**
     * Every implementer anywhere in what this was bound to, by the behavior it implements: what the
     * library can be handed while it registers one of each.
     *
     * @return array<string, object>
     */
    private function flattened(): array
    {
        $flat = $this->implementers;
        foreach ($this->constructed as $bound) {
            foreach ($bound->flattened() as $behavior => $implementer) {
                $already = $flat[$behavior] ?? null;
                if ($already !== null && $already !== $implementer) {
                    throw new \InvalidArgumentException("{$behavior} is bound to two"
                        . ' implementations, and the library calls one implementation of it at a time');
                }
                $flat[$behavior] = $implementer;
            }
        }
        return $flat;
    }
}
