<?php

declare(strict_types=1);

namespace App\Database;

use Model\Com\Example\Cart\Domain\PriceCart;
use Model\Com\Example\Cart\Domain\PricedCart;
use Model\Com\Example\Cart\Domain\ProductNotFound;
use Model\Com\Example\Cart\Domain\SaleEnded;
use Model\Com\Example\Cart\Domain\UserId;
use PDO;
use Souther\Runtime\Session;

/**
 * `priceCart` over PDO. It reads every line of the cart, looks each product up to see that it is
 * there and on sale, and answers the lines with their prices as a `PricedCart`. A product that is
 * gone or no longer on sale ends it with the model's own case.
 *
 * The loop that asks for each line's product stays here, in the implementation, for the same reason
 * it does in the Java example: the model has no traverse, and a fold cannot call another injected
 * behavior.
 */
final class PdoPriceCart extends PriceCart
{
    public function __construct(private readonly PDO $pdo)
    {
    }

    public function apply(Session $session, UserId $userId): PricedCart|SaleEnded|ProductNotFound
    {
        $select = $this->pdo->prepare(<<<'SQL'
            SELECT ci.product_id, ci.quantity
            FROM cart_item ci
            JOIN cart c ON c.cart_id = ci.cart_id
            WHERE c.user_id = ?
            ORDER BY ci.product_id
            SQL);
        $select->execute([$userId->value()]);
        $product = $this->pdo->prepare('SELECT on_sale, price FROM product WHERE product_id = ?');

        $lines = [];
        foreach ($select->fetchAll(PDO::FETCH_ASSOC) as $row) {
            $product->execute([$row['product_id']]);
            $found = $product->fetch(PDO::FETCH_ASSOC);
            if ($found === false) {
                return ProductNotFound::of($session)->getOrThrow();
            }
            if (!$found['on_sale']) {
                return SaleEnded::of($session)->getOrThrow();
            }
            $lines[] = [
                'productId' => $row['product_id'],
                'quantity' => (int) $row['quantity'],
                'unitPrice' => (int) $found['price'],
            ];
        }
        return PricedCart::decoder($session)->decode(['lines' => $lines])->getOrThrow();
    }
}
