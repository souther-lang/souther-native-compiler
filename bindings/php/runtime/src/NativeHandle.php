<?php

declare(strict_types=1);

namespace Souther\Runtime;

use FFI\CData;

/**
 * Where a value stands in a library's arena, and the session it was made in.
 *
 * Every way a generated binding reaches the pointer goes through {@see borrow()}, so a value whose
 * run has ended is refused here and nowhere else, before anything reads memory the arena has
 * handed out again.
 */
final class NativeHandle
{
    /** @internal */
    public function __construct(
        private readonly Session $session,
        private readonly CData $pointer,
    ) {
    }

    /** @internal */
    public function session(): Session
    {
        return $this->session;
    }

    /**
     * @internal The pointer, for a function of the library `$into` is a session of.
     *
     * A value made in an outer run may be handed to a call in a run inside it, which ends first.
     * A value made in a run that has ended may not be handed anywhere.
     */
    public function borrow(Session $into): CData
    {
        if (!$this->session->isActive()) {
            throw new Expired('a value was used after the run it was made in ended');
        }
        if ($into->library() !== $this->session->library()) {
            throw new ForeignHandle('a value one library made was handed to another');
        }
        if (!$into->isActive()) {
            throw new Expired('a session was used after its run ended');
        }
        return $this->pointer;
    }

    /** @internal The pointer, for a function reading the value itself. */
    public function read(): CData
    {
        return $this->borrow($this->session);
    }
}
