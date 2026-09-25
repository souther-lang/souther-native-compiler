<?php

declare(strict_types=1);

namespace App\Http;

use Model\Com\Example\Cart\Domain\Orderer;
use Model\Com\Example\Cart\Domain\OrdererCodec;
use Model\Com\Example\Cart\Domain\ProductId;
use Model\Com\Example\Cart\Domain\Quantity;
use Model\Com\Example\Cart\Domain\UserId;
use Raoh\Decoder;
use Souther\Runtime\Session;

use function Raoh\Boundary\Json\combine;
use function Raoh\Boundary\Json\field;
use function Raoh\Boundary\Json\from_json;
use function Raoh\Boundary\Json\int_;
use function Raoh\Boundary\Json\optional_field;
use function Raoh\Boundary\Json\string_;

/**
 * Request bodies, decoded into the model's values in two steps, as the Java example does.
 *
 * raoh-php checks the form of each field and normalises it: a UUID in lower case, a positive
 * quantity, an email trimmed, lowercased and shaped like one, a corporate number of thirteen
 * digits, which the model has no regular expression to say. What it hands on is read by the model's
 * own decoder, which the binding generates: which case an orderer is, the fields each case has, and
 * what each type states. Either failing is an issue under the field's path, and a 400.
 *
 * A value of the model belongs to the run it is made in, so each decoder is made for a session.
 */
final class Decoders
{
    /** @return Decoder<mixed, UserId> */
    public static function userId(Session $session): Decoder
    {
        return string_()->uuid()->map(strtolower(...))->pipe(UserId::decoder($session));
    }

    /** `{"userId":"…","productId":"…","quantity":n}` as the arguments of addItemToCart. */
    public static function addItem(Session $session): Decoder
    {
        return from_json(combine(
            field('userId', self::userId($session)),
            field('productId', string_()->uuid()->map(strtolower(...))->pipe(ProductId::decoder($session))),
            field('quantity', int_()->positive()->pipe(Quantity::decoder($session))),
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
     * An orderer, `{"type":"Individual","email":"…","name":"…"}` or
     * `{"type":"Corporation","email":"…","companyName":"…","corporateNumber":"…"}`: the model's own
     * encoding of one. raoh-php checks the fields that are there, and the model reads the whole.
     *
     * @return Decoder<mixed, Orderer>
     */
    public static function orderer(Session $session): Decoder
    {
        return combine(
            field('type', string_()),
            field('email', string_()->trim()->toLowerCase()->email()),
            optional_field('name', string_()->trim()->nonBlank()->maxLength(100)),
            optional_field('companyName', string_()->trim()->nonBlank()->maxLength(200)),
            optional_field('corporateNumber', string_()->pattern('/^\d{13}$/')),
        )->map(fn (string $type, string $email, ?string $name, ?string $companyName, ?string $corporateNumber): array =>
            array_filter(compact('type', 'email', 'name', 'companyName', 'corporateNumber'),
                fn (?string $given): bool => $given !== null))
            ->pipe(OrdererCodec::decoder($session));
    }
}
