package souther.bindings;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;
import java.util.Set;

/**
 * Writes the binding of a library for one host's language, found by the command through
 * {@link java.util.ServiceLoader}.
 *
 * <p>A generator writes files into a directory, and does nothing else to the file system. Where the
 * binding goes, what was there before, and putting it in place are the command's: it hands each
 * generator an empty directory of its own, and puts what was written there in place of the
 * directory the binding was asked for only once every generator it asked has written its own. So a
 * generator that refuses or fails changes no directory a binding was asked for, and no earlier
 * binding of the same command either. A failure of the file system while the command puts the
 * bindings in place is the one thing that can leave some of them in place and not others, since
 * several directories are never replaced as one.
 *
 * <p>Which bindings there are, and what each is asked with on the command line, is the command's
 * own catalog, and a generator is asked by the {@link #id} that catalog names it by. Options are
 * handed over as they were written, keyed by the option's name without its dashes, and only those
 * {@link #options} names. What each means is the generator's: one it needs and was not given, or
 * one it refuses, is refused in {@link #preflight}.
 *
 * <p>A generation has two moments at which it can refuse, and they are apart because the second
 * needs a library the first is meant to spare building. {@link #preflight} refuses what holds
 * whatever the model says: an option missing or not one the language takes. {@link #generate}
 * refuses what only the model can say: a name in it the language will not take. Both refuse with
 * {@link NotBindable}. Anything else a generator throws is its own failure, which the command
 * reports as that.
 */
public interface BindingGenerator {

    /** The id the command's catalog names this generator by. */
    String id();

    /** The options this generator takes, each named without its dashes. */
    Set<String> options();

    /**
     * Refuses, before anything is built, what would be refused whatever the model says.
     *
     * @param options every option given for this binding, each one of {@link #options}
     * @throws NotBindable where an option would be refused
     */
    void preflight(Map<String, String> options);

    /**
     * Writes the binding into {@code into}.
     *
     * @param input   what the library is, shared by every generator the command asks, and changed
     *                by none: every part of it is immutable
     * @param into    an empty directory that is this generator's alone until it returns
     * @param options as {@link #preflight} was handed them
     * @throws NotBindable where a name in the model is not one the language takes
     */
    void generate(BindingInput input, Path into, Map<String, String> options) throws IOException;
}
