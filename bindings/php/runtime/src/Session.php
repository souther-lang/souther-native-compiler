<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;
use FFI\CData;
use Raoh\CallableDecoder;
use Raoh\Decoder;
use Raoh\Err;
use Raoh\Issue;
use Raoh\Input\JsonNumber;
use Raoh\Input\JsonObject;
use Raoh\Issues;
use Raoh\Value\Temporal\LocalDate;
use Raoh\Value\Temporal\LocalDateTime;
use Raoh\Value\Temporal\LocalTime;
use Raoh\Path;
use Raoh\Result;

/**
 * One run of a library: what is made in it stands in the scope of the library's arena the run
 * opened, and is handed back when the run ends. A value made in a run is good until then.
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
     * @var list<object> what a value of this run reads that PHP laid out, kept until the run ends:
     *      each function value made of a closure PHP handed over ({@see FunctionSlot})
     */
    private array $kept = [];

    /**
     * @var array<string, CData> each function value made in this run of a closure PHP handed over,
     *      by the slot it was made through and the closure
     */
    private array $made = [];

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
        $this->kept = [];
        $this->made = [];
    }

    /**
     * @internal The function value `$closure` is made into through `$slot` in this run, made by
     * `$make` the first time it is asked for.
     *
     * One for each closure and not one for each time it is handed over: a function value made of a
     * closure calls that closure whenever it is called, and lives as long as the run either way, so
     * a second one would be the same value, and a closure handed over in a loop would hold room for
     * every time it went round until the run ended. The closure is kept by the slot until the run
     * ends, so no other closure is given its id before then.
     *
     * @param \Closure(): CData $make
     */
    public function functionOf(FunctionSlot $slot, \Closure $closure, \Closure $make): CData
    {
        return $this->made[spl_object_id($slot) . ' ' . spl_object_id($closure)] ??= $make();
    }

    /** @internal Keeps `$it` for as long as this run is going: a value of the run reads it. */
    public function keep(object $it): void
    {
        $this->kept[] = $it;
    }

    /**
     * @internal A function value the library answered, held for this session's run, as a closure:
     * each call of it is made by `$calling` in the innermost run going when it is called, which
     * `$binding` finds, handed that run's session, the function value, and what the closure was
     * called with.
     *
     * The function value is held as any other value is ({@see held()}), so a closure called after
     * the run it was answered in has ended is refused before anything reads memory the arena has
     * handed out again.
     *
     * @param class-string<Binding> $binding
     * @param \Closure(Session, CData, mixed...): mixed $calling
     */
    public function callable(string $binding, CData $value, \Closure $calling): \Closure
    {
        $held = $this->held($value);
        return static function (mixed ...$arguments) use ($binding, $held, $calling): mixed {
            $session = $binding::session();
            return $calling($session, $held->borrow($session), ...$arguments);
        };
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
     * Only through the innermost run's session. What a computation makes stands in that run's
     * scope and is dropped when it ends, so one started through an outer session would answer a
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
     * the pointer came from. What a computation answers was made in the scope of the innermost
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
     * so a PHP string, which is any bytes, is asked here and refused as an exception instead — a
     * violation of this binding's own contract, and not a Souther computation refusing a value.
     *
     * A canonical value longer than a `String` holds (spec §what-a-string-holds) is different: it
     * is text this binding correctly handed over, that the language refuses. The library answers
     * whether it wrote one (souther-native-compiler#109), and where it did not, this throws what
     * the library calls `REQUIRED_FORM_HAS_NO_PLACE` — a Souther refusal, and so `$this->library
     * ->failure(...)`, not an `\InvalidArgumentException` beside the UTF-8 one above.
     */
    public function string(string $text): CData
    {
        if (preg_match('//u', $text) !== 1) {
            throw new \InvalidArgumentException('text handed to a Souther library is not UTF-8');
        }
        $ffi = $this->ffi();
        $room = $ffi->new('souther_string');
        $admitted = $ffi->souther_string_of_utf8($this->bytes($text), strlen($text), FFI::addr($room));
        if ($admitted === 0) {
            throw $this->library->failure($this->library->status('REQUIRED_FORM_HAS_NO_PLACE'));
        }
        return $room;
    }

    /**
     * @internal A `Decimal` as the library holds it, made of its integer and its scale, which
     * `Decimal` has already held to what the library takes.
     *
     * The unscaled digits are handed over as bytes, not a `String`: they are the integer's text and
     * never the value's written form, so they are never asked through `string()` and cannot fail
     * the way it can (souther-native-compiler#109).
     */
    public function decimal(Decimal $decimal): CData
    {
        $ffi = $this->ffi();
        $room = $ffi->new('souther_decimal');
        $unscaled = $decimal->unscaled;
        return $this->made($ffi->souther_decimal_of_parts(
            $this->bytes($unscaled), strlen($unscaled), $decimal->scale, FFI::addr($room)), $room, 'Decimal');
    }

    /** @internal A `Decimal` the library answered, as its integer and its scale. */
    public function amount(CData $decimal): Decimal
    {
        $ffi = $this->ffi();
        return new Decimal($this->text($ffi->souther_decimal_unscaled($decimal)),
            $ffi->souther_decimal_scale($decimal));
    }

    /**
     * @internal A `Date` as the library holds it, made in this run of its year, month and day,
     * which `Date` has already held to a day the library takes.
     */
    public function date(Date $date): CData
    {
        return $this->temporal('date', 'souther_date', [$date->year, $date->month, $date->day]);
    }

    /** @internal A `Date` the library answered, as its year, month and day. */
    public function dateOf(CData $date): Date
    {
        [$year, $month, $day] = $this->parts('date', $date, 3);
        return new Date($year, $month, $day);
    }

    /** @internal A `Time` as the library holds it, made in this run of its hour, minute and second. */
    public function time(Time $time): CData
    {
        return $this->temporal('time', 'souther_time', [$time->hour, $time->minute, $time->second]);
    }

    /** @internal A `Time` the library answered, as its hour, minute and second. */
    public function timeOf(CData $time): Time
    {
        [$hour, $minute, $second] = $this->parts('time', $time, 3);
        return new Time($hour, $minute, $second);
    }

    /** @internal A `DateTime` as the library holds it, made in this run of its date's and time's parts. */
    public function dateTime(DateTime $dateTime): CData
    {
        $date = $dateTime->date;
        $time = $dateTime->time;
        return $this->temporal('datetime', 'souther_datetime',
            [$date->year, $date->month, $date->day, $time->hour, $time->minute, $time->second]);
    }

    /** @internal A `DateTime` the library answered, as its date's and time's parts. */
    public function dateTimeOf(CData $dateTime): DateTime
    {
        [$year, $month, $day, $hour, $minute, $second] = $this->parts('datetime', $dateTime, 6);
        return new DateTime(new Date($year, $month, $day), new Time($hour, $minute, $second));
    }

    /** @internal An `Instant` as the library holds it, made in this run of its second and nanosecond. */
    public function instant(Instant $instant): CData
    {
        return $this->temporal('instant', 'souther_instant', [$instant->second, $instant->nano]);
    }

    /** @internal An `Instant` the library answered, as its second and nanosecond. */
    public function instantOf(CData $instant): Instant
    {
        [$second, $nano] = $this->parts('instant', $instant, 2);
        return new Instant($second, $nano);
    }

    /**
     * The temporal of `$type` the library makes of `$parts`, the numbers it means, each an Int
     * (souther-native-compiler#137).
     *
     * @param list<int> $parts
     */
    private function temporal(string $type, string $word, array $parts): CData
    {
        $ffi = $this->ffi();
        $room = $ffi->new($word);
        $made = $ffi->{'souther_' . $type . '_of_parts'}(...[...$parts, FFI::addr($room)]);
        return $this->made($made, $room, $type);
    }

    /**
     * The `$count` numbers the library writes of the temporal of `$type` at `$value`.
     *
     * @return list<int>
     */
    private function parts(string $type, CData $value, int $count): array
    {
        $ffi = $this->ffi();
        $rooms = [];
        for ($at = 0; $at < $count; $at++) {
            $rooms[] = $ffi->new('int64_t');
        }
        $ffi->{'souther_' . $type . '_parts'}($value, ...array_map(fn (CData $room) => FFI::addr($room), $rooms));
        return array_map(fn (CData $room) => $room->cdata, $rooms);
    }

    /**
     * The value a function making one wrote through `$room`, where it answered that it made one. A
     * value this binding hands over is one of its own classes, each held where it is made to what
     * the library takes, so a refusal is the library and this binding disagreeing about what a
     * value is.
     */
    private function made(int $made, CData $room, string $what): CData
    {
        if ($made === 0) {
            throw new \LogicException("the library refused as no {$what} the parts this binding holds as one");
        }
        return $room;
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
        $list = $ffi->new('souther_list');
        return $this->made($ffi->{$construct}($count, ...[...$held, FFI::addr($list)]), $list, 'list');
    }

    /**
     * @internal What `$value`, a value of a union the library said is a case holding a primitive,
     * holds: read by `$read` into room of `$type`, the case's word as the declarations spell it.
     * The library answers whether the value is that case, and a value it said is and reads as not
     * is it and this binding disagreeing.
     */
    public function carried(string $read, string $type, CData $value): CData
    {
        $ffi = $this->ffi();
        $room = $ffi->new($type);
        if ($ffi->{$read}($value, FFI::addr($room)) === 0) {
            throw new \LogicException('the library read a value as a case it said it is not');
        }
        return $room;
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
                $json = self::written($in, 512);
            } catch (\JsonException $unwritten) {
                return Result::fail($at, 'type_mismatch', 'a value no JSON writes: ' . $unwritten->getMessage());
            }
            $read = $decode($json);
            return $read instanceof Err ? Result::err($read->issues->rebase($at)) : $read;
        });
    }

    /**
     * `$in` as the JSON the library reads, each array an object keyed as PHP keyed it. raoh-php
     * reads JSON text into values of its own, an object as a {@see JsonObject} and a number as a
     * {@see JsonNumber} that keeps the text it was written as, and those are written as what they
     * are: the object its members, and the number its text, so `1.50` reaches the library as `1.50`
     * and not as the float nearest it. Nested deeper than `$depth` is refused, as json_encode
     * refuses it.
     *
     * @throws \JsonException where the value holds what JSON does not write
     */
    private static function written(mixed $in, int $depth): string
    {
        if ($depth < 0) {
            throw new \JsonException('nested deeper than ' . 512);
        }
        if ($in instanceof JsonNumber) {
            return $in->lexeme;
        }
        $members = match (true) {
            $in instanceof JsonObject => array_map(
                static fn (string $name): array => [$name, $in->get($name)],
                $in->names(),
            ),
            is_array($in) => array_map(
                static fn (int|string $key, mixed $value): array => [(string) $key, $value],
                array_keys($in),
                array_values($in),
            ),
            default => null,
        };
        if ($members === null) {
            return json_encode($in, JSON_PRESERVE_ZERO_FRACTION | JSON_THROW_ON_ERROR);
        }
        return '{' . implode(',', array_map(
            static fn (array $member): string => json_encode($member[0], JSON_THROW_ON_ERROR) . ':'
                . self::written($member[1], $depth - 1),
            $members,
        )) . '}';
    }

    /**
     * What a reading found, as Raoh's issues. The codes and the metadata are Raoh's already, so
     * this changes how they are held and not what they say: each value of the metadata arrives as
     * the type it is, and is made that type ({@see metaValue}). The library gives no message, so
     * each issue's message is its code until something resolves it, by the message key the library
     * answers (`souther_issue_message_key`): Raoh's own where it gives the issue one
     * (`out_of_range.minimum`), and the code where it gives none.
     */
    private function issues(CData $reading): Issues
    {
        $ffi = $this->ffi();
        $issues = Issues::empty();
        $count = $ffi->souther_decoded_issue_count($reading);
        for ($at = 0; $at < $count; $at++) {
            $issue = $ffi->souther_decoded_issue($reading, $at);
            $meta = array_map(
                self::metaValue(...),
                json_decode(
                    $this->text($ffi->souther_issue_meta($issue)),
                    true,
                    512,
                    JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING,
                ),
            );
            $code = $this->text($ffi->souther_issue_code($issue));
            $key = $this->text($ffi->souther_issue_message_key($issue));
            $issues = $issues->add(Issue::of(
                self::path($this->text($ffi->souther_issue_path($issue))), $code, $code, $meta, $key));
        }
        return $issues;
    }

    /**
     * A value of an issue's metadata as the library writes it, an array of one member named for its
     * type in Raoh's value model, made a value of that type: an int, a
     * {@see \Raoh\Value\Decimal} at the scale it was written at, a string, a bool, a date, a time,
     * a date-time or an instant as Raoh reads one, or a list of the same.
     * So `5` and `1.50` are the decimals they are, as the JVM's issue holds them. The library writes
     * nothing else, and a library of another contract was refused at its generation when it was
     * loaded.
     */
    private static function metaValue(mixed $said): mixed
    {
        if (!is_array($said) || count($said) !== 1) {
            throw new \LogicException('the library writes a value of metadata as its type');
        }
        $value = reset($said);
        return match (key($said)) {
            'int' => is_int($value) ? $value
                : throw new \LogicException('the library writes an int of 64 bits'),
            'decimal' => (is_string($value) ? \Raoh\Value\Decimal::parse($value) : null)
                ?? throw new \LogicException('the library writes a decimal as its text'),
            'string' => is_string($value) ? $value
                : throw new \LogicException('the library writes text as a string'),
            'list' => is_array($value) && array_is_list($value) ? array_map(self::metaValue(...), $value)
                : throw new \LogicException('the library writes a list as an array'),
            'bool' => is_bool($value) ? $value
                : throw new \LogicException('the library writes a bool as a boolean'),
            'date' => (is_string($value) ? LocalDate::parse($value) : null)
                ?? throw new \LogicException('the library writes a date as Raoh reads one'),
            'time' => (is_string($value) ? LocalTime::parse($value) : null)
                ?? throw new \LogicException('the library writes a time as Raoh reads one'),
            'datetime' => (is_string($value) ? LocalDateTime::parse($value) : null)
                ?? throw new \LogicException('the library writes a date-time as Raoh reads one'),
            'instant' => (is_string($value) ? \Raoh\Value\Temporal\Instant::parse($value) : null)
                ?? throw new \LogicException('the library writes an instant as Raoh reads one'),
            default => throw new \LogicException('the library writes no ' . key($said)),
        };
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
