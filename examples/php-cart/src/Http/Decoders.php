<?php

declare(strict_types=1);

namespace App\Http;

use Model\Com\Example\Cart\Domain\Orderer;
use Model\Com\Example\Cart\Domain\OrdererCodec;
use Model\Com\Example\Cart\Domain\ProductId;
use Model\Com\Example\Cart\Domain\Quantity;
use Model\Com\Example\Cart\Domain\UserId;
use Raoh\Decoder;

use function Raoh\Boundary\Json\combine;
use function Raoh\Boundary\Json\field;
use function Raoh\Boundary\Json\from_json;
use function Raoh\Boundary\Json\string_;

/**
 * Request bodies, decoded into the arguments of a behavior.
 *
 * Two parties read a request, and each owns a different part of what it means. The model owns what
 * a value is, and its decoders, which the binding generates, read it: which case an orderer is, the
 * fields each case has, and every rule a type states, a positive quantity, a name that is not blank
 * and a corporate number of thirteen digits among them. Nothing here says any of that again. The
 * boundary owns how a value is written from outside: the canonical form of what a client sends (an
 * id in lower case, an email and a name without the spaces around them, an email in lower case),
 * and the forms the model leaves to it (an id is a UUID, an email is shaped like one). raoh-php does
 * the boundary's part.
 *
 * An id's form is about the very value the model reads, so it is piped into the model's decoder.
 * An orderer's members are canonicalised apart from the model's reading of the whole, by
 * `Canonical`, so that a member the boundary refuses does not keep the model from reading the rest.
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
     * An orderer in the model's own encoding of one, read whole by the model once the members
     * whose canonical form is the boundary's are in it. Whether each is there at all, and what it
     * has to be, is the model's to say, as it is of every other member.
     *
     * @return Decoder<mixed, Orderer>
     */
    public static function orderer(): Decoder
    {
        return Canonical::of([
            'email' => string_()->trim()->toLowerCase()->email(),
            'name' => string_()->trim(),
            'companyName' => string_()->trim(),
        ], OrdererCodec::decoder());
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
