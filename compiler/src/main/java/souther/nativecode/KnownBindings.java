package souther.nativecode;

import java.util.List;
import java.util.Optional;

/**
 * The bindings this compiler writes, and what each is asked for with on the command line.
 *
 * <p>The one place that says there is a PHP, a Rust and a Go binding: which ids {@code --binding}
 * reads as one of them, which option keys each takes, what the command's usage says, which artifact a
 * release fetches for each, and which ids a jar named with {@code --binding} may not say it is. It is
 * closed, since what an id of the catalog fetches and runs is only ever what this names, and it is
 * not derived from the generators found, because a generator that is not there cannot say that it is
 * missing. A generator of anyone else's is named on the command line with its digest or its path, and
 * is not this catalog's.
 *
 * <p>It says how a binding is asked for and nothing of what the asking means. What a namespace, a
 * crate or a package is, and whether one is acceptable, is the generator's to say.
 */
public final class KnownBindings {

    /**
     * One binding: the id its generator's jar says it is, which is also what {@code --binding} names it
     * by, and the keys of the {@code --binding-option}s that qualify it.
     *
     * @param artifact {@code groupId:artifactId} of the generator, whose version is this
     *                 compiler's own
     */
    public record Kind(String id, String display, List<String> options, String artifact) {

        public Kind {
            options = List.copyOf(options);
        }
    }

    private static final List<Kind> ALL = List.of(
            new Kind("php", "PHP", List.of("namespace"), "org.souther-lang:souther-binding-php"),
            new Kind("rust", "Rust", List.of("crate"), "org.souther-lang:souther-binding-rust"),
            new Kind("go", "Go", List.of("package"), "org.souther-lang:souther-binding-go"));

    private KnownBindings() {
    }

    /** Every binding, in the order the command's usage lists them. */
    public static List<Kind> all() {
        return ALL;
    }

    /** The binding whose id is {@code id}, if there is one. */
    static Optional<Kind> named(String id) {
        return ALL.stream().filter(kind -> kind.id().equals(id)).findFirst();
    }
}
