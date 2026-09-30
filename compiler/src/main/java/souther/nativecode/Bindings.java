package souther.nativecode;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;

import java.io.IOException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Iterator;
import java.util.Map;
import java.util.ServiceConfigurationError;
import java.util.ServiceLoader;

/**
 * The generators a command can use, each by the id the catalog names it by.
 *
 * <p>A generator is someone else's code, and what it throws is its failure and not the command's.
 * So the command never holds one: it is handed a {@link Generator}, through which alone a
 * generator's code is called, and each call is told apart there ({@link GeneratorFailed#asking}).
 * Finding the generators runs their code too, since loading one makes it and asking its id calls
 * it, and a failure there is the installation's ({@link NotInstalled}).
 */
public final class Bindings {

    private final Map<String, BindingGenerator> byId;

    private Bindings(Map<String, BindingGenerator> byId) {
        this.byId = byId;
    }

    /** The generators on the class path. */
    static Bindings installed() throws NotInstalled {
        return of(ServiceLoader.load(BindingGenerator.class));
    }

    /** Exactly these generators, for a caller that has chosen them. */
    static Bindings of(Iterable<BindingGenerator> generators) throws NotInstalled {
        return found(generators);
    }

    /**
     * Each of {@code generators} by the id it answers to; one that does not load, or that throws
     * or answers nothing when asked its id, and two that answer to one id, refused.
     */
    private static Bindings found(Iterable<BindingGenerator> generators) throws NotInstalled {
        Map<String, BindingGenerator> byId = new HashMap<>();
        Iterator<BindingGenerator> each = generators.iterator();
        while (true) {
            BindingGenerator generator;
            String id;
            try {
                if (!each.hasNext()) {
                    return new Bindings(byId);
                }
                generator = each.next();
                id = generator.id();
            } catch (RuntimeException | LinkageError | ServiceConfigurationError e) {
                throw new NotInstalled("a generator installed with this command does not load: "
                        + e.getClass().getName()
                        + (e.getMessage() == null ? "" : ": " + e.getMessage()), e);
            }
            if (id == null) {
                throw new NotInstalled(generator.getClass().getName() + ", installed with this"
                        + " command, answers to no id", null);
            }
            BindingGenerator earlier = byId.putIfAbsent(id, generator);
            if (earlier != null) {
                throw new NotInstalled("two generators answer to \"" + id + "\": "
                        + earlier.getClass().getName() + " and "
                        + generator.getClass().getName(), null);
            }
        }
    }

    /**
     * The generator of {@code kind} that {@code jar} provides, which is then one of those found.
     *
     * <p>Loaded beside the compiler and not into it: it sees the API it is written against and what
     * the compiler is written against, and is found through its own service file. Loading it runs
     * its code, and what goes wrong there is its; a jar that provides no generator for the id it
     * was fetched for is not what the catalog said it was, which is the fetch's.
     */
    Generator load(Path jar, KnownBindings.Kind kind) throws IOException, GeneratorFailed {
        URLClassLoader loader = new URLClassLoader(new URL[] {jar.toUri().toURL()},
                Bindings.class.getClassLoader());
        BindingGenerator provided = GeneratorFailed.asking(kind, () -> {
            for (BindingGenerator generator : ServiceLoader.load(BindingGenerator.class, loader)) {
                if (kind.id().equals(generator.id())) {
                    return generator;
                }
            }
            return null;
        });
        if (provided == null) {
            loader.close();
            throw new NotFetched(jar + " provides no generator for \"" + kind.id() + "\", which "
                    + kind.artifact() + " is to");
        }
        byId.put(kind.id(), provided);
        return new Generator(kind, provided);
    }

    Generator generatorFor(KnownBindings.Kind kind) throws BindingUnavailable {
        BindingGenerator generator = byId.get(kind.id());
        if (generator == null) {
            throw new BindingUnavailable(kind);
        }
        return new Generator(kind, generator);
    }

    /**
     * A generator the command asks for {@code kind}: its code called only here, and what it throws
     * that is not a refusal said as its failure.
     */
    static final class Generator {

        private final KnownBindings.Kind kind;
        private final BindingGenerator generator;

        private Generator(KnownBindings.Kind kind, BindingGenerator generator) {
            this.kind = kind;
            this.generator = generator;
        }

        /** As {@link BindingGenerator#preflight}. */
        void preflight(Map<String, String> options) throws GeneratorFailed {
            GeneratorFailed.asking(kind, () -> {
                generator.preflight(options);
                return null;
            });
        }

        /** As {@link BindingGenerator#generate}. */
        void generate(BindingInput input, Path into, Map<String, String> options)
                throws GeneratorFailed {
            GeneratorFailed.asking(kind, () -> {
                generator.generate(input, into, options);
                return null;
            });
        }

        /** The name of the class that generates, as a catalog names what provides it. */
        String implementation() {
            return generator.getClass().getName();
        }
    }
}
