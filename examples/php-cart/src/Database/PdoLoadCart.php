<?php

declare(strict_types=1);

namespace App\Database;

use Model\Com\Example\Cart\Domain\Cart;
use Model\Com\Example\Cart\Domain\LoadCart;
use Model\Com\Example\Cart\Domain\UserId;
use PDO;

/**
 * `loadCart` over PDO. It makes sure the user has a cart row, then reads the cart with the total
 * quantity of what is in it in one aggregate query: the items themselves are not loaded, and the
 * total is what the capacity rule needs.
 */
final class PdoLoadCart extends LoadCart
{
    public function __construct(private readonly PDO $pdo)
    {
    }

    public function apply(UserId $userId): Cart
    {
        $this->ensureCartExists($userId->value());
        $select = $this->pdo->prepare(<<<'SQL'
            SELECT c.cart_id, COALESCE(SUM(ci.quantity), 0) AS total
            FROM cart c
            LEFT JOIN cart_item ci ON ci.cart_id = c.cart_id
            WHERE c.user_id = ?
            GROUP BY c.cart_id
            SQL);
        $select->execute([$userId->value()]);
        $row = $select->fetch(PDO::FETCH_ASSOC);

        return Cart::decoder()->decode([
            'id' => $row['cart_id'],
            'currentQuantity' => (int) $row['total'],
        ])->getOrThrow();
    }

    private function ensureCartExists(string $userId): void
    {
        $insert = $this->pdo->prepare('INSERT OR IGNORE INTO cart (cart_id, user_id) VALUES (?, ?)');
        $insert->execute([Uuid::v4(), $userId]);
    }
}
