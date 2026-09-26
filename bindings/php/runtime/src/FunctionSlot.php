<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;
use FFI\CData;

/**
 * What the library calls for one function type a binding hands closures over as, whichever closure
 * a function value was made of.
 *
 * For the reason {@see InjectionSlot} is one per behavior: PHP makes a new C entry each time a
 * closure is handed to C as a function pointer and keeps it until the request ends, so the pointer
 * is made here once, and every function value of a closure of the type is made with it. What each
 * is handed first is a number of its own, which says which closure it calls.
 *
 * A function value is a value of the run it is handed over in, as every other value is: the room it
 * is laid out in and the closure are kept by the run's session and let go of when the run ends, and
 * the library holds nothing of it past then.
 *
 * @internal
 */
final class FunctionSlot
{
    private readonly CData $pointer;

    /** @var array<int, \Closure> by the number each function value is handed */
    private array $closures = [];

    private int $next = 1;

    /**
     * @param string $type      the C type of the function the library calls a closure through
     * @param string $implement the library's function making a function value of one
     * @param \Closure(Session, \Closure, list<mixed>): void $adapter what the generated binding
     *        does with what C hands over: makes PHP values of it, calls the closure with them, and
     *        writes what it answered through the room C handed over
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

    /**
     * A function value of `$closure`, for the length of `$session`'s run: what the session keeps is
     * what the value reads, so the library may call it until the run ends and no later.
     */
    public function implement(Session $session, \Closure $closure): CData
    {
        $ffi = $session->ffi();
        $token = $this->next++;
        $this->closures[$token] = $closure;
        $room = $ffi->new('souther_hosted_function');
        $userdata = $ffi->new('int64_t');
        $userdata->cdata = $token;
        $session->keep(new HostedFunction($this, $token, $room, $userdata));
        return $ffi->{$this->implement}(FFI::addr($room), $this->pointer, FFI::addr($userdata));
    }

    /** @internal Lets go of the closure a function value made through this called. */
    public function release(int $token): void
    {
        unset($this->closures[$token]);
    }

    /**
     * What C calls, handed first the number of the closure it is for. An exception is kept and
     * answered as `HOST_EXCEPTION`, to be thrown again where the call into the library returns;
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
            $closure = $this->closures[$token]
                ?? throw new InjectionProtocolViolation(
                    'the library called a function value whose run has ended');
            ($this->adapter)($this->library->current(), $closure, $handed);
            return $this->library->status('ANSWERED');
        } catch (\Throwable $thrown) {
            Pending::keep($thrown);
            return $this->library->status('HOST_EXCEPTION');
        }
    }
}
