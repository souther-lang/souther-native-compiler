<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A library of another ABI generation than the one this runtime calls
 * ({@see NativeLibrary::ABI_GENERATION}), or one from before a library said which it was: refused
 * where it is loaded, before any of its functions is called.
 */
final class UnsupportedGeneration extends \RuntimeException
{
}
