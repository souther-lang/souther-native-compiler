<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A Souther `Decimal` as PHP holds one: its integer and its scale, the amount being the integer over
 * ten to the scale.
 *
 * The two numbers the language says a `Decimal` is, and nothing else. PHP has no decimal type that
 * keeps a scale below nought, and a string of the value would be one text among several for it,
 * so a binding hands over and is handed these two, and what PHP does with them — a `BcMath\Number`,
 * a money library, text — is the application's. The scale is kept as it was: `1.50` is `150` at
 * scale 2, and `1.5` is `15` at scale 1, two values of one amount, as they are in Souther.
 *
 * Refused where it could not be a `Decimal`: an integer written other than as an optional `-` and
 * ASCII digits, or a scale outside the 32-bit range a `Decimal`'s scale is in.
 */
final class Decimal
{
    /** The integer, as an optional `-` and its digits, with no leading zero. */
    public readonly string $unscaled;

    public function __construct(string $unscaled, public readonly int $scale)
    {
        if (preg_match('/\A(-?)0*([0-9]+)\z/', $unscaled, $written) !== 1) {
            throw new \InvalidArgumentException(
                "a Decimal's integer is an optional '-' and ASCII digits, and not '{$unscaled}'");
        }
        if ($scale < -2147483648 || $scale > 2147483647) {
            throw new \InvalidArgumentException(
                "a Decimal's scale is a 32-bit number, and not {$scale}");
        }
        $digits = $written[2];
        $this->unscaled = ($digits !== '0' ? $written[1] : '') . $digits;
    }
}
