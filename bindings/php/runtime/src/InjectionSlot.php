<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI\CData;

/**
 * What a binding registers for one behavior a host implements.
 *
 * PHP makes a new C entry each time a closure is handed to C as a function pointer, and keeps it
 * until the request ends, so a binding that handed one over on every run would grow for as long as
 * a worker lives. The pointer is made here once, and it is that pointer which is registered around
 * each run. Which PHP implementation it calls is the innermost run's, kept here as a stack.
 *
 * Registered and put back around each run rather than once for good, so that a behavior no run has
 * bound answers `INJECTION_UNBOUND` as the library says, and a second binding of the same library
 * does not replace the first's registration for longer than its own run.
 *
 * @internal
 */
final class InjectionSlot
{
    private readonly CData $pointer;

    /** @var list<\Closure> */
    private array $implementations = [];

    /**
     * @param string $type     the C type of the function a host implements the behavior as
     * @param string $register the library's function registering one
     * @param \Closure(Session, \Closure, list<mixed>): void $adapter what the generated binding
     *        does with what C hands over: makes PHP values of it, calls the implementation with
     *        them, and writes what it answered through the room C handed over
     */
    public function __construct(
        private readonly NativeLibrary $library,
        string $type,
        private readonly string $register,
        private readonly \Closure $adapter,
    ) {
        $held = $library->ffi()->new($type . '[1]');
        $held[0] = fn (mixed ...$handed): int => $this->dispatch($handed);
        $this->pointer = $held[0];
    }

    /** Registers this for a run, answering what it replaced. */
    public function enter(\Closure $implementation): mixed
    {
        $this->implementations[] = $implementation;
        return $this->library->ffi()->{$this->register}($this->pointer);
    }

    /** Puts back what {@see enter()} replaced. */
    public function leave(mixed $replaced): void
    {
        array_pop($this->implementations);
        $this->library->ffi()->{$this->register}($replaced);
    }

    /**
     * What C calls. An exception is kept and answered as `HOST_EXCEPTION`, to be thrown again where
     * the call into the library returns; nothing else but `ANSWERED` is ever answered from here.
     *
     * @param list<mixed> $handed
     */
    private function dispatch(array $handed): int
    {
        try {
            $implementation = end($this->implementations);
            if ($implementation === false) {
                throw new InjectionProtocolViolation('the library called an implementation no run registered');
            }
            ($this->adapter)($this->library->current(), $implementation, $handed);
            return $this->library->status('ANSWERED');
        } catch (\Throwable $thrown) {
            Pending::keep($thrown);
            return $this->library->status('HOST_EXCEPTION');
        }
    }
}
