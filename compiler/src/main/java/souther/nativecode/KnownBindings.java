package souther.nativecode;

import java.util.List;
import java.util.Optional;

/**
 * The bindings this compiler writes, and what each is asked for on the command line.
 *
 * <p>The one place that says there is a PHP, a Rust and a Go binding: what the command reads its
 * options from, what its usage says, and, where a generator is not among those installed, which
 * artifact would bring it. It is closed, since what is fetched and run on the strength of a flag is
 * only ever what this names, and it is not derived from the generators found, because a generator
 * that is not there cannot say that it is missing.
 *
 * <p>It says how a binding is asked for and nothing of what the asking means. What a namespace, a
 * crate or a package is, and whether one is acceptable, is the generator's to say.
 */
public final class KnownBindings {

    /**
     * One binding: the id its generator answers to, the option that asks for it (whose value is the
     * directory it is written into), and the options that qualify it, each named without dashes.
     *
     * @param artifact {@code groupId:artifactId} of the generator, whose version is this
     *                 compiler's own
     */
    public record Kind(String id, String flag, String display, List<String> options,
                       String artifact) {

        public Kind {
            options = List.copyOf(options);
        }
    }

    private static final List<Kind> ALL = List.of(
            new Kind("php", "--php", "PHP", List.of("namespace"),
                    "org.souther-lang:souther-binding-php"),
            new Kind("rust", "--rust", "Rust", List.of("crate"),
                    "org.souther-lang:souther-binding-rust"),
            new Kind("go", "--go", "Go", List.of("package"),
                    "org.souther-lang:souther-binding-go"));

    private KnownBindings() {
    }

    /** Every binding, in the order a command writes them. */
    public static List<Kind> all() {
        return ALL;
    }

    /** The binding that {@code option} asks for, if it is the option that does. */
    static Optional<Kind> askedBy(String option) {
        return ALL.stream().filter(kind -> kind.flag().equals(option)).findFirst();
    }

    /** The binding that {@code option} qualifies, if it qualifies one. */
    static Optional<Kind> qualifiedBy(String option) {
        return ALL.stream()
                .filter(kind -> kind.options().stream().anyMatch(it -> ("--" + it).equals(option)))
                .findFirst();
    }
}
