package souther.nativecode;

import souther.bindings.BindingGenerator;

import java.io.IOException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.ServiceLoader;

/** The generators a command can use, each by the id the catalog names it by. */
public final class Bindings {

    private final Map<String, BindingGenerator> byId = new HashMap<>();

    private Bindings(Iterable<BindingGenerator> generators) {
        for (BindingGenerator generator : generators) {
            BindingGenerator earlier = byId.putIfAbsent(generator.id(), generator);
            if (earlier != null) {
                throw new IllegalStateException("two generators answer to \"" + generator.id()
                        + "\": " + earlier.getClass().getName() + " and "
                        + generator.getClass().getName());
            }
        }
    }

    /** The generators on the class path. */
    static Bindings installed() {
        return new Bindings(ServiceLoader.load(BindingGenerator.class));
    }

    /** Exactly these generators, for a caller that has chosen them. */
    static Bindings of(List<BindingGenerator> generators) {
        return new Bindings(generators);
    }

    /**
     * The generator of {@code kind} that {@code jar} provides, which is then one of those found.
     *
     * <p>Loaded beside the compiler and not into it: it sees the API it is written against and what
     * the compiler is written against, and is found through its own service file. A jar that
     * provides no generator for the id it was fetched for is not what the catalog said it was.
     */
    BindingGenerator load(Path jar, KnownBindings.Kind kind) throws IOException {
        URLClassLoader loader = new URLClassLoader(new URL[] {jar.toUri().toURL()},
                Bindings.class.getClassLoader());
        for (BindingGenerator generator : ServiceLoader.load(BindingGenerator.class, loader)) {
            if (generator.id().equals(kind.id())) {
                byId.put(kind.id(), generator);
                return generator;
            }
        }
        loader.close();
        throw new NotFetched(jar + " provides no generator for \"" + kind.id() + "\", which "
                + kind.artifact() + " is to");
    }

    BindingGenerator generatorFor(KnownBindings.Kind kind) throws BindingUnavailable {
        BindingGenerator generator = byId.get(kind.id());
        if (generator == null) {
            throw new BindingUnavailable(kind);
        }
        return generator;
    }
}
