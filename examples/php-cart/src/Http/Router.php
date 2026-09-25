<?php

declare(strict_types=1);

namespace App\Http;

/**
 * A method and a path to a handler, and nothing else: the example shows the binding, not a
 * framework's container, so a route is a line of code.
 */
final class Router
{
    /** @var array<string, array<string, \Closure(Request): Response>> */
    private array $routes = [];

    /** @param \Closure(Request): Response $handler */
    public function get(string $path, \Closure $handler): self
    {
        $this->routes['GET'][$path] = $handler;
        return $this;
    }

    /** @param \Closure(Request): Response $handler */
    public function post(string $path, \Closure $handler): self
    {
        $this->routes['POST'][$path] = $handler;
        return $this;
    }

    public function handle(Request $request): Response
    {
        $handler = $this->routes[$request->method][$request->path] ?? null;
        return $handler === null ? Response::notFound() : $handler($request);
    }
}
