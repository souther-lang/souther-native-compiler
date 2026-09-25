<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;
use FFI\CData;

/**
 * The capability the library made of one implementation a host wrote, and what it reads the
 * implementation out of. Kept for as long as something may call through the capability; the
 * implementation is let go of when this is.
 *
 * @internal
 */
final class Hosted
{
    /** @internal */
    public function __construct(
        private readonly InjectionSlot $slot,
        private readonly int $token,
        private readonly CData $capability,
        private readonly CData $hosted,
        private readonly CData $userdata,
    ) {
    }

    /** The address of the capability. */
    public function capability(): CData
    {
        return FFI::addr($this->capability);
    }

    public function __destruct()
    {
        $this->slot->release($this->token);
    }
}
