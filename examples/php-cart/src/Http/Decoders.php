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
 * boundary owns how a client writes a value: an id is a UUID in lower case, an email is trimmed,
 * lowercased and shaped like one, a name is trimmed. Each of those is a raoh-php decoder, which
 * writes the value in its form and refuses what cannot be written so, as one step.
 *
 * Where the boundary owns the value the model reads, an id, the two are piped: the model reads what
 * the boundary answered, and nothing where it refused, since its issue would be at the same path.
 * Where the model reads a value whole and the boundary owns some of its members, an orderer, the
 * two are put together by `Members`, so that a member the boundary refuses does not keep the model
 * from reading the rest.
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
     * An orderer in the model's own encoding of one, read whole by the model once the members the
     * boundary owns are decoded. Whether each is there at all, and what it has to be, is the
     * model's to say, as it is of every other member.
     *
     * @return Decoder<mixed, Orderer>
     */
    public static function orderer(): Decoder
    {
        return Members::of([
            'email' => string_()->trim()->toLowerCase()->email(),
            'name' => string_()->trim(),
            'companyName' => string_()->trim(),
        ], OrdererCodec::decoder());
    }

    /**
     * A UUID as this API writes one, and as the database keeps it: in lower case, with its hyphens.
     * Another notation of one (braced, a URN, without hyphens) is not how a client writes an id here.
     *
     * @return Decoder<mixed, string>
     */
    private static function uuid(): Decoder
    {
        return string_()->uuid()->map(strtolower(...));
    }
}
