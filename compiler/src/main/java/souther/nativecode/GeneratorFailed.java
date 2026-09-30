package souther.nativecode;

import souther.bindings.NotBindable;

import java.io.IOException;
import java.util.ServiceConfigurationError;

/**
 * A generator's own failure: anything its code threw that is not a refusal ({@link NotBindable}),
 * told apart from the command's own by where it was thrown and not by its type. An
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
        } catch (IOException | RuntimeException | LinkageError | ServiceConfigurationError e) {
            throw new GeneratorFailed(generator, e);
        }
    }
}
