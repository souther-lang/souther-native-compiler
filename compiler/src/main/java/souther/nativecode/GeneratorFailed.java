package souther.nativecode;

import souther.bindings.NotBindable;

import java.io.IOException;
import java.util.ServiceConfigurationError;

/**
 * A generator's own failure: anything its code threw that is not a refusal ({@link NotBindable}),
 * told apart from the command's own by where it was thrown and not by its type, and not by a list of
 * types either: what a generator throws is anything, an {@link Error} of its own among it, and only
 * what ends the process ({@link VirtualMachineError}) is not said as the generator's. An
 * {@link IOException} from a generator and one from the command's own file system work are the same
 * type and two different parties' failures, so every call into a generator's code goes through
 * {@link #asking}, and nothing else is read as the generator's.
 */
final class GeneratorFailed extends Exception {

    GeneratorFailed(String generator, Throwable cause) {
        super(generator + " failed, and wrote nothing: " + said(cause), cause);
    }

    /**
     * What {@code cause} was, as one line: a generator that could not be made is said by what its
     * constructor threw, which {@link java.util.ServiceLoader} wraps.
     */
    private static String said(Throwable cause) {
        Throwable thrown = cause instanceof ServiceConfigurationError && cause.getCause() != null
                ? cause.getCause() : cause;
        return thrown.getClass().getName()
                + (thrown.getMessage() == null ? "" : ": " + thrown.getMessage());
    }

    /** Work that calls into a generator's code. */
    @FunctionalInterface
    interface Work<T> {
        T run() throws IOException;
    }

    /**
     * What {@code work}, a call into the code of {@code generator} (as the command names it),
     * answers; a refusal as it was thrown, and anything else it threw, constructing it among them,
     * as that generator's failure.
     */
    static <T> T asking(String generator, Work<T> work) throws GeneratorFailed {
        try {
            return work.run();
        } catch (NotBindable refused) {
            throw refused;
        } catch (VirtualMachineError fatal) {
            // What ends the process and not the generator: the JVM out of memory or stack.
            throw fatal;
        } catch (Throwable thrown) {
            // Everything else is the generator's, an Error among it: the command says so in one line,
            // and lets go of what it held, rather than end with a trace of someone else's code.
            throw new GeneratorFailed(generator, thrown);
        }
    }
}
