<?php

declare(strict_types=1);

namespace App\Database;

use Model\Com\Example\Cart\Domain\PriceCart;
use Model\Com\Example\Cart\Domain\PricedCart;
use Model\Com\Example\Cart\Domain\ProductNotFound;
use Model\Com\Example\Cart\Domain\SaleEnded;
use Model\Com\Example\Cart\Domain\UserId;
use PDO;

/**
 * `priceCart` over PDO. It reads every line of the cart with its product in one query, sees that
 * each product is there and on sale, and answers the lines with their prices as a `PricedCart`. A
 * product that is gone or no longer on sale ends it with the model's own case, the first such line
 * in the order of the product ids deciding which.
 *
 * Deciding that for each line stays here, in the implementation, for the same reason it does in the
 * Java example: the model has no traverse, and a fold cannot call another injected behavior. The
 * products are joined to the lines rather than asked for one by one, so a cart is one query however
 * many lines it has.
 */
final class PdoPriceCart extends PriceCart
{
    public function __construct(private readonly PDO $pdo)
    {
    }

    public function apply(UserId $userId): PricedCart|SaleEnded|ProductNotFound
    {
        $select = $this->pdo->prepare(<<<'SQL'
            SELECT ci.product_id, ci.quantity, p.on_sale, p.price
            FROM cart_item ci
            JOIN cart c ON c.cart_id = ci.cart_id
            LEFT JOIN product p ON p.product_id = ci.product_id
            WHERE c.user_id = ?
            ORDER BY ci.product_id
            SQL);
        $select->execute([$userId->value()]);

        $lines = [];
        foreach ($select->fetchAll(PDO::FETCH_ASSOC) as $row) {
            if ($row['on_sale'] === null) {
                return ProductNotFound::of()->getOrThrow();
            }
            if (!$row['on_sale']) {
                return SaleEnded::of()->getOrThrow();
            }
            $lines[] = [
                'productId' => $row['product_id'],
                'quantity' => (int) $row['quantity'],
                'unitPrice' => (int) $row['price'],
            ];
        }
        return PricedCart::decoder()->decode(['lines' => $lines])->getOrThrow();
    }
}
