<?php

declare(strict_types=1);

namespace App\Database;

use App\Http\Outcome;
use App\Http\Response;
use PDO;

/**
 * Runs a piece of work in one database transaction, committed where the work's outcome keeps what
 * it wrote and rolled back where it does not, or where the work throws.
 */
final readonly class Transaction
{
    public function __construct(private PDO $pdo)
    {
    }

    /** @param callable(): Outcome $work */
    public function execute(callable $work): Response
    {
        $this->pdo->beginTransaction();
        try {
            $outcome = $work();
        } catch (\Throwable $thrown) {
            $this->pdo->rollBack();
            throw $thrown;
        }
        if ($outcome->kept) {
            $this->pdo->commit();
        } else {
            $this->pdo->rollBack();
        }
        return $outcome->response;
    }
}
