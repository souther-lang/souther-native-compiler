package souther.nativecode;

import java.nio.file.Path;

/**
 * A generator as the command is to run it, whichever way it was asked for: the jar, and which
 * generator the jar may say it is. After this, nothing tells a binding of the catalog apart from one
 * named on the command line.
 *
 * @param ref     the jar
 * @param rule    which generator the jar may say it is
 * @param display how the command names the generator before its jar has said which it is
 */
record GeneratorSpec(GeneratorRef ref, IdRule rule, String display) {

    /**
     * The property naming the jar of the {@code <id>} generator in a build that is not a release: a
     * clone's launcher names the jars its reactor built. A release never reads it, and fetches the jar
     * its catalog names at its own version, held to the checksum it was released with.
     */
    static final String DEVELOPMENT_PROPERTY = "souther.generator.";

    /** What {@code asked} is run as, in the build {@code fetching} is of. */
    static GeneratorSpec of(Main.Asked asked, Fetching fetching) throws NotFetched {
        return switch (asked) {
            case Main.Asked.Standard standard -> standard(standard.kind(), fetching);
            case Main.Asked.External external -> new GeneratorSpec(external.ref(),
                    new IdRule.External(), "the generator " + external.ref());
        };
    }

    /**
     * The generator of {@code kind}: in a release, the artifact the catalog names at this compiler's
     * version, held to the SHA-256 the release carries for it; in a build that is not one, the jar
     * named by {@value #DEVELOPMENT_PROPERTY}{@code <id>}.
     */
    static GeneratorSpec standard(KnownBindings.Kind kind, Fetching fetching) throws NotFetched {
        IdRule rule = new IdRule.Exactly(kind.id());
        String display = "the " + kind.display() + " generator";
        String version = fetching.version();
        if (version == null || version.endsWith("-SNAPSHOT")) {
            String named = System.getProperty(DEVELOPMENT_PROPERTY + kind.id());
            if (named == null) {
                throw new NotFetched(kind.flag() + " needs " + display + ", and this is not a release"
                        + (version == null ? "" : " (" + version + ")") + ", so there is none to"
                        + " fetch: a clone names the jar its build made with -D"
                        + DEVELOPMENT_PROPERTY + kind.id() + ", as scripts/souther-native does");
            }
            return new GeneratorSpec(new GeneratorRef.Local(Path.of(named)), rule, display);
        }
        String sha256 = fetching.checksums().get(ReleaseChecksums.generator(kind.id()));
        if (sha256 == null) {
            throw new NotFetched("this compiler was released with no checksum for " + display
                    + ", so it will not take one");
        }
        MavenCoordinate coordinate = MavenCoordinate.parse(kind.artifact() + ":" + version);
        return new GeneratorSpec(new GeneratorRef.Maven(coordinate, sha256), rule, display);
    }
}
