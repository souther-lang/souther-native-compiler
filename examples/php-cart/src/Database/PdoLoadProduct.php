<?php

declare(strict_types=1);

namespace App\Database;

use Model\Com\Example\Cart\Domain\LoadProduct;
use Model\Com\Example\Cart\Domain\Product;
use Model\Com\Example\Cart\Domain\ProductId;
use Model\Com\Example\Cart\Domain\ProductNotFound;
use PDO;

/**
 * `loadProduct` over PDO. The row is handed to `Product`'s decoder under the type's field names, and
 * the model checks `ProductId`'s invariant again: this is where the database meets the model.
 * No row is the model's own `ProductNotFound`.
 */
final class PdoLoadProduct extends LoadProduct
{
    public function __construct(private readonly PDO $pdo)
    {
    }

    public function apply(ProductId $productId): Product|ProductNotFound
    {
        $select = $this->pdo->prepare('SELECT product_id, on_sale, price FROM product WHERE product_id = ?');
        $select->execute([$productId->value()]);
        $row = $select->fetch(PDO::FETCH_ASSOC);
        if ($row === false) {
            return ProductNotFound::of()->getOrThrow();
        }
        return Product::decoder()->decode([
            'id' => $row['product_id'],
            'onSale' => (bool) $row['on_sale'],
            'price' => (int) $row['price'],
        ])->getOrThrow();
    }
}
