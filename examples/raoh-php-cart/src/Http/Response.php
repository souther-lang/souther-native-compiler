<?php

declare(strict_types=1);

namespace App\Http;

/** A status and, where there is one, a body that is written as JSON. */
final readonly class Response
{
    private function __construct(public int $status, public mixed $body)
    {
    }

    public static function ok(mixed $body): self
    {
        return new self(200, $body);
    }

    public static function created(mixed $body = null): self
    {
        return new self(201, $body);
    }

    public static function badRequest(mixed $body): self
    {
        return new self(400, $body);
    }

    public static function notFound(): self
    {
        return new self(404, ['error' => 'not_found']);
    }

    public static function unprocessable(mixed $body): self
    {
        return new self(422, is_string($body) ? ['error' => $body] : $body);
    }

    public function json(): string
    {
        return $this->body === null
            ? ''
            : json_encode($this->body, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
    }

    public function send(): void
    {
        http_response_code($this->status);
        if ($this->body !== null) {
            header('Content-Type: application/json');
            echo $this->json();
        }
    }
}
