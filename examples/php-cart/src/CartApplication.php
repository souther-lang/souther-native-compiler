<?php

declare(strict_types=1);

namespace App;

use App\Database\PdoLoadCart;
use App\Database\PdoLoadProduct;
use App\Database\PdoPriceCart;
use App\Database\PdoSaveItem;
use App\Database\PdoSaveOrder;
use App\Database\Transaction;
use App\Http\BadRequest;
use App\Http\CartController;
use App\Http\Request;
use App\Http\Response;
use App\Http\Router;
use Model\Binding;
use Model\Com\Example\Cart\Domain\AddItemToCart;
use Model\Com\Example\Cart\Domain\IssueQuote;
use Model\Com\Example\Cart\Domain\PlaceOrder;
use PDO;

/**
 * The wiring, which the Java example leaves to Spring: the injected behaviors implemented over PDO,
 * each composed behavior bound to them once, and the routes. Each request is handled in one run of
 * the library and one transaction, which is where the Java example's `TransactionTemplate` stands.
 */
final readonly class CartApplication
{
    private function __construct(
        private Binding $binding,
        private Transaction $tx,
        private Router $router,
    ) {
    }

    public static function create(Binding $binding, PDO $pdo): self
    {
        $pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
        foreach (['schema.sql', 'data.sql'] as $script) {
            $pdo->exec((string) file_get_contents(__DIR__ . '/../resources/' . $script));
        }

        $loadProduct = new PdoLoadProduct($pdo);
        $loadCart = new PdoLoadCart($pdo);
        $saveItem = new PdoSaveItem($pdo);
        $priceCart = new PdoPriceCart($pdo);
        $saveOrder = new PdoSaveOrder($pdo);

        $controller = new CartController(
            AddItemToCart::bind($loadProduct, $loadCart, $saveItem),
            PlaceOrder::bind($priceCart, $saveOrder),
            IssueQuote::bind($priceCart),
            $pdo,
        );

        return new self($binding, new Transaction($pdo), (new Router())
            ->post('/carts/items', $controller->addItem(...))
            ->post('/carts/checkout', $controller->checkout(...))
            ->post('/carts/quote', $controller->quote(...))
            ->get('/carts/items', $controller->listItems(...)));
    }

    /** The shared library `bin/build` wrote, whichever name the platform gives it. */
    public static function library(): string
    {
        $found = glob(__DIR__ . '/../build/native/libsouther.*') ?: [];
        if ($found === []) {
            throw new \RuntimeException('no library under build/native: run bin/build first');
        }
        return $found[0];
    }

    /**
     * The response to `$request`. Every value of the model made while handling it belongs to this
     * run and is gone when it ends, so what comes back is JSON text.
     */
    public function handle(Request $request): Response
    {
        try {
            return $this->binding->run(fn (): Response =>
                $this->tx->execute(fn (): Response => $this->router->handle($request)));
        } catch (BadRequest $bad) {
            return Response::badRequest($bad->issues);
        }
    }
}
