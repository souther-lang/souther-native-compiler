<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A value, or the session it was made in, used after the run it belongs to ended. Its memory has
 * been handed back to the library, so it is refused before anything reads it.
 */
final class Expired extends \LogicException implements SoutherFailure
{
}
