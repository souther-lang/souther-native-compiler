<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * What the runtime throws, apart from a host's own exception coming back out of a call.
 */
interface SoutherFailure extends \Throwable
{
}
