package souther.nativecode;

import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;

/**
 * What a native run came to: a value, or the reason it ended without one.
 *
 * <p>Two cases and not the {@code Optional<ObservedValue>} this replaces, because issue #9 is
 * exactly the difference between them: a native run used to be able to say only that no value came
 * back, and now it says which {@link AbortKind} the language itself ended it for. A process that
 * failed on its own account — a symbol the harness would not link, a signal neither case here
 * means — is neither of these; it is a harness failure, and {@link Running} throws rather than
 * answering one of these for it, the same way it always refused to call that a {@code Optional}
 * held.
 */
sealed interface RunOutcome {

    record Answered(ObservedValue value) implements RunOutcome {}

    record Aborted(AbortKind kind) implements RunOutcome {}
}
