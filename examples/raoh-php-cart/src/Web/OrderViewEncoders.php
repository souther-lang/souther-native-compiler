<?php

declare(strict_types=1);

namespace App\Web;

use Model\Com\Example\Cart\Domain\Corporation;
use Model\Com\Example\Cart\Domain\Individual;
use Model\Com\Example\Cart\Domain\Order;
use Model\Com\Example\Cart\Domain\OrderLine;
use Model\Com\Example\Cart\Domain\Orderer;
use Model\Com\Example\Cart\Domain\Quotation;

/**
 * The response bodies for an order and a quotation, read off the model's values through their
 * accessors. The amounts are the `Charge` the model worked out, as it worked them out.
 *
 * What this answers is plain PHP arrays: the values it reads belong to the run they were made in,
 * and the response is written after that run has ended.
 */
final class OrderViewEncoders
{
    /** @return array<string, mixed> */
    public static function orderView(Order $order): array
    {
        return [
            'orderId' => $order->id()->value(),
            'orderer' => self::ordererView($order->orderer()),
            'subtotal' => $order->charge()->subtotal()->value(),
            'discount' => $order->charge()->discount()->value(),
            'total' => $order->charge()->total()->value(),
            'lines' => array_map(self::lineView(...), $order->lines()),
        ];
    }

    /** @return array<string, mixed> */
    public static function quotationView(Quotation $quote): array
    {
        return [
            'quoteId' => $quote->id()->value(),
            'orderer' => self::ordererView($quote->orderer()),
            'subtotal' => $quote->charge()->subtotal()->value(),
            'discount' => $quote->charge()->discount()->value(),
            'total' => $quote->charge()->total()->value(),
            'validUntil' => $quote->validUntil(),
            'lines' => array_map(self::lineView(...), $quote->lines()),
        ];
    }

    /** @return array<string, mixed> */
    private static function lineView(OrderLine $line): array
    {
        $unitPrice = $line->unitPrice()->value();
        $quantity = $line->quantity()->value();
        return [
            'productId' => $line->productId()->value(),
            'quantity' => $quantity,
            'unitPrice' => $unitPrice,
            'subtotal' => $unitPrice * $quantity,
        ];
    }

    /** @return array<string, mixed> */
    private static function ordererView(Orderer $orderer): array
    {
        return match ($orderer::class) {
            Individual::class => [
                'type' => 'individual',
                'email' => $orderer->email()->value(),
                'name' => $orderer->name(),
            ],
            Corporation::class => [
                'type' => 'corporation',
                'email' => $orderer->email()->value(),
                'companyName' => $orderer->companyName(),
                'corporateNumber' => $orderer->corporateNumber(),
            ],
        };
    }
}
