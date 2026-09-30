package souther.nativecode;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;

import java.io.IOException;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.ServiceConfigurationError;
import java.util.ServiceLoader;

/**
 * How the command runs a generator: from a jar it was pointed at, in a fixed order, each step before
 * the next can run anything the last did not allow.
 *
 * <pre>
 * bytes verified → MANIFEST.MF (id, API major, ABI generations) → id rule
 *     → isolated loader → exactly one provider → constructed → preflight → generate
 * </pre>
 *
 * <p>Nothing the jar says of itself is read before its digest has matched
 * ({@link GeneratorArtifacts}), and none of its code runs before what it says has been checked
 * ({@link GeneratorDescriptor}). The providers its service file names are counted before any is
 * constructed, so a jar naming none or two runs nothing. Every generator, the catalog's and one
 * named on the command line alike, comes this one way.
 *
 * <p>A generator is someone else's code, and what it throws is its failure and not the command's.
 * So the command never holds one: it is handed a {@link Generator}, through which alone a
 * generator's code is called, each call told apart there ({@link GeneratorFailed#asking}) and made
 * with the generator's loader as the thread's context class loader, so that a library in the jar
 * looking up its services finds the jar's and not the compiler's.
 */
final class Bindings {

    private Bindings() {
    }

    /**
     * The generator {@code spec} names, verified, checked and constructed, or why not. What the load
     * makes on the way, the copy of the jar and the loader, is the load's until the generator is
     * made, and is let go of on every way out that does not make it; only a generator made is handed
     * them ({@link Holding}).
     */
    static Generator load(Fetching fetching, GeneratorSpec spec)
            throws IOException, GeneratorFailed {
        try (Holding held = new Holding()) {
            VerifiedJar jar = GeneratorArtifacts.materialize(held, fetching, spec.ref());
            GeneratorDescriptor descriptor = GeneratorDescriptor.read(jar);
            descriptor.check(spec.rule(), jar);
            String named = spec.rule() instanceof IdRule.Exactly ? spec.display()
                    : "the generator " + descriptor.id();
            GeneratorLoader loader = new GeneratorLoader(jar.jar());
            held.hold(loader, loader::close);
            // Found and none constructed or initialized, so none of the jar's code runs; a service
            // file naming a class that is not there, does not link, or is not a generator is a jar
            // that is not one.
            List<ServiceLoader.Provider<BindingGenerator>> providers;
            try {
                providers = ServiceLoader.load(BindingGenerator.class, loader).stream().toList();
            } catch (ServiceConfigurationError | LinkageError e) {
                throw new NotAGenerator(jar.ref() + " names a generator that does not load: "
                        + e.getMessage(), e);
            }
            // Counted before any is constructed, so a jar naming none or two runs nothing.
            if (providers.size() != 1) {
                throw new NotAGenerator(jar.ref() + " provides " + providers.size()
                        + " generators in META-INF/services/" + BindingGenerator.class.getName()
                        + ", and a generator's jar provides exactly one");
            }
            ServiceLoader.Provider<BindingGenerator> provider = providers.getFirst();
            BindingGenerator generator = GeneratorFailed.asking(named,
                    () -> within(loader, provider::get));
            Generator made = new Generator(named, descriptor, jar, loader, generator);
            held.handOver();
            return made;
        }
    }

    /** What {@code work} answers, run with {@code loader} as this thread's context class loader. */
    private static <T> T within(ClassLoader loader, GeneratorFailed.Work<T> work)
            throws IOException {
        Thread thread = Thread.currentThread();
        ClassLoader before = thread.getContextClassLoader();
        thread.setContextClassLoader(loader);
        try {
            return work.run();
        } finally {
            thread.setContextClassLoader(before);
        }
    }

    /**
     * A generator the command runs: its code called only here, and what it throws that is not a
     * refusal said as its failure. It owns its loader and its jar, and {@link #close} lets go of
     * both, deleting the jar where it was this command's own copy.
     */
    static final class Generator implements AutoCloseable {

        private final String named;
        private final GeneratorDescriptor descriptor;
        private final VerifiedJar jar;
        private final GeneratorLoader loader;
        private final BindingGenerator generator;

        private Generator(String named, GeneratorDescriptor descriptor, VerifiedJar jar,
                          GeneratorLoader loader, BindingGenerator generator) {
            this.named = named;
            this.descriptor = descriptor;
            this.jar = jar;
            this.loader = loader;
            this.generator = generator;
        }

        /** Which generator this is, as its jar says: what owns a directory it wrote. */
        String id() {
            return descriptor.id();
        }

        /** How the command names it. */
        String named() {
            return named;
        }

        /** What the mark of a binding it writes records of the jar that wrote it. */
        BindingDirectory.Artifact artifact() {
            return jar.artifact();
        }

        /** The loader its code resolves against. */
        ClassLoader loader() {
            return loader;
        }

        /** The jar it was loaded from, which is the copy that was hashed. */
        Path jar() {
            return jar.jar();
        }

        /** As {@link BindingGenerator#preflight}. */
        void preflight(Map<String, String> options) throws GeneratorFailed {
            GeneratorFailed.asking(named, () -> within(loader, () -> {
                generator.preflight(options);
                return null;
            }));
        }

        /** As {@link BindingGenerator#generate}. */
        void generate(BindingInput input, Path into, Map<String, String> options)
                throws GeneratorFailed {
            GeneratorFailed.asking(named, () -> within(loader, () -> {
                generator.generate(input, into, options);
                return null;
            }));
        }

        /** The name of the class that generates. */
        String implementation() {
            return generator.getClass().getName();
        }

        @Override
        public void close() throws IOException {
            try {
                loader.close();
            } finally {
                jar.discard();
            }
        }
    }
}
