<?php

declare(strict_types=1);

namespace App;

use App\Database\PdoLoadCart;
use App\Database\PdoLoadProduct;
use App\Database\PdoPriceCart;
use App\Database\PdoSaveItem;
use App\Database\PdoSaveOrder;
use App\Database\Transaction;
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
 * each composed behavior bound to them once, and the routes.
 */
final readonly class CartApplication
{
    private function __construct(private Router $router)
    {
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
            $binding,
            AddItemToCart::bind($loadProduct, $loadCart, $saveItem),
            PlaceOrder::bind($priceCart, $saveOrder),
            IssueQuote::bind($priceCart),
            new Transaction($pdo),
            $pdo,
        );

        return new self((new Router())
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

    public function handle(Request $request): Response
    {
        return $this->router->handle($request);
    }
}
