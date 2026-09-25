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

    /**
     * The runs going, innermost last. They nest as calls do, and each ends before the one it was
     * started in: the arena is reset to each run's mark, and what each registered is put back, in
     * that order and no other.
     *
     * @var list<Session>
     */
    private array $open = [];

    /**
     * The fiber the runs going are on, the main one being null; meaningful while any is going.
     *
     * A fiber can be suspended in the middle of a run and another resumed, which would start a run
     * of its own on top and could end its first. So while a run is going, the fiber it is on is the
     * only one that may start another or use one: the runs stay one stack whatever fibers there
     * are.
     */
    private ?\Fiber $holder = null;

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
        $key = self::identity($library);
        if (isset(self::$loaded[$key])) {
            return self::$loaded[$key];
        }
        $declared = file_get_contents($declarations);
        if ($declared === false) {
            throw new \InvalidArgumentException("no declarations at {$declarations}");
        }
        return self::$loaded[$key] = new self(FFI::cdef($declared, $library), $statuses, $outcomes);
    }

    /**
     * The library at `$library`, as `opcache.preload` declared it under `$scope`, for
     * `ffi.enable=preload`, where a request cannot declare one itself.
     *
     * @param array<string, int> $statuses
     * @param array<string, int> $outcomes
     */
    public static function preloaded(string $scope, string $library, array $statuses,
                                     array $outcomes): self
    {
        return self::$loaded[self::identity($library)]
            ??= new self(FFI::scope($scope), $statuses, $outcomes);
    }

    /**
     * Which file `$library` is, as the loader tells files apart: by device and inode, and not by
     * a path. The arena and what is registered are the library's, one for each file however it is
     * reached, so two instances over one file would be two stacks of runs over one arena, each
     * taking the other's inner run for its own.
     */
    private static function identity(string $library): string
    {
        $stat = @stat($library);
        if ($stat === false) {
            throw new \InvalidArgumentException("no library at {$library}");
        }
        return $stat['dev'] . ':' . $stat['ino'];
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

    /**
     * @internal A session for a run starting now, inside whichever runs are going, registering
     * implementations through `$registry`.
     */
    public function open(InjectionRegistry $registry): Session
    {
        $fiber = \Fiber::getCurrent();
        if ($this->open !== [] && $fiber !== $this->holder) {
            throw new RunOnAnotherFiber(
                'a run of this library is going on another fiber, which has to end it first');
        }
        $this->holder = $fiber;
        return $this->open[] = new Session($this, $fiber, $registry);
    }

    /** @internal Ends the run `$session` is for, which is the innermost one. */
    public function close(Session $session): void
    {
        if (end($this->open) !== $session) {
            throw new \LogicException('runs end in the order opposite the one they started in');
        }
        array_pop($this->open);
        $session->expire();
        if ($this->open === []) {
            $this->holder = null;
        }
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
