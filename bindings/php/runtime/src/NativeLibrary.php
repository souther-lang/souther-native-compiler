<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;

/**
 * A shared library a binding was generated for, loaded once per process, with the numbers its
 * functions answer.
 *
 * One instance for each library, however many bindings load it: what the library keeps (its arena,
 * what is registered for each behavior a host implements) is the library's and not a binding's, so
 * a value one binding made is one another binding of the same library may hand over.
 */
final class NativeLibrary
{
    /** @var array<string, self> */
    private static array $loaded = [];

    /** @var array<int, string> */
    private readonly array $named;

    /** @var list<Session> */
    private array $open = [];

    /**
     * @param array<string, int> $statuses every status the library answers, by name
     * @param array<string, int> $outcomes what a reading comes to, by name
     */
    private function __construct(
        private readonly FFI $ffi,
        private readonly array $statuses,
        private readonly array $outcomes,
    ) {
        foreach ($statuses as $name => $number) {
            if ($ffi->{'SOUTHER_' . $name} !== $number) {
                throw new \LogicException(
                    "the declarations number {$name} otherwise than the binding was generated with");
            }
        }
        foreach ($outcomes as $name => $number) {
            if ($ffi->{'SOUTHER_DECODED_' . $name} !== $number) {
                throw new \LogicException(
                    "the declarations number the outcome {$name} otherwise than the binding was generated with");
            }
        }
        $this->named = array_flip($statuses);
    }

    /**
     * The library at `$library`, declared by the preprocessor-free declarations the build wrote
     * beside it.
     *
     * @param array<string, int> $statuses
     * @param array<string, int> $outcomes
     */
    public static function load(string $declarations, string $library, array $statuses,
                                array $outcomes): self
    {
        $key = realpath($library);
        if ($key === false) {
            throw new \InvalidArgumentException("no library at {$library}");
        }
        if (isset(self::$loaded[$key])) {
            return self::$loaded[$key];
        }
        $declared = file_get_contents($declarations);
        if ($declared === false) {
            throw new \InvalidArgumentException("no declarations at {$declarations}");
        }
        return self::$loaded[$key] = new self(FFI::cdef($declared, $key), $statuses, $outcomes);
    }

    /**
     * The library `opcache.preload` declared under `$scope`, for `ffi.enable=preload`, where a
     * request cannot declare one itself.
     *
     * @param array<string, int> $statuses
     * @param array<string, int> $outcomes
     */
    public static function preloaded(string $scope, array $statuses, array $outcomes): self
    {
        $key = 'scope:' . $scope;
        return self::$loaded[$key] ??= new self(FFI::scope($scope), $statuses, $outcomes);
    }

    /**
     * What a preload script hands `FFI::load()`: the declarations, under the scope a request asks
     * for and naming where the library is on the machine it is deployed to. Written at deployment
     * and not by the build, which does not know that path.
     */
    public static function preloadHeader(string $declarations, string $scope, string $library): string
    {
        $declared = file_get_contents($declarations);
        if ($declared === false) {
            throw new \InvalidArgumentException("no declarations at {$declarations}");
        }
        return '#define FFI_SCOPE "' . addcslashes($scope, "\"\\") . "\"\n"
            . '#define FFI_LIB "' . addcslashes($library, "\"\\") . "\"\n"
            . $declared;
    }

    /** @internal */
    public function ffi(): FFI
    {
        return $this->ffi;
    }

    /** @internal */
    public function status(string $name): int
    {
        return $this->statuses[$name]
            ?? throw new \LogicException("the library numbers no status {$name}");
    }

    /** @internal */
    public function outcome(string $name): int
    {
        return $this->outcomes[$name]
            ?? throw new \LogicException("the library numbers no outcome {$name}");
    }

    /** @internal A session for a run starting now, inside whichever runs are going. */
    public function open(): Session
    {
        return $this->open[] = new Session($this);
    }

    /** @internal Ends the run `$session` is for, which is the innermost one. */
    public function close(Session $session): void
    {
        if (end($this->open) !== $session) {
            throw new \LogicException('runs end in the order opposite the one they started in');
        }
        array_pop($this->open);
        $session->expire();
    }

    /** @internal The session of the innermost run going. */
    public function current(): Session
    {
        $current = end($this->open);
        if ($current === false) {
            throw new \LogicException('the library was called outside any run');
        }
        return $current;
    }

    /** @internal What a status other than `ANSWERED` means, as something to throw. */
    public function failure(int $status): \Throwable
    {
        $name = $this->named[$status] ?? null;
        return match ($name) {
            'HOST_EXCEPTION' => Pending::take()
                ?? new InjectionProtocolViolation('an implementation answered that it threw, and nothing was kept'),
            'INJECTION_UNBOUND' => new UnboundInjection(
                'a behavior the host implements was called with nothing registered for it'),
            'INJECTION_PROTOCOL_VIOLATION' => new InjectionProtocolViolation(
                'an implementation answered something other than a value or an exception'),
            null => new SoutherAbort("status {$status}", $status),
            default => new SoutherAbort($name, $status),
        };
    }
}
