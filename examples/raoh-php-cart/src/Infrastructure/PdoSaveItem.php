<?php

declare(strict_types=1);

namespace App\Infrastructure;

use Model\Com\Example\Cart\Domain\ItemAdded;
use Model\Com\Example\Cart\Domain\PendingItem;
use Model\Com\Example\Cart\Domain\SaveItem;
use PDO;
use Souther\Runtime\Session;

/**
 * `saveItem` over PDO. It takes the cart and the item out of the `PendingItem` the model built,
 * adds the quantity to the row already there or inserts one, and answers what it wrote.
 */
final class PdoSaveItem extends SaveItem
{
    public function __construct(private readonly PDO $pdo)
    {
    }

    public function apply(Session $session, PendingItem $pending): ItemAdded
    {
        $cartId = $pending->cart()->id()->value();
        $item = $pending->item();
        $productId = $item->productId()->value();
        $quantity = $item->quantity()->value();

        $update = $this->pdo->prepare(
            'UPDATE cart_item SET quantity = quantity + ? WHERE cart_id = ? AND product_id = ?');
        $update->execute([$quantity, $cartId, $productId]);
        if ($update->rowCount() === 0) {
            $insert = $this->pdo->prepare(
                'INSERT INTO cart_item (cart_item_id, cart_id, product_id, quantity) VALUES (?, ?, ?, ?)');
            $insert->execute([Uuid::v4(), $cartId, $productId, $quantity]);
        }

        return ItemAdded::of($session, $item->productId(), $item->quantity())->getOrThrow();
    }
}
