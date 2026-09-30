package souther.nativecode;

import org.jspecify.annotations.Nullable;

/**
 * Whether a version is a release: the one statement of it, which everything that behaves otherwise
 * for a release asks. A build from a clone has no version, or a snapshot's, and has nothing published
 * to fetch from; a release has its checksums and fetches what they are of.
 */
public final class Release {

    private Release() {
    }

    /** Whether {@code version} is that of a release. */
    public static boolean is(@Nullable String version) {
        return version != null && !version.endsWith("-SNAPSHOT");
    }
}
