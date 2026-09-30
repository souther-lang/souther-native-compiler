package souther.nativecode;

import java.io.IOException;
import java.util.ArrayDeque;
import java.util.Deque;

/**
 * What a step that acquires several things holds until it has acquired them all: a copy of a jar, a
 * class loader, a directory being written. Each is released on every way out of the step, whatever
 * was thrown, and only a step that finishes hands them on ({@link #handOver}), after which it holds
 * nothing.
 *
 * <p>Releasing in a {@code catch} names the throwables it releases on, and whatever it did not name,
 * an {@link Error} from someone else's code or a type added later, left what was acquired behind.
 * So a step that acquires is written as {@code try (Holding held = new Holding()) { ... }}, and never
 * releases in a {@code catch}.
 */
final class Holding implements AutoCloseable {

    /** How one thing held is let go of. */
    @FunctionalInterface
    interface Release {
        void release() throws Exception;
    }

    private final Deque<Release> held = new ArrayDeque<>();

    /** Holds what {@code release} lets go of, until the step ends or hands it on. */
    <T> T hold(T acquired, Release release) {
        held.push(release);
        return acquired;
    }

    /** Hands on everything held: the step finished, and what it acquired is its result's now. */
    void handOver() {
        held.clear();
    }

    /**
     * Lets go of everything still held, the last acquired first. A failure to let go of one does not
     * stop the others, and is thrown once they are all let go of: added to what the step threw, where
     * it threw.
     */
    @Override
    public void close() throws IOException {
        IOException failed = null;
        while (!held.isEmpty()) {
            try {
                held.pop().release();
            } catch (Exception e) {
                if (failed == null) {
                    failed = new IOException("could not let go of what a step held: "
                            + e.getMessage(), e);
                } else {
                    failed.addSuppressed(e);
                }
            }
        }
        if (failed != null) {
            throw failed;
        }
    }
}
