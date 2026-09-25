<?php

declare(strict_types=1);

namespace App\Infrastructure;

use Model\Com\Example\Cart\Domain\CartItem;
use Model\Com\Example\Cart\Domain\UserId;
use PDO;
use Souther\Runtime\Session;

/** The read side: a page of a cart's items, each read by the library as a `CartItem`. */
final readonly class CartQueryRepository
{
    public function __construct(private PDO $pdo)
    {
    }

    /** @return Page<CartItem> */
    public function findItemsByPage(Session $session, UserId $userId, int $page, int $size): Page
    {
        $count = $this->pdo->prepare(<<<'SQL'
            SELECT COUNT(*) FROM cart_item ci JOIN cart c ON c.cart_id = ci.cart_id WHERE c.user_id = ?
            SQL);
        $count->execute([$userId->value()]);

        $select = $this->pdo->prepare(<<<'SQL'
            SELECT ci.product_id, ci.quantity
            FROM cart_item ci
            JOIN cart c ON c.cart_id = ci.cart_id
            WHERE c.user_id = ?
            ORDER BY ci.product_id
            LIMIT ? OFFSET ?
            SQL);
        $select->execute([$userId->value(), $size, $page * $size]);

        $items = array_map(
            fn (array $row): CartItem => CartItem::decode($session, json_encode([
                'productId' => $row['product_id'],
                'quantity' => (int) $row['quantity'],
            ], JSON_THROW_ON_ERROR))->getOrThrow(),
            $select->fetchAll(PDO::FETCH_ASSOC));
        return new Page((int) $count->fetchColumn(), $page, $size, $items);
    }
}
