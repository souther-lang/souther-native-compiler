<?php

declare(strict_types=1);

namespace App\Web;

use Model\Com\Example\Cart\Domain\Orderer;
use Model\Com\Example\Cart\Domain\ProductId;
use Model\Com\Example\Cart\Domain\Quantity;
use Model\Com\Example\Cart\Domain\UserId;
use Raoh\Decoder;
use Souther\Runtime\Session;

use function Raoh\Boundary\Json\combine;
use function Raoh\Boundary\Json\field;
use function Raoh\Boundary\Json\from_json;
use function Raoh\Boundary\Json\int_;
use function Raoh\Boundary\Json\string_;

/**
 * The request bodies, decoded. Each field is checked by raoh first (a UUID, a positive quantity),
 * and what it normalised is handed to the model's own `of`, which checks the type's invariant
 * again and answers the model's value. A body that does not decode is a 400; what the model
 * answers once it has the values is the behavior's, and a business case among it is a 422.
 *
 * A value of the model is made in a run and belongs to it, so each decoder is made for the
 * session it builds values in.
 *
 * The composed behaviors take several arguments, so these answer them as a list, which the
 * controller spreads into `apply`.
 */
final class JsonCartDecoders
{
    /** @return Decoder<mixed, UserId> */
    public static function userId(Session $session): Decoder
    {
        return string_()->uuid()->map(strtolower(...))
            ->flatMap(fn (string $id) => UserId::of($session, $id));
    }

    /** `{"userId":"...","productId":"...","quantity":n}` as `[UserId, ProductId, Quantity]`. */
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

    /** `{"userId":"...","orderer":{...}}` as `[UserId, Orderer]`. */
    public static function checkout(Session $session): Decoder
    {
        return from_json(combine(
            field('userId', self::userId($session)),
            field('orderer', JsonOrdererDecoders::orderer($session)),
        )->map(fn (UserId $userId, Orderer $orderer): array => [$userId, $orderer]));
    }
}
