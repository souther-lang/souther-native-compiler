<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI\CData;

/**
 * A function value the library made of a closure PHP handed over, and what it reads the closure
 * out of. Kept by the session of the run it was made in; the closure is let go of when this is.
 *
 * @internal
 */
final class HostedFunction
{
    /** @internal */
    public function __construct(
        private readonly FunctionSlot $slot,
        private readonly int $token,
        private readonly CData $room,
        private readonly CData $userdata,
    ) {
    }

    public function __destruct()
    {
        $this->slot->release($this->token);
    }
}
