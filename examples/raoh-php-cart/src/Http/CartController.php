<?php

declare(strict_types=1);

namespace App\Http;

use App\Database\Transaction;
use App\Database\Uuid;
use Model\Binding;
use Model\Com\Example\Cart\Domain\AddItemToCart;
use Model\Com\Example\Cart\Domain\CartFull;
use Model\Com\Example\Cart\Domain\Corporation;
use Model\Com\Example\Cart\Domain\EmptyCart;
use Model\Com\Example\Cart\Domain\Individual;
use Model\Com\Example\Cart\Domain\IssueQuote;
use Model\Com\Example\Cart\Domain\ItemAdded;
use Model\Com\Example\Cart\Domain\OrderId;
use Model\Com\Example\Cart\Domain\OrderPlaced;
use Model\Com\Example\Cart\Domain\PlaceOrder;
use Model\Com\Example\Cart\Domain\ProductNotFound;
use Model\Com\Example\Cart\Domain\Quotation;
use Model\Com\Example\Cart\Domain\QuoteId;
use Model\Com\Example\Cart\Domain\SaleEnded;
use Model\Com\Example\Cart\Domain\UserId;
use PDO;
use Souther\Runtime\Session;

/**
 * The HTTP boundary. A body is decoded into the model's values, the composed behavior is applied
 * once, and a `match` on the class of what it answered picks the response. What an order or a
 * quotation is written as is the model's own encoding of it, so there is no view to keep in step
 * with the model.
 *
 * Each request is one run of the library: every value of the model is made in it and gone when it
 * ends, so what leaves a handler is JSON text.
 */
final readonly class CartController
{
    public function __construct(
        private Binding $binding,
        private AddItemToCart $addItemToCart,
        private PlaceOrder $placeOrder,
        private IssueQuote $issueQuote,
        private Transaction $tx,
        private PDO $pdo,
    ) {
    }

    /** `POST /carts/items` */
    public function addItem(Request $request): Response
    {
        return $this->binding->run(fn (Session $session): Response =>
            Decoders::addItem($session)->decode($request->body)->fold(
                fn (array $arguments): Response => $this->tx->execute(fn (): Response =>
                    match ($this->addItemToCart->apply($session, ...$arguments)::class) {
                        ItemAdded::class => Response::created(),
                        ProductNotFound::class => Response::unprocessable('product_not_found'),
                        SaleEnded::class => Response::unprocessable('sale_ended'),
                        CartFull::class => Response::unprocessable('cart_full'),
                    }),
                Response::badRequest(...)));
    }

    /** `POST /carts/checkout` */
    public function checkout(Request $request): Response
    {
        return $this->binding->run(fn (Session $session): Response =>
            Decoders::checkout($session)->decode($request->body)->fold(
                fn (array $decoded): Response => $this->tx->execute(function () use ($session, $decoded): Response {
                    [$userId, $orderer] = $decoded;
                    $answer = $this->placeOrder->apply(
                        $session, OrderId::of($session, Uuid::v4())->getOrThrow(), $userId, $orderer);
                    return match ($answer::class) {
                        OrderPlaced::class => Response::created($answer->order()->encode()),
                        EmptyCart::class => Response::unprocessable('empty_cart'),
                        SaleEnded::class => Response::unprocessable('sale_ended'),
                        ProductNotFound::class => Response::unprocessable('product_not_found'),
                    };
                }),
                Response::badRequest(...)));
    }

    /** `POST /carts/quote`, for a corporation only: issueQuote takes a Corporation, so the orderer is narrowed here. */
    public function quote(Request $request): Response
    {
        return $this->binding->run(fn (Session $session): Response =>
            Decoders::checkout($session)->decode($request->body)->fold(
                function (array $decoded) use ($session): Response {
                    [$userId, $orderer] = $decoded;
                    return match ($orderer::class) {
                        Corporation::class => $this->quoteFor($session, $userId, $orderer),
                        Individual::class => Response::unprocessable('quote_for_corporations_only'),
                    };
                },
                Response::badRequest(...)));
    }

    /**
     * `GET /carts/items?userId=…&page=…&size=…`
     *
     * A listing for a screen, which the model has no behavior for and needs none: the rows are read
     * and written out as they are. Only the user is the model's, checked as every other input is.
     */
    public function listItems(Request $request): Response
    {
        return $this->binding->run(fn (Session $session): Response =>
            Decoders::userId($session)->decode($request->query['userId'] ?? null)->fold(
                function (UserId $userId) use ($request): Response {
                    $page = max(0, (int) ($request->query['page'] ?? 0));
                    $size = max(1, (int) ($request->query['size'] ?? 20));
                    $count = $this->pdo->prepare(<<<'SQL'
                        SELECT COUNT(*) FROM cart_item ci JOIN cart c ON c.cart_id = ci.cart_id WHERE c.user_id = ?
                        SQL);
                    $count->execute([$userId->value()]);
                    $items = $this->pdo->prepare(<<<'SQL'
                        SELECT ci.product_id AS productId, ci.quantity
                        FROM cart_item ci JOIN cart c ON c.cart_id = ci.cart_id
                        WHERE c.user_id = ?
                        ORDER BY ci.product_id
                        LIMIT ? OFFSET ?
                        SQL);
                    $items->execute([$userId->value(), $size, $page * $size]);
                    return Response::ok(Response::json([
                        'total' => (int) $count->fetchColumn(),
                        'page' => $page,
                        'size' => $size,
                        'items' => $items->fetchAll(PDO::FETCH_ASSOC),
                    ]));
                },
                Response::badRequest(...)));
    }

    private function quoteFor(Session $session, UserId $userId, Corporation $corporation): Response
    {
        $validUntil = (new \DateTimeImmutable('+30 days'))->format('Y-m-d');
        $answer = $this->issueQuote->apply(
            $session, QuoteId::of($session, Uuid::v4())->getOrThrow(), $userId, $corporation, $validUntil);
        return match ($answer::class) {
            Quotation::class => Response::ok($answer->encode()),
            EmptyCart::class => Response::unprocessable('empty_cart'),
            SaleEnded::class => Response::unprocessable('sale_ended'),
            ProductNotFound::class => Response::unprocessable('product_not_found'),
        };
    }
}
