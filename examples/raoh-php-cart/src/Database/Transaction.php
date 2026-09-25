<?php

declare(strict_types=1);

namespace App\Database;

use PDO;

/** Runs a piece of work in one database transaction, rolled back where it throws. */
final readonly class Transaction
{
    public function __construct(private PDO $pdo)
    {
    }

    /**
     * @template T
     * @param callable(): T $work
     * @return T
     */
    public function execute(callable $work): mixed
    {
        $this->pdo->beginTransaction();
        try {
            $done = $work();
            $this->pdo->commit();
            return $done;
        } catch (\Throwable $thrown) {
            $this->pdo->rollBack();
            throw $thrown;
        }
    }
}
