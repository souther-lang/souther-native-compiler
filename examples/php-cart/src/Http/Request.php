<?php

declare(strict_types=1);

namespace App\Http;

/** What a handler is given of an HTTP request: its method, path, query and body, as they came. */
final readonly class Request
{
    /** @param array<string, string> $query */
    public function __construct(
        public string $method,
        public string $path,
        public array $query = [],
        public string $body = '',
    ) {
    }

    public static function fromGlobals(): self
    {
        $path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH);
        /** @var array<string, string> $query */
        $query = array_filter($_GET, is_string(...));
        return new self(
            $_SERVER['REQUEST_METHOD'] ?? 'GET',
            is_string($path) ? $path : '/',
            $query,
            (string) file_get_contents('php://input'),
        );
    }
}
