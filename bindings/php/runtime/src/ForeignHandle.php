<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A value one library made, handed to a function of another.
 */
final class ForeignHandle extends \LogicException implements SoutherFailure
{
}
