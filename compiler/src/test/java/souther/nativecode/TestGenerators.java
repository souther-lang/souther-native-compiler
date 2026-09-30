package souther.nativecode;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.NotBindable;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.TreeMap;

/**
 * Generators a test packs into a jar ({@link GeneratorJar}) and has the command load, as it loads
 * any: each is loaded by a loader of its own, so what it says goes where every loader reaches, the
 * system properties, under {@value #ASKED}.
 */
public final class TestGenerators {

    /** Where the generators say what they were asked, a line each. */
    public static final String ASKED = "souther.test.asked";

    private TestGenerators() {
    }

    static void said(String line) {
        System.setProperty(ASKED, System.getProperty(ASKED, "") + line + "\n");
    }

    /** Writes one file, and says what it was asked. */
    public static class Recording implements BindingGenerator {

        @Override
        public void preflight(Map<String, String> options) {
            said("preflight " + new TreeMap<>(options));
        }

        @Override
        public void generate(BindingInput input, Path into, Map<String, String> options)
                throws IOException {
            boolean empty;
            try (var held = Files.list(into)) {
                empty = held.findAny().isEmpty();
            }
            said("generate " + new TreeMap<>(options) + " of " + input.manifest().modules().size()
                    + " module into " + (empty ? "an empty" : "a") + " directory");
            Files.writeString(into.resolve("generated.txt"), "written by " + getClass().getSimpleName());
        }
    }

    /** Refuses before anything is built. */
    public static final class RefusingAhead extends Recording {
        @Override
        public void preflight(Map<String, String> options) {
            super.preflight(options);
            throw new NotBindable("not that namespace");
        }
    }

    /** Refuses a name only the model says. */
    public static final class RefusingLate extends Recording {
        @Override
        public void generate(BindingInput input, Path into, Map<String, String> options) {
            throw new NotBindable("no such crate");
        }
    }

    /** Fails with a bug of its own. */
    public static final class Failing extends Recording {
        @Override
        public void generate(BindingInput input, Path into, Map<String, String> options) {
            throw new IllegalStateException("the generator's own bug");
        }
    }

    /** Fails with an I/O failure of its own, the type the command's own are. */
    public static final class FailingOnItsDisk extends Recording {
        @Override
        public void generate(BindingInput input, Path into, Map<String, String> options)
                throws IOException {
            throw new IOException("the generator's own disk");
        }
    }

    /** Cannot be constructed. */
    public static final class FailingToBeMade extends Recording {
        public FailingToBeMade() {
            throw new IllegalStateException("the generator's own constructor");
        }
    }

    /** Throws an {@link Error} of its own where it is asked to preflight. */
    public static final class ErringAhead extends Recording {
        @Override
        public void preflight(Map<String, String> options) {
            throw new AssertionError("the generator's own assertion, ahead");
        }
    }

    /** Throws an {@link Error} of its own where it is asked to generate. */
    public static final class Erring extends Recording {
        @Override
        public void generate(BindingInput input, Path into, Map<String, String> options) {
            throw new AssertionError("the generator's own assertion");
        }
    }

    /** Throws an {@link Error} of its own where it is made. */
    public static final class ErringToBeMade extends Recording {
        public ErringToBeMade() {
            throw new AssertionError("the generator's own assertion, made");
        }
    }

    /** Says when its class is initialized and when it is made: neither is to happen before its checks. */
    public static final class Watched extends Recording {
        static {
            said("initialized " + Watched.class.getSimpleName());
        }

        public Watched() {
            said("constructed " + Watched.class.getSimpleName());
        }
    }

    /** A second provider beside {@link Watched}, which says when it is made. */
    public static final class AlsoWatched extends Recording {
        static {
            said("initialized " + AlsoWatched.class.getSimpleName());
        }

        public AlsoWatched() {
            said("constructed " + AlsoWatched.class.getSimpleName());
        }
    }

    /** Says what it can resolve, and what the thread it is called on resolves against. */
    public static final class Looking extends Recording {
        @Override
        public void preflight(Map<String, String> options) {
            ClassLoader own = getClass().getClassLoader();
            said("context is own " + (Thread.currentThread().getContextClassLoader() == own));
            said("api is own " + (BindingGenerator.class.getClassLoader() == own));
            try {
                Class.forName("souther.nativecode.Main", false, own);
                said("compiler resolves");
            } catch (ClassNotFoundException e) {
                said("compiler does not resolve");
            }
        }
    }
}
