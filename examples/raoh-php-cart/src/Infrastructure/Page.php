<?php

declare(strict_types=1);

namespace App\Infrastructure;

/**
 * One page of a listing. The model does not generate it, so it is a small value of the boundary's.
 *
 * @template T
 */
final readonly class Page
{
    /** @param list<T> $items */
    public function __construct(
        public int $total,
        public int $page,
        public int $size,
        public array $items,
    ) {
    }
}
