package souther.bindings;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;

/**
 * Writes the binding of a library for one host's language, found by the command through
 * {@link java.util.ServiceLoader}.
 *
 * <p>Which bindings there are, and what each is asked with on the command line, is the command's
 * own catalog, and a generator is asked by the {@link #id} that catalog names it by. What each
 * option means is the generator's: the command hands over the values as they were written, keyed
 * by the option's name without its dashes, and a generator that is missing one it needs, or given
 * one it refuses, says so in {@link #preflight}.
 *
 * <p>A generation has two moments at which it can refuse, and they are apart because the second
 * needs a library the first is meant to spare building. {@link #preflight} refuses what holds
 * whatever the model says: an option missing or not one the language takes, a directory that holds
 * what no generation of this host wrote. {@link #generate} refuses what only the model can say: a
 * name in it the language will not take. Both refuse with {@link NotBindable}.
 */
public interface BindingGenerator {

    /** The id the command's catalog names this generator by. */
    String id();

    /**
     * Refuses, before anything is built, what would be refused whatever the manifest says.
     *
     * @throws NotBindable where an option or the directory would be refused
     */
    void preflight(Path into, Map<String, String> options) throws IOException;

    /**
     * Writes the binding into {@code into}, replacing what a generation of this host wrote there.
     *
     * @throws NotBindable where a name in the model is not one the language takes
     */
    Generated generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException;
}
