package souther.nativecode;

import org.jspecify.annotations.Nullable;
import souther.bindings.BindingApi;

import java.net.URI;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.HexFormat;
import java.util.Map;

/**
 * What a command may fetch, from where, and where it keeps what it has fetched.
 *
 * <p>What is fetched is a fact about one release: the generators are the artifacts of that version,
 * and the driver is the bundle of that version, so {@code version} is this compiler's own and there
 * is none where it is not a release. A build from a clone has no release to fetch from, and says so
 * rather than fetching whatever is newest.
 *
 * <p>The generators are read from Maven Central and can be read from a mirror, named by the property
 * {@code souther.maven.repository}. The bundles are read from the GitHub release of the version. What
 * is fetched from either is run, and neither can vouch for what it serves: an asset of a release can
 * be replaced, and a mirror is somebody else's. So every file is held to the SHA-256 in
 * {@code checksums} ({@link ReleaseChecksums}), which is written into the compiler's own artifact
 * when it is released, and one that does not match is refused.
 *
 * @param cache     where fetched files are kept, one directory a kind and a version
 * @param offline   whether nothing is to be fetched, so that only what is kept is used
 * @param version   the release this is, or null where this is not one
 * @param api       the release of the API this runs with, which a release is to be of too
 * @param checksums the SHA-256 of each file a release fetches, by {@link ReleaseChecksums}' keys
 */
record Fetching(Path cache, boolean offline, URI maven, URI releases, @Nullable String version,
                @Nullable String api, Map<String, String> checksums, Downloads downloads) {

    static final String MAVEN_PROPERTY = "souther.maven.repository";
    static final String RELEASES_PROPERTY = "souther.releases";
    static final String HOME_VARIABLE = "SOUTHER_HOME";

    /**
     * A release is the compiler and the API it was built with, of one version, and nothing is made of
     * one that is not: an API of another release on the class path beside it, which a build resolving
     * versions can put there, is refused here, before the command does anything, whichever way it then
     * goes.
     */
    Fetching {
        checksums = Map.copyOf(checksums);
        if (Release.is(version) && !version.equals(api)) {
            throw new IllegalArgumentException("souther-native-compiler " + version
                    + " runs with souther-bindings-api " + (api == null ? "of no release" : api)
                    + ": the two are to be of one release");
        }
    }

    /**
     * As the environment says: the cache in {@code $SOUTHER_HOME} or {@code ~/.souther}; refused where
     * the compiler and the API beside it are not of one release.
     */
    static Fetching standard() throws NotFetched {
        String home = System.getenv(HOME_VARIABLE);
        Path cache = home != null && !home.isBlank() ? Path.of(home)
                : Path.of(System.getProperty("user.home"), ".souther");
        try {
            return new Fetching(cache, false,
                    URI.create(System.getProperty(MAVEN_PROPERTY, "https://repo1.maven.org/maven2")),
                    URI.create(System.getProperty(RELEASES_PROPERTY,
                            "https://github.com/souther-lang/souther-native-compiler/releases/download")),
                    Main.class.getPackage().getImplementationVersion(),
                    BindingApi.class.getPackage().getImplementationVersion(),
                    ReleaseChecksums.carried(), Downloads.http());
        } catch (IllegalArgumentException mixed) {
            throw new NotFetched(mixed.getMessage(), mixed);
        }
    }

    Fetching withOffline(boolean offline) {
        return new Fetching(cache, offline, maven, releases, version, api, checksums, downloads);
    }

    /** As this, saying on {@code where} what it is about to fetch, since that takes a while. */
    Fetching saying(java.io.PrintStream where) {
        Downloads inner = downloads;
        return new Fetching(cache, offline, maven, releases, version, api, checksums, address -> {
            where.println("fetching " + address);
            return inner.get(address);
        });
    }

    /** The release this is, or why it is not one. */
    String release() throws NotFetched {
        if (!isRelease()) {
            throw new NotFetched("this is not a release" + (version == null ? "" : " (" + version + ")")
                    + ", so there is nothing published to fetch it from");
        }
        return version;
    }

    /** Whether this is a release. */
    boolean isRelease() {
        return Release.is(version);
    }

    static String sha256(byte[] bytes) {
        try {
            return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("every Java has SHA-256", e);
        }
    }
}
