<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A computation started through the session of a run that has another run going inside it. What
 * it would make belongs to the inner run, which ends first, so it is refused rather than handed
 * back as a value of the outer one.
 */
final class NotTheInnermostRun extends \LogicException implements SoutherFailure
{
}
