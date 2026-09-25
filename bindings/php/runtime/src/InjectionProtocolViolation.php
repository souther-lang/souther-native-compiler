<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * An implementation a host wrote answered in a way the library does not take: anything but an
 * answer or an exception. A constructor refusing a value inside an implementation arrives as this,
 * since an implementation is outside the model and cannot end a computation with a model's abort.
 */
final class InjectionProtocolViolation extends \LogicException implements SoutherFailure
{
}
