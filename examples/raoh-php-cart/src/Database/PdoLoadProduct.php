<?php

declare(strict_types=1);

namespace App\Database;

use Model\Com\Example\Cart\Domain\LoadProduct;
use Model\Com\Example\Cart\Domain\Product;
use Model\Com\Example\Cart\Domain\ProductId;
use Model\Com\Example\Cart\Domain\ProductNotFound;
use PDO;
use Souther\Runtime\Session;

/**
 * `loadProduct` over PDO. The row is put back into the external form of `Product` and read by the
 * library, which checks `ProductId`'s invariant again: this is where the database meets the model.
 * No row is the model's own `ProductNotFound`.
 */
final class PdoLoadProduct extends LoadProduct
{
    public function __construct(private readonly PDO $pdo)
    {
    }

    public function apply(Session $session, ProductId $productId): Product|ProductNotFound
    {
        $select = $this->pdo->prepare('SELECT product_id, on_sale, price FROM product WHERE product_id = ?');
        $select->execute([$productId->value()]);
        $row = $select->fetch(PDO::FETCH_ASSOC);
        if ($row === false) {
            return ProductNotFound::of($session)->getOrThrow();
        }
        return Product::decode($session, json_encode([
            'id' => $row['product_id'],
            'onSale' => (bool) $row['on_sale'],
            'price' => (int) $row['price'],
        ], JSON_THROW_ON_ERROR))->getOrThrow();
    }
}
