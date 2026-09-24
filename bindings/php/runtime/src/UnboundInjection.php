<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A behavior the host implements was reached with no implementation registered for it: the run it
 * was called in was not handed one.
 */
final class UnboundInjection extends \LogicException implements SoutherFailure
{
}
