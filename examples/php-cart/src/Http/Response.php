<?php

declare(strict_types=1);

namespace App\Http;

use Raoh\Issues;

/** A status and, where there is one, a body that is JSON text. */
final readonly class Response
{
    private function __construct(public int $status, public ?string $body)
    {
    }

    /** A 200 with `$json`, which is what a value of the model encodes to, or any other JSON. */
    public static function ok(string $json): self
    {
        return new self(200, $json);
    }

    public static function created(?string $json = null): self
    {
        return new self(201, $json);
    }

    /**
     * raoh-php's issues, each with its path, and the messages by path.
     *
     * An issue's `meta` and the messages by path are maps, and a PHP array does not say whether it
     * is a map or a list: an empty one is written as `[]`. So each is written as the object it is.
     */
    public static function badRequest(Issues $issues): self
    {
        return new self(400, self::json([
            'issues' => array_map(
                static fn (array $issue): array => ['meta' => (object) $issue['meta']] + $issue,
                $issues->toJsonList()),
            'errors' => (object) $issues->flatten(),
        ]));
    }

    public static function notFound(): self
    {
        return new self(404, self::json(['error' => 'not_found']));
    }

    public static function unprocessable(string $error): self
    {
        return new self(422, self::json(['error' => $error]));
    }

    public static function json(mixed $body): string
    {
        return json_encode($body, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
    }

    public function send(): void
    {
        http_response_code($this->status);
        if ($this->body !== null) {
            header('Content-Type: application/json');
            echo $this->body;
        }
    }
}
