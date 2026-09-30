package souther.nativecode;

import java.nio.file.Path;
import java.util.regex.Pattern;

/**
 * The jar a generator is, as it was pointed at: a coordinate in a Maven repository with the SHA-256
 * the jar is held to, or a jar on this machine.
 *
 * <p>Which bytes are run is decided by the digest alone. A coordinate says where to fetch them from
 * and a path where to read them from, and nothing either serves is believed about itself.
 */
sealed interface GeneratorRef {

    Pattern SHA256 = Pattern.compile("[0-9a-f]{64}");

    /** The jar at {@code coordinate}, which is to be the one whose SHA-256 is {@code sha256}. */
    record Maven(MavenCoordinate coordinate, String sha256) implements GeneratorRef {

        public Maven {
            if (!SHA256.matcher(sha256).matches()) {
                throw new IllegalArgumentException("not a SHA-256 in lowercase hex: \"" + sha256 + "\"");
            }
        }

        @Override
        public String toString() {
            return coordinate + "@sha256:" + sha256;
        }
    }

    /** The jar at {@code path}, for a generator's author's own build: no digest asked, one computed. */
    record Local(Path path) implements GeneratorRef {

        @Override
        public String toString() {
            return path.toString();
        }
    }

    /** A coordinate and the digest of its jar, as {@code --binding} names one. */
    Pattern COORDINATE = Pattern.compile("([^:@/\\\\]+):([^:@/\\\\]+):([^:@/\\\\]+)@(.*)");

    /**
     * What {@code --binding} names, which is one of two forms that cannot be taken for each other: a
     * path to a jar, which ends in {@code .jar}, whatever else it holds; or
     * {@code groupId:artifactId:version@sha256:<hex>}, which cannot end so. Anything else is refused,
     * a coordinate without its digest among it, since nothing is trusted on first use.
     */
    static GeneratorRef parse(String named) {
        if (named.endsWith(".jar")) {
            return new Local(Path.of(named));
        }
        java.util.regex.Matcher coordinate = COORDINATE.matcher(named);
        if (coordinate.matches()) {
            String digest = coordinate.group(4);
            if (!digest.startsWith("sha256:")) {
                throw new IllegalArgumentException("\"" + named + "\" names a digest that is not"
                        + " sha256:<hex>");
            }
            return new Maven(new MavenCoordinate(coordinate.group(1), coordinate.group(2),
                    coordinate.group(3)), digest.substring("sha256:".length()));
        }
        if (named.split(":", -1).length == 3) {
            throw new IllegalArgumentException("\"" + named + "\" is a coordinate with no digest:"
                    + " a generator is named as groupId:artifactId:version@sha256:<hex>, and"
                    + " nothing is trusted on first use");
        }
        throw new IllegalArgumentException("\"" + named + "\" is neither a path to a jar, which ends"
                + " in .jar, nor groupId:artifactId:version@sha256:<hex>");
    }
}
