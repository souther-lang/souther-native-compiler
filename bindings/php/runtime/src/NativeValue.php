<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A value of a type a Souther library declares, held where the library made it.
 */
interface NativeValue
{
    /** @internal What a generated binding hands the library for this value. */
    public function nativeHandle(): NativeHandle;
}
