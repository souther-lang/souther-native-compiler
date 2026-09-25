<?php

declare(strict_types=1);

namespace App\Http;

use App\Database\Uuid;
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
use PDO;

/**
 * The HTTP boundary. A body is decoded into the arguments of a behavior, the behavior is called
 * like any PHP function, and a `match` on the class of what it answered picks the response. What an
 * order or a quotation is written as is the model's own encoding of it.
 *
 * A body that does not decode throws `BadRequest`, which the application answers with a 400.
 */
final readonly class CartController
{
    public function __construct(
        private AddItemToCart $addItemToCart,
        private PlaceOrder $placeOrder,
        private IssueQuote $issueQuote,
        private PDO $pdo,
    ) {
    }

    /** `POST /carts/items` */
    public function addItem(Request $request): Response
    {
        [$userId, $productId, $quantity] = Decoders::addItem()
            ->decode($request->body)
            ->orElseThrow(BadRequest::of(...));

        $answer = ($this->addItemToCart)($userId, $productId, $quantity);

        return match ($answer::class) {
            ItemAdded::class => Response::created(),
            ProductNotFound::class => Response::unprocessable('product_not_found'),
            SaleEnded::class => Response::unprocessable('sale_ended'),
            CartFull::class => Response::unprocessable('cart_full'),
        };
    }

    /** `POST /carts/checkout` */
    public function checkout(Request $request): Response
    {
        [$userId, $orderer] = Decoders::checkout()
            ->decode($request->body)
            ->orElseThrow(BadRequest::of(...));

        $answer = ($this->placeOrder)(OrderId::of(Uuid::v4())->getOrThrow(), $userId, $orderer);

        return match ($answer::class) {
            OrderPlaced::class => Response::created($answer->order()->encode()),
            EmptyCart::class => Response::unprocessable('empty_cart'),
            SaleEnded::class => Response::unprocessable('sale_ended'),
            ProductNotFound::class => Response::unprocessable('product_not_found'),
        };
    }

    /** `POST /carts/quote`, for a corporation only. */
    public function quote(Request $request): Response
    {
        [$userId, $orderer] = Decoders::checkout()
            ->decode($request->body)
            ->orElseThrow(BadRequest::of(...));

        // issueQuote takes a Corporation, so the orderer is narrowed here.
        if (!$orderer instanceof Corporation) {
            return Response::unprocessable('quote_for_corporations_only');
        }
        $validUntil = (new \DateTimeImmutable('+30 days'))->format('Y-m-d');

        $answer = ($this->issueQuote)(QuoteId::of(Uuid::v4())->getOrThrow(), $userId, $orderer, $validUntil);

        return match ($answer::class) {
            Quotation::class => Response::ok($answer->encode()),
            EmptyCart::class => Response::unprocessable('empty_cart'),
            SaleEnded::class => Response::unprocessable('sale_ended'),
            ProductNotFound::class => Response::unprocessable('product_not_found'),
        };
    }

    /**
     * `GET /carts/items?userId=…&page=…&size=…`
     *
     * A listing for a screen, which the model has no behavior for and needs none: the rows are read
     * and written out as they are. Only the user is the model's, checked as every other input is.
     */
    public function listItems(Request $request): Response
    {
        $userId = Decoders::userId()
            ->decode($request->query['userId'] ?? null)
            ->orElseThrow(BadRequest::of(...));
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
    }
}
