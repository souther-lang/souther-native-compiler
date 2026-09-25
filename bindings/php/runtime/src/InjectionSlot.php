<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;
use FFI\CData;

/**
 * What the library calls for one behavior a host implements, whichever implementation of it a
 * capability was made of.
 *
 * PHP makes a new C entry each time a closure is handed to C as a function pointer, and keeps it
 * until the request ends, so a binding that handed one over for every implementation would grow
 * for as long as a worker lives. The pointer is made here once, and every capability of an
 * implementation of the behavior is made with it; what each is handed first is a number of its own,
 * which says which implementation it calls. So two implementations of one behavior are two
 * capabilities the library can hold at once, and a call reaches the one it was constructed with.
 *
 * @internal
 */
final class InjectionSlot
{
    private readonly CData $pointer;

    /** @var array<int, \Closure> by the number each capability is handed */
    private array $implementations = [];

    private int $next = 1;

    /**
     * @param string $type      the C type of the function a host implements the behavior as
     * @param string $implement the library's function making a capability of one
     * @param \Closure(Session, \Closure, list<mixed>): void $adapter what the generated binding
     *        does with what C hands over: makes PHP values of it, calls the implementation with
     *        them, and writes what it answered through the room C handed over
     */
    public function __construct(
        private readonly NativeLibrary $library,
        string $type,
        private readonly string $implement,
        private readonly \Closure $adapter,
    ) {
        $held = $library->ffi()->new($type . '[1]');
        $held[0] = fn (mixed ...$handed): int => $this->dispatch($handed);
        $this->pointer = $held[0];
    }

    /** The capability of `$implementation`, kept for as long as what this answers is. */
    public function implement(\Closure $implementation): Hosted
    {
        $ffi = $this->library->ffi();
        $token = $this->next++;
        $this->implementations[$token] = $implementation;
        $capability = $ffi->new('souther_capability');
        $hosted = $ffi->new('souther_hosted');
        $userdata = $ffi->new('int64_t');
        $userdata->cdata = $token;
        $ffi->{$this->implement}(FFI::addr($capability), FFI::addr($hosted), $this->pointer,
            FFI::addr($userdata));
        return new Hosted($this, $token, $capability, $hosted, $userdata);
    }

    /** @internal Lets go of the implementation a capability made through this called. */
    public function release(int $token): void
    {
        unset($this->implementations[$token]);
    }

    /**
     * What C calls, handed first the number of the implementation it is for. An exception is kept
     * and answered as `HOST_EXCEPTION`, to be thrown again where the call into the library returns;
     * nothing else but `ANSWERED` is ever answered from here.
     *
     * @param list<mixed> $handed
     */
    private function dispatch(array $handed): int
    {
        try {
            $userdata = array_shift($handed);
            $token = $userdata === null ? 0
                : $this->library->ffi()->cast('int64_t *', $userdata)[0];
            $implementation = $this->implementations[$token]
                ?? throw new InjectionProtocolViolation(
                    'the library called an implementation nothing holds a capability of');
            ($this->adapter)($this->library->current(), $implementation, $handed);
            return $this->library->status('ANSWERED');
        } catch (\Throwable $thrown) {
            Pending::keep($thrown);
            return $this->library->status('HOST_EXCEPTION');
        }
    }
}
