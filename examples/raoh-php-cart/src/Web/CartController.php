<?php

declare(strict_types=1);

namespace App\Web;

use App\Http\Request;
use App\Http\Response;
use App\Infrastructure\CartQueryRepository;
use App\Infrastructure\Transaction;
use App\Infrastructure\Uuid;
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
use Raoh\Issues;
use Souther\Runtime\Session;

/**
 * Where every boundary meets. A body is decoded with raoh into the model's values, the composed
 * behavior is applied once, and a `match` on the class of what it answered picks the response.
 * Reading and writing the database is done by the injected behaviors the composed ones were bound
 * to, so this stays thin.
 *
 * Two kinds of failure, as in the Java example: an input that does not decode is a 400 with
 * raoh's issues, and a business case the behavior answers is a 422.
 *
 * Each request is one run of the library. Every value of the model is made in that run and is
 * gone when it ends, so what leaves a handler is a response body of plain arrays.
 */
final readonly class CartController
{
    public function __construct(
        private Binding $binding,
        private AddItemToCart $addItemToCart,
        private PlaceOrder $placeOrder,
        private IssueQuote $issueQuote,
        private CartQueryRepository $cartQuery,
        private Transaction $tx,
    ) {
    }

    public function addItem(Request $request): Response
    {
        return $this->binding->run(fn (Session $session): Response =>
            JsonCartDecoders::addItem($session)->decode($request->body)->fold(
                fn (array $decoded): Response => $this->tx->execute(fn (): Response =>
                    match ($this->addItemToCart->apply($session, ...$decoded)::class) {
                        ItemAdded::class => Response::created(),
                        ProductNotFound::class => Response::unprocessable('product_not_found'),
                        SaleEnded::class => Response::unprocessable('sale_ended'),
                        CartFull::class => Response::unprocessable('cart_full'),
                    }),
                fn (Issues $issues): Response => Response::badRequest(self::errorBody($issues))));
    }

    public function checkout(Request $request): Response
    {
        return $this->binding->run(fn (Session $session): Response =>
            JsonCartDecoders::checkout($session)->decode($request->body)->fold(
                function (array $decoded) use ($session): Response {
                    [$userId, $orderer] = $decoded;
                    $orderId = OrderId::of($session, Uuid::v4())->getOrThrow();
                    return $this->tx->execute(function () use ($session, $orderId, $userId, $orderer): Response {
                        $answer = $this->placeOrder->apply($session, $orderId, $userId, $orderer);
                        return match ($answer::class) {
                            OrderPlaced::class => Response::created(OrderViewEncoders::orderView($answer->order())),
                            EmptyCart::class => Response::unprocessable('empty_cart'),
                            SaleEnded::class => Response::unprocessable('sale_ended'),
                            ProductNotFound::class => Response::unprocessable('product_not_found'),
                        };
                    });
                },
                fn (Issues $issues): Response => Response::badRequest(self::errorBody($issues))));
    }

    public function quote(Request $request): Response
    {
        return $this->binding->run(fn (Session $session): Response =>
            JsonCartDecoders::checkout($session)->decode($request->body)->fold(
                function (array $decoded) use ($session): Response {
                    [$userId, $orderer] = $decoded;
                    // A quotation is for a corporation only. issueQuote takes a Corporation, so the
                    // orderer is narrowed here.
                    return match ($orderer::class) {
                        Corporation::class => $this->quoteFor($session, $userId, $orderer),
                        Individual::class => Response::unprocessable('quote_for_corporations_only'),
                    };
                },
                fn (Issues $issues): Response => Response::badRequest(self::errorBody($issues))));
    }

    public function listItems(Request $request): Response
    {
        return $this->binding->run(fn (Session $session): Response =>
            JsonCartDecoders::userId($session)->decode($request->query['userId'] ?? null)->fold(
                fn (UserId $userId): Response => Response::ok(CartViewEncoders::pageView($this->cartQuery->findItemsByPage(
                    $session,
                    $userId,
                    max(0, (int) ($request->query['page'] ?? 0)),
                    max(1, (int) ($request->query['size'] ?? 20))))),
                fn (Issues $issues): Response => Response::badRequest(self::errorBody($issues))));
    }

    private function quoteFor(Session $session, UserId $userId, Corporation $corporation): Response
    {
        $validUntil = (new \DateTimeImmutable('+30 days'))->format('Y-m-d');
        $answer = $this->issueQuote->apply(
            $session, QuoteId::of($session, Uuid::v4())->getOrThrow(), $userId, $corporation, $validUntil);
        return match ($answer::class) {
            Quotation::class => Response::ok(OrderViewEncoders::quotationView($answer)),
            EmptyCart::class => Response::unprocessable('empty_cart'),
            SaleEnded::class => Response::unprocessable('sale_ended'),
            ProductNotFound::class => Response::unprocessable('product_not_found'),
        };
    }

    /** raoh's issues as a body with their paths: each issue, and the messages by path. */
    private static function errorBody(Issues $issues): array
    {
        return ['issues' => $issues->toJsonList(), 'errors' => $issues->flatten()];
    }
}
