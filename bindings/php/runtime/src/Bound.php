<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;
use FFI\CData;

/**
 * What a behavior was bound to, as binding it said: what stands for each behavior it requires, in
 * the order the behavior requires them. Each is an implementation a host wrote of a behavior it
 * implements ({@see Implemented}), or what a behavior constructed in turn was bound to, or nothing,
 * where the run was handed nothing for it.
 *
 * The library is called with the capability of each, laid out as binding said, so two of these
 * that stand for one behavior with two implementations each call their own: the requirement set is
 * what a behavior is constructed with, and each constructed behavior holds its own, as on the JVM.
 * What the library is handed is made here the first time a call asks for it and kept for as long as
 * this is, since what is called reads it for as long as a value made in the call can.
 *
 * @internal
 */
final class Bound implements Requirement
{
    /**
     * What was made for the library, by the library: the capabilities of what this requires, laid
     * out one after another, and the capability of this.
     *
     * @var array<int, array{?CData, ?CData}>
     */
    private array $made = [];

    /**
     * @param ?string $bind the library's function making the capability of the behavior, where
     *        something may require it
     * @param list<?Requirement> $requires
     */
    private function __construct(
        private readonly ?string $bind,
        private readonly array $requires,
    ) {
    }

    /**
     * What is bound to `$requires`, in the order the behavior requires them, through `$bind`.
     */
    public static function of(?string $bind, ?Requirement ...$requires): self
    {
        return new self($bind, array_values($requires));
    }

    /**
     * What the behavior is called with: the address of the capabilities of what it requires, in
     * order, or null where it requires nothing. A place nothing was handed for holds null, which the
     * library answers `INJECTION_UNBOUND` for where it is reached.
     */
    public function requirements(Session $session): ?CData
    {
        return $this->made($session)[0];
    }

    public function capability(Session $session): CData
    {
        $capability = $this->made($session)[1]
            ?? throw new \LogicException('a behavior nothing may require was required');
        return FFI::addr($capability);
    }

    /** @return array{?CData, ?CData} */
    private function made(Session $session): array
    {
        return $this->made[spl_object_id($session->library())] ??= $this->make($session);
    }

    /** @return array{?CData, ?CData} */
    private function make(Session $session): array
    {
        $ffi = $session->ffi();
        $requirements = null;
        if ($this->requires !== []) {
            $requirements = $ffi->new('const souther_capability *[' . count($this->requires) . ']');
            foreach ($this->requires as $at => $required) {
                $requirements[$at] = $required?->capability($session);
            }
        }
        $capability = null;
        if ($this->bind !== null) {
            $capability = $ffi->new('souther_capability');
            $ffi->{$this->bind}(FFI::addr($capability), $requirements);
        }
        return [$requirements, $capability];
    }
}
