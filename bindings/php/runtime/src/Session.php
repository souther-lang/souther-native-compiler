<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;
use FFI\CData;
use Raoh\Issue;
use Raoh\Issues;
use Raoh\Path;
use Raoh\Result;

/**
 * One run of a library: what is made in it stands in the library's arena from the mark the run
 * took, and is handed back when the run ends. A value made in a run is good until then.
 *
 * Handed to the body of {@see Binding::run()} and to every implementation the library calls during
 * it. Everything that makes a value takes one, so which arena a value stands in is never a
 * question of what happens to be loaded.
 */
final class Session
{
    private bool $active = true;

    /** @internal */
    public function __construct(private readonly NativeLibrary $library)
    {
    }

    public function isActive(): bool
    {
        return $this->active;
    }

    /** @internal */
    public function library(): NativeLibrary
    {
        return $this->library;
    }

    /** @internal */
    public function expire(): void
    {
        $this->active = false;
    }

    /** @internal The library's functions, to read what a value of a run still going holds. */
    public function ffi(): FFI
    {
        if (!$this->active) {
            throw new Expired('a session was used after its run ended');
        }
        return $this->library->ffi();
    }

    /**
     * @internal The library's functions, to start a computation: construct, read, call.
     *
     * Only through the innermost run's session. What a computation makes stands after that run's
     * mark and is dropped when it ends, so one started through an outer session would answer a
     * value its caller holds for longer than it lives.
     */
    public function call(): FFI
    {
        $ffi = $this->ffi();
        if ($this->library->current() !== $this) {
            throw new NotTheInnermostRun(
                'a computation was started through the session of a run with another run going'
                . ' inside it; start it through the session of the innermost run');
        }
        return $ffi;
    }

    /**
     * @internal A value the library answered, held for this session's run.
     *
     * Asked through the session the value's memory is dropped with, which a binding knows by where
     * the pointer came from. What a computation answers was made after the mark of the innermost
     * run, and a computation is started only through that run's session ({@see call()}). What a
     * reader answers is a value the one it was read out of already held, made no later than it, so
     * it is asked through that value's session. What an implementation is handed is asked through
     * the innermost run's, which is no longer than the value lives.
     */
    public function held(CData $pointer): NativeHandle
    {
        return new NativeHandle($this, $pointer);
    }

    /**
     * @internal Text as the library holds it.
     *
     * The library takes text as UTF-8 already in the form Souther keeps it in, NFC, and checks
     * neither: bytes that are not would make two equal texts compare unequal. So both are done
     * here, where a PHP string, which is any bytes, becomes one.
     */
    public function string(string $text): CData
    {
        if (preg_match('//u', $text) !== 1) {
            throw new \InvalidArgumentException('text handed to a Souther library is not UTF-8');
        }
        $normalized = \Normalizer::normalize($text, \Normalizer::FORM_C);
        if ($normalized === false) {
            throw new \InvalidArgumentException('text handed to a Souther library could not be put in NFC');
        }
        return $this->ffi()->souther_string_of_utf8($this->bytes($normalized), strlen($normalized));
    }

    /** @internal Bytes the library reads for the length of one call and does not keep. */
    public function bytes(string $bytes): CData
    {
        $held = $this->ffi()->new('uint8_t[' . max(1, strlen($bytes)) . ']');
        FFI::memcpy($held, $bytes, strlen($bytes));
        return $held;
    }

    /** @internal Text the library answered, as a PHP string. */
    public function text(CData $string): string
    {
        $ffi = $this->ffi();
        $length = $ffi->souther_string_length($string);
        return $length === 0 ? '' : FFI::string($ffi->souther_string_bytes($string), $length);
    }

    /** @internal Throws what a status other than `ANSWERED` means. */
    public function answered(int $status): void
    {
        if ($status !== $this->library->status('ANSWERED')) {
            throw $this->library->failure($status);
        }
    }

    /**
     * @internal What a constructor answered: the value, or an `invariant_violation` where it does
     * not hold what its type states. Any other status is thrown.
     *
     * @template T
     * @param \Closure(): T $made
     * @return Result<T>
     */
    public function constructed(int $status, \Closure $made): Result
    {
        if ($status === $this->library->status('INVARIANT_NOT_HELD')) {
            return Result::fail(Path::root(), 'invariant_violation', 'invariant_violation');
        }
        $this->answered($status);
        return Result::ok($made());
    }

    /**
     * @internal What a reading of external text came to: the value, the issues it found, or an
     * `invalid_format` where the text is not JSON at all.
     *
     * @template T
     * @param \Closure(CData): T $made
     * @return Result<T>
     */
    public function decoded(int $status, CData $reading, \Closure $made): Result
    {
        $this->answered($status);
        $ffi = $this->ffi();
        $outcome = $ffi->souther_decoded_outcome($reading);
        if ($outcome === $this->library->outcome('VALUE')) {
            return Result::ok($made($ffi->souther_decoded_value($reading)));
        }
        if ($outcome === $this->library->outcome('ISSUES')) {
            return Result::err($this->issues($reading));
        }
        $at = $ffi->souther_decoded_malformed_at($reading);
        return Result::fail(Path::root(), 'invalid_format', "the text stops being JSON at byte {$at}");
    }

    /**
     * What a reading found, as Raoh's issues. The codes are Raoh's already, so this changes how
     * they are held and not what they say. The library gives no message, so each issue's message is
     * its code until something resolves it.
     */
    private function issues(CData $reading): Issues
    {
        $ffi = $this->ffi();
        $issues = Issues::empty();
        $count = $ffi->souther_decoded_issue_count($reading);
        for ($at = 0; $at < $count; $at++) {
            $issue = $ffi->souther_decoded_issue($reading, $at);
            $meta = [];
            $entries = $ffi->souther_issue_meta_count($issue);
            for ($entry = 0; $entry < $entries; $entry++) {
                $meta[$this->text($ffi->souther_issue_meta_key($issue, $entry))] =
                    $this->text($ffi->souther_issue_meta_value($issue, $entry));
            }
            $code = $this->text($ffi->souther_issue_code($issue));
            $issues = $issues->add(Issue::of(
                self::path($this->text($ffi->souther_issue_path($issue))), $code, $code, $meta));
        }
        return $issues;
    }

    /** A JSON pointer as Raoh holds a path. */
    private static function path(string $pointer): Path
    {
        if ($pointer === '') {
            return Path::root();
        }
        $segments = array_map(
            static fn (string $segment): string => str_replace(['~1', '~0'], ['/', '~'], $segment),
            explode('/', substr($pointer, 1)),
        );
        return Path::of(...$segments);
    }
}
