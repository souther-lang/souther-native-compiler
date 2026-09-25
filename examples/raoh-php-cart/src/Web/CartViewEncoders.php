<?php

declare(strict_types=1);

namespace App\Web;

use App\Infrastructure\Page;
use Model\Com\Example\Cart\Domain\CartItem;

/** The response body for a page of a cart's items. */
final class CartViewEncoders
{
    /** @return array<string, mixed> */
    public static function itemView(CartItem $item): array
    {
        return [
            'productId' => $item->productId()->value(),
            'quantity' => $item->quantity()->value(),
        ];
    }

    /**
     * @param Page<CartItem> $page
     * @return array<string, mixed>
     */
    public static function pageView(Page $page): array
    {
        return [
            'total' => $page->total,
            'page' => $page->page,
            'size' => $page->size,
            'items' => array_map(self::itemView(...), $page->items),
        ];
    }
}
