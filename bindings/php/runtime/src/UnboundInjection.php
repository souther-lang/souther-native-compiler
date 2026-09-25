<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A behavior was reached through a requirement it was handed nothing for: what it was bound to, or
 * the run it was called in, held no implementation of a behavior the host implements.
 */
final class UnboundInjection extends \LogicException implements SoutherFailure
{
}
