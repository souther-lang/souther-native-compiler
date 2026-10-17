<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI;

/**
 * One `example` row of a behavior a host implements, as the class generated for the behavior lists
 * it (`examples()`), and what evaluating it against one implementation comes to (`evaluate()`).
 *
 * The two are apart, and there is no third that runs every row. What an implementation answers
 * comes out of what the host's world holds when it is asked, the rows of a table or what a clock
 * says, and that is the test's to arrange between one row and the next: whoever changes it owns the
 * loop, which is a test framework's, and one exists. So a row is a value a PHPUnit data provider
 * can hand a test, and the test evaluates it once in the world it set up, as often and under as many
 * worlds as it likes. Nothing of one evaluation is kept for the next.
 *
 * Whether an answer keeps the row is the library's to say, by the language's equality, and never
 * this runtime's: `1.50` and `1.5` are one `Decimal` and a set is the same set in any order, which a
 * comparison of the text they are written as would not say. The answer's external form comes back
 * beside the verdict only for a report to show.
 */
final readonly class Row
{
    /**
     * @param class-string<Binding> $binding the binding the class listing the row was generated in
     * @param string $behavior the declared name of the behavior the row is of
     * @param ?string $run the library's function running the row, where there is one
     * @param int $at where the row stands among the behavior's rows, from nought, in the order
     *        they are written
     * @param ?string $name the name the row was written with, or null where it was written with none
     * @param string $shown what a report calls the row: its name, or which row it is
     * @param ?string $why why there is nothing to run, where there is nothing
     */
    private function __construct(
        private string $binding,
        private string $behavior,
        private ?string $run,
        public int $at,
        public ?string $name,
        public string $shown,
        public RowKind $kind,
        public ?string $why,
    ) {
    }

    /**
     * @internal A row stating an answer, run through `$run`, or, where `$run` is null, one stating
     * an answer the library cannot hold an answer to.
     *
     * @param class-string<Binding> $binding
     */
    public static function asserted(string $binding, string $behavior, int $at, ?string $name,
                                    string $shown, ?string $run): self
    {
        return $run === null
            ? new self($binding, $behavior, null, $at, $name, $shown, RowKind::NotComparable,
                'the library has no way to hold an answer to what the row states')
            : new self($binding, $behavior, $run, $at, $name, $shown, RowKind::Asserted, null);
    }

    /**
     * @internal A row whose answer is owed, run through `$run`.
     *
     * @param class-string<Binding> $binding
     */
    public static function owed(string $binding, string $behavior, int $at, ?string $name,
                                string $shown, string $run): self
    {
        return new self($binding, $behavior, $run, $at, $name, $shown, RowKind::Owed, null);
    }

    /**
     * @internal A row the compile could not read, for the reason `$why` says.
     *
     * @param class-string<Binding> $binding
     */
    public static function unreproducible(string $binding, string $behavior, int $at, ?string $name,
                                          string $shown, string $why): self
    {
        return new self($binding, $behavior, null, $at, $name, $shown, RowKind::Unreproducible,
            $why);
    }

    /**
     * @internal What `$implementation` of `$behavior`, written against `$binding`, answers this
     * row, in the innermost run going: called with the row's inputs through a capability made of it
     * as one handed to a run is made, and held to the row by the library.
     *
     * What the row's function ends with otherwise than an answer is said as what a call of a
     * behavior throws for it ({@see NativeLibrary::failure()}), and answered rather than thrown:
     * the exception the implementation threw, an `UnboundInjection`, or a `SoutherAbort` naming the
     * status. It is an outcome of the row as kept and not kept are, and one a report keeps apart
     * from both. A row with nothing to run answers why, and nothing is called.
     *
     * @param class-string<Binding> $binding
     * @throws \InvalidArgumentException where the row is another behavior's, or another binding's
     */
    public function evaluate(string $binding, string $behavior, \Closure $implementation): RowOutcome
    {
        if ($binding !== $this->binding || $behavior !== $this->behavior) {
            throw new \InvalidArgumentException("a row of {$this->behavior}, listed by {$this->binding},"
                . " was handed to the evaluation of {$behavior} by {$binding}");
        }
        if ($this->run === null) {
            return new RowNotRun((string) $this->why);
        }
        $session = $binding::session();
        $ffi = $session->call();
        $library = $session->library();
        // Kept until the call returns: the capability is let go of with what made it.
        $handed = Bound::of(null, Implemented::by($binding, $behavior, $implementation));
        $answer = $ffi->new('souther_string');
        if ($this->kind === RowKind::Asserted) {
            $kept = $ffi->new('uint8_t');
            $status = $ffi->{$this->run}($handed->requirements($session), FFI::addr($kept),
                FFI::addr($answer));
        } else {
            $kept = null;
            $status = $ffi->{$this->run}($handed->requirements($session), FFI::addr($answer));
        }
        if ($status !== $library->status('ANSWERED')) {
            return new RowEnded($library->failure($status));
        }
        $written = $session->text($answer);
        if ($kept === null) {
            return new RowAnswered($written);
        }
        return $kept->cdata !== 0 ? new RowKept($written) : new RowNotKept($written);
    }
}
