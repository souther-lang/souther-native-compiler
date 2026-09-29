package souther.nativecode;

import org.jspecify.annotations.Nullable;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.net.URI;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.Map;
import java.util.Properties;

/**
 * What a command may fetch, from where, and where it keeps what it has fetched.
 *
 * <p>What is fetched is a fact about one release: the generators are the artifacts of that version,
 * and the driver is the bundle of that version, so {@code version} is this compiler's own and there
 * is none where it is not a release. A build from a clone has no release to fetch from, and says so
 * rather than fetching whatever is newest.
 *
 * <p>The generators are read from Maven Central and can be read from a mirror, named by the property
 * {@code souther.maven.repository}. The bundles are read from the GitHub release of the version, and
 * an asset there can be replaced, so it cannot vouch for itself: each bundle's SHA-256 is in
 * {@code checksums}, which is written into the compiler's own artifact when it is released, and a
 * bundle that does not match is refused.
 *
 * @param cache     where fetched files are kept, one directory a kind and a version
 * @param offline   whether nothing is to be fetched, so that only what is kept is used
 * @param version   the release this is, or null where this is not one
 * @param checksums the SHA-256 of each platform's bundle, by the platform's name
 */
record Fetching(Path cache, boolean offline, URI maven, URI releases, @Nullable String version,
                Map<String, String> checksums, Downloads downloads) {

    static final String MAVEN_PROPERTY = "souther.maven.repository";
    static final String RELEASES_PROPERTY = "souther.releases";
    static final String HOME_VARIABLE = "SOUTHER_HOME";
    private static final String CHECKSUMS = "/souther/nativecode/native-bundles.properties";

    Fetching {
        checksums = Map.copyOf(checksums);
    }

    /** As the environment says: the cache in {@code $SOUTHER_HOME} or {@code ~/.souther}. */
    static Fetching standard() {
        String home = System.getenv(HOME_VARIABLE);
        Path cache = home != null && !home.isBlank() ? Path.of(home)
                : Path.of(System.getProperty("user.home"), ".souther");
        return new Fetching(cache, false,
                URI.create(System.getProperty(MAVEN_PROPERTY, "https://repo1.maven.org/maven2")),
                URI.create(System.getProperty(RELEASES_PROPERTY,
                        "https://github.com/souther-lang/souther-native-compiler/releases/download")),
                Main.class.getPackage().getImplementationVersion(), released(), Downloads.http());
    }

    Fetching withOffline(boolean offline) {
        return new Fetching(cache, offline, maven, releases, version, checksums, downloads);
    }

    /** As this, saying on {@code where} what it is about to fetch, since that takes a while. */
    Fetching saying(java.io.PrintStream where) {
        Downloads inner = downloads;
        return new Fetching(cache, offline, maven, releases, version, checksums, address -> {
            where.println("fetching " + address);
            return inner.get(address);
        });
    }

    /** The release this is, or why it is not one. */
    String release() throws NotFetched {
        if (version == null || version.endsWith("-SNAPSHOT")) {
            throw new NotFetched("this is not a release" + (version == null ? "" : " (" + version + ")")
                    + ", so there is nothing published to fetch it from");
        }
        return version;
    }

    static String sha256(byte[] bytes) {
        try {
            return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("every Java has SHA-256", e);
        }
    }

    private static Map<String, String> released() {
        Map<String, String> read = new HashMap<>();
        try (InputStream in = Fetching.class.getResourceAsStream(CHECKSUMS)) {
            if (in != null) {
                Properties properties = new Properties();
                properties.load(in);
                properties.forEach((platform, sum) -> read.put(platform.toString(), sum.toString()));
            }
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        return read;
    }
}
