<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI\CData;

/**
 * What stands where a behavior is required: something the library makes a capability of, which the
 * behavior requiring it is called with.
 *
 * @internal
 */
interface Requirement
{
    /**
     * The address of the capability, made the first time it is asked for in `$session`'s library
     * and kept for as long as this is: what is called through it reads it for as long as the call
     * goes, and a function value made in the call may carry it further, to the end of the run.
     */
    public function capability(Session $session): CData;
}
