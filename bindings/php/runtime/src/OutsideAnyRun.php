<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A function of a binding called with no run of its library going on this fiber. What it would make
 * has no run to belong to, so a host opens one with `Binding::run` and calls it inside.
 */
final class OutsideAnyRun extends \LogicException implements SoutherFailure
{
}
