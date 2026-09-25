<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * What a behavior was bound to: for each behavior a host implements that calling it reaches, the
 * object implementing it, registered around each call.
 *
 * The library registers one implementation of a behavior at a time, whoever calls it. So what one
 * behavior is bound to has one implementation of each: a behavior bound to another that was bound
 * to a different implementation of something it is bound to as well is refused where it is bound,
 * and not answered by whichever of the two happened to be registered last.
 *
 * @internal
 */
final class Bound
{
    /** @var array<string, \Closure> */
    private readonly array $implementations;

    /**
     * @param array<string, object> $implementers by the declared name of the behavior each
     *        implements, each an instance of the class generated for it
     */
    private function __construct(private readonly array $implementers)
    {
        $this->implementations = array_map(
            static fn (object $implementer): \Closure => $implementer->apply(...), $implementers);
    }

    /**
     * What is bound to `$implementers`, and to what each of `$constructed` was bound to.
     *
     * @param array<string, object> $implementers by the declared name of the behavior each implements
     */
    public static function of(array $implementers, self ...$constructed): self
    {
        $joined = $implementers;
        foreach ($constructed as $bound) {
            foreach ($bound->implementers as $behavior => $implementer) {
                $already = $joined[$behavior] ?? null;
                if ($already !== null && $already !== $implementer) {
                    throw new \InvalidArgumentException("{$behavior} is bound to two"
                        . ' implementations, and the library calls one implementation of it at a time');
                }
                $joined[$behavior] = $implementer;
            }
        }
        return new self($joined);
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
        return $session->withInjections($body, $this->implementations);
    }
}
