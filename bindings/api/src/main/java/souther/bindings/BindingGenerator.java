package souther.bindings;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;

/**
 * Writes the binding of a library for one host's language.
 *
 * <p>A generator is one jar, which the command is pointed at: by a Maven coordinate and the SHA-256
 * of the jar, by a path to a jar, or, for the bindings the command ships, by its own catalog. The
 * jar says which generator it is and what it is compatible with in its manifest ({@link BindingApi}),
 * and provides exactly one implementation of this interface, named in its
 * {@code META-INF/services/souther.bindings.BindingGenerator}. The command reads the manifest before
 * any of the jar's code runs, and constructs the one implementation through
 * {@link java.util.ServiceLoader}. What the generator can resolve is the JDK, {@code souther.bindings}
 * and what its own jar holds; a library it needs, it carries inside that jar.
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
 * <p>A generator is handed the options it was asked with, keyed by name. How an option is spelt on
 * the command line is the command's; which keys there are and what each value means is the
 * generator's. {@link #preflight} refuses a key it does not take, one it needs and was not given, and
 * a value it will not take.
 *
 * <p>A generation has two moments at which it can refuse, and they are apart because the second
 * needs a library the first is meant to spare building. {@link #preflight} refuses what holds
 * whatever the model says: an option missing, unknown, or not one the language takes.
 * {@link #generate} refuses what only the model can say: a name in it the language will not take.
 * Both refuse with {@link NotBindable}. Anything else a generator throws is its own failure, which
 * the command reports as that.
 */
public interface BindingGenerator {

    /**
     * Refuses, before anything is built, what would be refused whatever the model says.
     *
     * @param options every option given for this binding
     * @throws NotBindable where an option is unknown, missing, or would be refused
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
