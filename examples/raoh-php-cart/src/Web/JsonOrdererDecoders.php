<?php

declare(strict_types=1);

namespace App\Web;

use Model\Com\Example\Cart\Domain\Corporation;
use Model\Com\Example\Cart\Domain\Email;
use Model\Com\Example\Cart\Domain\Individual;
use Model\Com\Example\Cart\Domain\Orderer;
use Raoh\CallableDecoder;
use Raoh\Decoder;
use Raoh\Path;
use Raoh\Result;
use Souther\Runtime\Session;

use function Raoh\Boundary\Json\combine;
use function Raoh\Boundary\Json\field;
use function Raoh\Boundary\Json\string_;

/**
 * The orderer, told apart by `type`. raoh normalises and checks each field (the email trimmed,
 * lowercased and checked for its form; the corporate number as thirteen digits, which the model
 * has no regular expression to say), and the model builds the `Individual` or the `Corporation`
 * from what it checked, checking `Email`'s invariant again.
 */
final class JsonOrdererDecoders
{
    /** `{"type":"individual","email":"...","name":"..."}` */
    public static function individual(Session $session): Decoder
    {
        return combine(
            field('email', string_()->trim()->toLowerCase()->email()),
            field('name', string_()->trim()->nonBlank()->maxLength(100)),
        )->flatMap(fn (string $email, string $name) => Email::of($session, $email)
            ->flatMap(fn (Email $valid) => Individual::of($session, $valid, $name)));
    }

    /** `{"type":"corporation","email":"...","companyName":"...","corporateNumber":"1234567890123"}` */
    public static function corporation(Session $session): Decoder
    {
        return combine(
            field('email', string_()->trim()->toLowerCase()->email()),
            field('companyName', string_()->trim()->nonBlank()->maxLength(200)),
            field('corporateNumber', string_()->pattern('/^\d{13}$/')),
        )->flatMap(fn (string $email, string $companyName, string $corporateNumber) =>
            Email::of($session, $email)->flatMap(fn (Email $valid) =>
                Corporation::of($session, $valid, $companyName, $corporateNumber)));
    }

    /** @return Decoder<mixed, Orderer> */
    public static function orderer(Session $session): Decoder
    {
        $type = field('type', string_()->oneOf(['individual', 'corporation']));
        $variants = [
            'individual' => self::individual($session),
            'corporation' => self::corporation($session),
        ];
        return CallableDecoder::of(fn (mixed $in, ?Path $path = null): Result =>
            $type->decode($in, $path)->flatMap(fn (string $which) => $variants[$which]->decode($in, $path)));
    }
}
