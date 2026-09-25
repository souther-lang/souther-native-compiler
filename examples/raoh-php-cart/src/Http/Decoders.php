<?php

declare(strict_types=1);

namespace App\Http;

use Model\Com\Example\Cart\Domain\Corporation;
use Model\Com\Example\Cart\Domain\Email;
use Model\Com\Example\Cart\Domain\Individual;
use Model\Com\Example\Cart\Domain\Orderer;
use Model\Com\Example\Cart\Domain\ProductId;
use Model\Com\Example\Cart\Domain\Quantity;
use Model\Com\Example\Cart\Domain\UserId;
use Raoh\CallableDecoder;
use Raoh\Decoder;
use Raoh\Path;
use Raoh\Result;
use Souther\Runtime\Session;

use function Raoh\Boundary\Json\combine;
use function Raoh\Boundary\Json\field;
use function Raoh\Boundary\Json\from_json;
use function Raoh\Boundary\Json\int_;
use function Raoh\Boundary\Json\string_;

/**
 * Request bodies, decoded into the model's values.
 *
 * raoh-php checks the form of each field and normalises it: a UUID, a positive quantity, an email
 * trimmed and lowercased, a corporate number of thirteen digits, which the model has no regular
 * expression to say. What it hands on goes to the model's own `of`, which checks what the type
 * states (an identifier is not empty, an email is at least three characters) and answers the
 * model's value. Either failing is an issue under the field's path, and a 400.
 *
 * A value of the model belongs to the run it is made in, so each decoder is made for a session.
 */
final class Decoders
{
    /** @return Decoder<mixed, UserId> */
    public static function userId(Session $session): Decoder
    {
        return string_()->uuid()->map(strtolower(...))
            ->flatMap(fn (string $id) => UserId::of($session, $id));
    }

    /** `{"userId":"…","productId":"…","quantity":n}` as the arguments of addItemToCart. */
    public static function addItem(Session $session): Decoder
    {
        return from_json(combine(
            field('userId', self::userId($session)),
            field('productId', string_()->uuid()->map(strtolower(...))
                ->flatMap(fn (string $id) => ProductId::of($session, $id))),
            field('quantity', int_()->positive()
                ->flatMap(fn (int $n) => Quantity::of($session, $n))),
        )->map(fn (UserId $userId, ProductId $productId, Quantity $quantity): array =>
            [$userId, $productId, $quantity]));
    }

    /** `{"userId":"…","orderer":{…}}` as a user and an orderer. */
    public static function checkout(Session $session): Decoder
    {
        return from_json(combine(
            field('userId', self::userId($session)),
            field('orderer', self::orderer($session)),
        )->map(fn (UserId $userId, Orderer $orderer): array => [$userId, $orderer]));
    }

    /**
     * An orderer, told apart by `type`, which names the model's case as the model's own encoding
     * of an orderer does: `{"type":"Individual",…}` or `{"type":"Corporation",…}`.
     *
     * @return Decoder<mixed, Orderer>
     */
    public static function orderer(Session $session): Decoder
    {
        $cases = [
            'Individual' => combine(
                field('email', self::email($session)),
                field('name', string_()->trim()->nonBlank()->maxLength(100)),
            )->flatMap(fn (Email $email, string $name) => Individual::of($session, $email, $name)),
            'Corporation' => combine(
                field('email', self::email($session)),
                field('companyName', string_()->trim()->nonBlank()->maxLength(200)),
                field('corporateNumber', string_()->pattern('/^\d{13}$/')),
            )->flatMap(fn (Email $email, string $companyName, string $corporateNumber) =>
                Corporation::of($session, $email, $companyName, $corporateNumber)),
        ];
        $type = field('type', string_()->oneOf(array_keys($cases)));
        return CallableDecoder::of(fn (mixed $in, ?Path $path = null): Result =>
            $type->decode($in, $path)->flatMap(fn (string $case) => $cases[$case]->decode($in, $path)));
    }

    /** @return Decoder<mixed, Email> */
    private static function email(Session $session): Decoder
    {
        return string_()->trim()->toLowerCase()->email()
            ->flatMap(fn (string $email) => Email::of($session, $email));
    }
}
