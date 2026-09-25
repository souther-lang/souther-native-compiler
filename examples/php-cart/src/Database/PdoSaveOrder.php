<?php

declare(strict_types=1);

namespace App\Database;

use Model\Com\Example\Cart\Domain\Corporation;
use Model\Com\Example\Cart\Domain\Individual;
use Model\Com\Example\Cart\Domain\Order;
use Model\Com\Example\Cart\Domain\OrderPlaced;
use Model\Com\Example\Cart\Domain\SaveOrder;
use PDO;

/**
 * `saveOrder` over PDO: one row for the order and one for each of its lines. The orderer is laid
 * out flat by which case it is, since an individual and a corporation fill different columns.
 */
final class PdoSaveOrder extends SaveOrder
{
    public function __construct(private readonly PDO $pdo)
    {
    }

    public function apply(Order $order): OrderPlaced
    {
        $orderId = $order->id()->value();
        $orderer = $order->orderer();
        [$type, $email, $name, $companyName, $corporateNumber] = match ($orderer::class) {
            Individual::class => ['individual', $orderer->email()->value(), $orderer->name(), null, null],
            Corporation::class => ['corporation', $orderer->email()->value(), null,
                $orderer->companyName(), $orderer->corporateNumber()],
        };
        $charge = $order->charge();

        $this->pdo->prepare(<<<'SQL'
            INSERT INTO orders (order_id, user_id, subtotal, discount, total, orderer_type, orderer_email,
                                orderer_name, orderer_company_name, orderer_corporate_number)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            SQL)->execute([
                $orderId, $order->userId()->value(),
                $charge->subtotal()->value(), $charge->discount()->value(), $charge->total()->value(),
                $type, $email, $name, $companyName, $corporateNumber,
            ]);

        $line = $this->pdo->prepare(<<<'SQL'
            INSERT INTO order_line (order_line_id, order_id, product_id, quantity, unit_price)
            VALUES (?, ?, ?, ?, ?)
            SQL);
        foreach ($order->lines() as $each) {
            $line->execute([
                Uuid::v4(), $orderId,
                $each->productId()->value(), $each->quantity()->value(), $each->unitPrice()->value(),
            ]);
        }

        return OrderPlaced::of($order)->getOrThrow();
    }
}
