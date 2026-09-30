package souther.nativecode;

import java.util.regex.Pattern;

/**
 * A jar in a Maven repository, by {@code groupId:artifactId:version}: where it came from, and never
 * what it is. Other bytes can be served under the same coordinate, so a coordinate is always named
 * with the SHA-256 of the jar ({@link GeneratorRef.Maven}).
 */
record MavenCoordinate(String group, String artifact, String version) {

    private static final Pattern PART = Pattern.compile("[A-Za-z0-9][A-Za-z0-9._-]*");

    MavenCoordinate {
        for (String part : new String[] {group, artifact, version}) {
            if (!PART.matcher(part).matches()) {
                throw new IllegalArgumentException("not a part of a Maven coordinate: \"" + part + "\"");
            }
        }
    }

    /** {@code text} as {@code groupId:artifactId:version}, or why it is not one. */
    static MavenCoordinate parse(String text) {
        String[] parts = text.split(":", -1);
        if (parts.length != 3) {
            throw new IllegalArgumentException("\"" + text + "\" is not groupId:artifactId:version");
        }
        return new MavenCoordinate(parts[0], parts[1], parts[2]);
    }

    /** The name of the jar in the repository. */
    String file() {
        return artifact + "-" + version + ".jar";
    }

    /** Where the jar is under a repository's root, as Maven lays one out. */
    String path() {
        return group.replace('.', '/') + "/" + artifact + "/" + version + "/" + file();
    }

    @Override
    public String toString() {
        return group + ":" + artifact + ":" + version;
    }
}
