<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;
use FFI\CData;
use Raoh\CallableDecoder;
use Raoh\Decoder;
use Raoh\Err;
use Raoh\Issue;
use Raoh\Issues;
use Raoh\Path;
use Raoh\Result;

/**
 * One run of a library: what is made in it stands in the library's arena from the mark the run
 * took, and is handed back when the run ends. A value made in a run is good until then.
 *
 * Held by the runtime and the binding, and by no host code. A function of a binding asks its
 * `Binding` for the session of the innermost run going on this fiber ({@see Binding::innermostOf()})
 * and makes what it makes there, so which arena a value stands in is decided by the run a call is
 * made in and never chosen by the caller. A value keeps the session it was made in, which is how
 * it is refused once that run has ended, on another fiber, or by another library.
 */
final class Session
{
    private static int $opened = 0;

    private bool $active = true;

    /** Where this run stands among every run opened in the process, later ones higher. */
    private readonly int $order;

    /**
     * @var array<string, Bound> what each behavior called through `Behaviors` is constructed as, by
     *      the binding it was called through and its declared name
     */
    private array $constructed = [];

    /**
     * @internal
     * @param array<string, Implemented> $injected what the run was handed, by the declared name of
     *        the behavior each implements, each with the binding it was written against
     */
    public function __construct(
        private readonly NativeLibrary $library,
        private readonly ?\Fiber $fiber,
        private readonly array $injected,
    ) {
        $this->order = ++self::$opened;
    }

    /** @internal Whether this run was opened after `$other`, which is the one inside the other where both are going. */
    public function openedAfter(self $other): bool
    {
        return $this->order > $other->order;
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
        if (\Fiber::getCurrent() !== $this->fiber) {
            throw new RunOnAnotherFiber(
                'a session, or a value made in it, was used on a fiber other than its run\'s');
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
     * @internal The implementation the run was handed of `$behavior`, which a host implements, with
     * the binding it was written against; null where it was handed none.
     */
    public function injected(string $behavior): ?Implemented
    {
        return $this->injected[$behavior] ?? null;
    }

    /**
     * @internal What `$key` is constructed as in this run, made by `$made` the first time it is
     * asked for and kept for as long as the run is, since what is called with it reads it as long.
     *
     * @param \Closure(): Bound $made
     */
    public function constructedAs(string $key, \Closure $made): Bound
    {
        return $this->constructed[$key] ??= $made();
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
     * The library admits text where it comes in: it puts it in NFC by the Unicode version the
     * language names, which PHP's own normalizer, reading whichever ICU it was built with, need not
     * be. What the library cannot answer is bytes that are not UTF-8, which end the process there,
     * so a PHP string, which is any bytes, is asked here and refused as an exception instead.
     */
    public function string(string $text): CData
    {
        if (preg_match('//u', $text) !== 1) {
            throw new \InvalidArgumentException('text handed to a Souther library is not UTF-8');
        }
        return $this->ffi()->souther_string_of_utf8($this->bytes($text), strlen($text));
    }

    /** @internal Bytes the library reads for the length of one call and does not keep. */
    public function bytes(string $bytes): CData
    {
        $held = $this->ffi()->new('uint8_t[' . max(1, strlen($bytes)) . ']');
        FFI::memcpy($held, $bytes, strlen($bytes));
        return $held;
    }

    /**
     * @internal A list the library builds of `$elements`, through `$construct`.
     *
     * Each element is handed over as `$words` makes it, one word for each of `$columns`, which are
     * what C calls each word; the library is handed a column of each word, every element's at its
     * index. The columns are read for the length of the call and not kept, and the list is made in
     * this run, which is why the call is started through the innermost run's session.
     *
     * @param list<string> $columns
     * @param array<mixed> $elements
     * @param \Closure(mixed): list<mixed> $words
     */
    public function list(string $construct, array $columns, array $elements, \Closure $words): CData
    {
        if (!array_is_list($elements)) {
            throw new \InvalidArgumentException(
                'an array handed to a Souther library as a list has keys other than 0, 1, 2 and on');
        }
        $ffi = $this->call();
        $count = count($elements);
        $held = array_map(
            static fn (string $type): CData => $ffi->new($type . '[' . max(1, $count) . ']'),
            $columns,
        );
        foreach ($elements as $at => $element) {
            foreach ($words($element) as $column => $word) {
                $held[$column][$at] = $word;
            }
        }
        return $ffi->{$construct}($count, ...$held);
    }

    /**
     * @internal The elements of a list the library holds, in order, each made by `$made` out of
     * room for each of `$rooms`, which `$at` wrote the element into.
     *
     * Room of its own for each element, since what is made may hold on to it.
     *
     * @template T
     * @param list<string> $rooms
     * @param \Closure(CData ...): T $made
     * @return list<T>
     */
    public function elements(string $length, string $at, array $rooms, CData $list, \Closure $made): array
    {
        $ffi = $this->ffi();
        $count = $ffi->{$length}($list);
        $elements = [];
        for ($index = 0; $index < $count; $index++) {
            $held = array_map(static fn (string $type): CData => $ffi->new($type), $rooms);
            $inside = $ffi->{$at}($list, $index, ...array_map(
                static fn (CData $room): CData => FFI::addr($room), $held));
            if ($inside === 0) {
                throw new \LogicException("the library answered no element at {$index} of a list it"
                    . " says holds {$count}");
            }
            $elements[] = $made(...$held);
        }
        return $elements;
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
     * @internal A type's reading of a host's value as a raoh-php decoder, which a host composes with
     * its own the way a JVM host composes a type's `decoder()`.
     *
     * What it is handed is a PHP value, and PHP's one container is an ordered map: a list is the
     * array keyed by its indices, and the empty list and the empty object are one value. So the
     * value is written with every array as an object keyed as PHP keyed it, which keeps everything
     * the value says and adds nothing it does not, and `$decode` hands that to the library's reading
     * of a host's value, which takes an array keyed by its indices as a list where the declaration
     * holds one there and as an object where it holds one of those. Text written with `json_encode`
     * would instead have guessed each array into one or the other, and guessed an empty one into a
     * list. What the library finds wrong is found at the path the decoder was reached at. A float
     * stays one, so a `1.0` handed where an `Int` is taken is refused rather than read as `1`.
     *
     * It holds no session: `$decode` finds the run it reads in when it is called, so a decoder can be
     * made once and kept, as a JVM host keeps one in a constant.
     *
     * @template T
     * @param \Closure(string): Result<T> $decode
     * @return Decoder<mixed, T>
     */
    public static function decoder(\Closure $decode): Decoder
    {
        return CallableDecoder::of(static function (mixed $in, ?Path $path = null) use ($decode): Result {
            $at = $path ?? Path::root();
            try {
                $json = json_encode($in, JSON_FORCE_OBJECT | JSON_PRESERVE_ZERO_FRACTION | JSON_THROW_ON_ERROR);
            } catch (\JsonException $unwritten) {
                return Result::fail($at, 'type_mismatch', 'a value no JSON writes: ' . $unwritten->getMessage());
            }
            $read = $decode($json);
            return $read instanceof Err ? Result::err($read->issues->rebase($at)) : $read;
        });
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
