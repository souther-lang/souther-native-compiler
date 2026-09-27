<?php

declare(strict_types=1);

namespace App\Http;

use Model\Com\Example\Cart\Domain\Orderer;
use Model\Com\Example\Cart\Domain\OrdererCodec;
use Model\Com\Example\Cart\Domain\ProductId;
use Model\Com\Example\Cart\Domain\Quantity;
use Model\Com\Example\Cart\Domain\UserId;
use Raoh\CallableDecoder;
use Raoh\Decoder;
use Raoh\Result;

use function Raoh\Boundary\Json\combine;
use function Raoh\Boundary\Json\field;
use function Raoh\Boundary\Json\from_json;
use function Raoh\Boundary\Json\optional_field;
use function Raoh\Boundary\Json\string_;

/**
 * Request bodies, decoded into the arguments of a behavior.
 *
 * What a value of the model is, the model's decoders read, which the binding generates: which case
 * an orderer is, the fields each case has, and every rule a type states, a positive quantity and a
 * corporate number of thirteen digits among them. Nothing here says any of it again. What is left
 * to the boundary is what the model leaves to it: an id is a UUID, written in lower case, and an
 * email is trimmed, lowercased and shaped like one. raoh-php does that part, and pipes what it
 * hands on into the model's decoder. Either failing is an issue under the field's path.
 */
final class Decoders
{
    /** @return Decoder<mixed, UserId> */
    public static function userId(): Decoder
    {
        return self::uuid()->pipe(UserId::decoder());
    }

    /** `{"userId":"…","productId":"…","quantity":n}` as the arguments of addItemToCart. */
    public static function addItem(): Decoder
    {
        return from_json(combine(
            field('userId', self::userId()),
            field('productId', self::uuid()->pipe(ProductId::decoder())),
            field('quantity', Quantity::decoder()),
        )->map(fn (UserId $userId, ProductId $productId, Quantity $quantity): array =>
            [$userId, $productId, $quantity]));
    }

    /** `{"userId":"…","orderer":{…}}` as a user and an orderer. */
    public static function checkout(): Decoder
    {
        return from_json(combine(
            field('userId', self::userId()),
            field('orderer', self::orderer()),
        )->map(fn (UserId $userId, Orderer $orderer): array => [$userId, $orderer]));
    }

    /**
     * An orderer in the model's own encoding of one, read whole by the model once its email is
     * normalised. Whether the email is there at all is the model's to say, as every other field is.
     *
     * @return Decoder<mixed, Orderer>
     */
    public static function orderer(): Decoder
    {
        return combine(
            optional_field('email', string_()->trim()->toLowerCase()->email()),
            CallableDecoder::of(fn (mixed $given): Result => Result::ok($given)),
        )->map(fn (?string $email, mixed $orderer): mixed =>
            $email === null ? $orderer : ['email' => $email] + $orderer)
            ->pipe(OrdererCodec::decoder());
    }

    /**
     * A UUID's text, as the database keeps one: in lower case, with its hyphens.
     *
     * @return Decoder<mixed, string>
     */
    private static function uuid(): Decoder
    {
        return string_()->uuid()->map(strtolower(...));
    }
}
