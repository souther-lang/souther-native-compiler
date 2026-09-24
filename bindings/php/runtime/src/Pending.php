<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * An exception an implementation threw, kept while the library answers the call it was in with
 * `HOST_EXCEPTION`, to be thrown again where that call returns to PHP. PHP cannot throw through a C
 * frame, so the same instance crosses the library as a status and comes back out whole.
 *
 * @internal
 */
final class Pending
{
    private static ?\Throwable $thrown = null;

    public static function keep(\Throwable $thrown): void
    {
        self::$thrown = $thrown;
    }

    public static function take(): ?\Throwable
    {
        $thrown = self::$thrown;
        self::$thrown = null;
        return $thrown;
    }
}
