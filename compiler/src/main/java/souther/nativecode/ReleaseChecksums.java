package souther.nativecode;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;
import java.util.Properties;
import java.util.Set;
import java.util.TreeSet;
import java.util.regex.Pattern;

/**
 * What a release writes into the compiler's own artifact: the SHA-256 of everything the compiler
 * fetches when it runs, by what it is. The driver of each platform, and the generator of each binding
 * the catalog names.
 *
 * <p>These are the only place a fetched file is vouched for. What a file is fetched from cannot vouch
 * for it: a GitHub release asset can be replaced, and a Maven repository can be a mirror, which is
 * to say somebody else's. The compiler on Maven Central cannot be changed once published, so what it
 * carries is what the release built.
 *
 * <p>The file is made by the release, from the files the release built, and is not committed: it
 * cannot be, since it is a fact about builds that come after the commit. So a build of a release
 * version without it is refused here, at build time, by {@link #main}: a compiler that carries none
 * would be published and would only say so to a user who needs a driver.
 */
public final class ReleaseChecksums {

    /** Where the file is in the compiler's artifact, and where the release writes it. */
    public static final String RESOURCE = "souther/nativecode/release-checksums.properties";

    private static final Pattern SHA256 = Pattern.compile("[0-9a-f]{64}");

    private ReleaseChecksums() {
    }

    /** The key of the bundle of {@code platform}. */
    static String bundle(String platform) {
        return "native." + platform;
    }

    /** The key of the generator of the binding named {@code id}. */
    static String generator(String id) {
        return "generator." + id;
    }

    /** What a release is to carry a checksum of, by key: every platform's bundle, every generator. */
    static Set<String> expected() {
        Set<String> keys = new TreeSet<>();
        NativeBundle.PLATFORMS.forEach(platform -> keys.add(bundle(platform)));
        KnownBindings.all().forEach(kind -> keys.add(generator(kind.id())));
        return keys;
    }

    /** What this compiler carries, none where it is not a release. */
    static Map<String, String> carried() {
        try (InputStream in = ReleaseChecksums.class.getClassLoader().getResourceAsStream(RESOURCE)) {
            return in == null ? Map.of() : read(in);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static Map<String, String> read(InputStream in) throws IOException {
        Properties properties = new Properties();
        properties.load(in);
        Map<String, String> read = new HashMap<>();
        properties.forEach((key, sum) -> read.put(key.toString(), sum.toString()));
        return read;
    }

    /**
     * Refuses a release version whose checksums are not all there: the file is missing, it lacks one of
     * {@link #expected}, it holds one that is not expected, or a value is not a SHA-256. A version that
     * is a snapshot is not a release, and is not held to it.
     */
    static void check(String version, Path file) throws IOException {
        if (version.endsWith("-SNAPSHOT")) {
            return;
        }
        Map<String, String> found;
        try (InputStream in = Files.newInputStream(file)) {
            found = read(in);
        } catch (NoSuchFileException e) {
            throw new IOException(version + " is a release, and carries no checksums: " + file
                    + " is written by the release workflow from what it built, and is not in a"
                    + " clone", e);
        }
        Set<String> lacking = new TreeSet<>(expected());
        lacking.removeAll(found.keySet());
        Set<String> unexpected = new TreeSet<>(found.keySet());
        unexpected.removeAll(expected());
        Set<String> malformed = new TreeSet<>();
        found.forEach((key, sum) -> {
            if (!SHA256.matcher(sum).matches()) {
                malformed.add(key);
            }
        });
        if (!lacking.isEmpty() || !unexpected.isEmpty() || !malformed.isEmpty()) {
            throw new IOException(version + " is a release, and its checksums in " + file
                    + " are not the ones it is to carry: lacking " + lacking + ", unexpected "
                    + unexpected + ", not a SHA-256 " + malformed);
        }
    }

    /**
     * The build's check of a release: {@code <version> <file>}. A release refused fails the build
     * that asked, through the exception this ends with.
     */
    public static void main(String[] args) {
        try {
            check(args[0], Path.of(args[1]));
        } catch (IOException e) {
            throw new IllegalStateException(e.getMessage(), e);
        }
    }
}
