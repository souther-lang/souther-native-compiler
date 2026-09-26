<?php
// The same surface twice: once spelt with `self`, `parent`, `?T` and a union in one order, and once
// with the class names, `T|null` and the union in another. A listing that is a fingerprint has to
// read them alike, since which spelling PHP's Reflection answers for one is not the surface.
class Base {}
final class Fixture extends Base
{
    public ?self $next = null;
    public function of(self $a, ?self $b, parent $c, int|string|null $d, ?string $e): self { return $a; }
    public static function make(): ?self { return null; }
    public function later(): static { return $this; }
    public function both(Countable&Traversable $x): int|string { return 0; }
}
final class Explicit extends Base
{
    public Explicit|null $next = null;
    public function of(Explicit $a, Explicit|null $b, Base $c, string|null|int $d, string|null $e): Explicit { return $a; }
    public static function make(): Explicit|null { return null; }
    public function later(): static { return $this; }
    public function both(Traversable&Countable $x): string|int { return 0; }
}
