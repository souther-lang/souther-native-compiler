package souther.bindings.testkit;

import souther.nativecode.NativeCompiler;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Where the driver the testkit builds with comes from, decided by the testkit's own version and by
 * nothing a user sets.
 *
 * <p>A released testkit fetches the driver of its release and holds it to the checksum the compiler
 * of that release carries, as the command does; it reads no property. A testkit that is not a release
 * has no release to fetch from, and is built and run in this repository's reactor, which hands it the
 * driver the same build made, as it hands every test: by {@value NativeCompiler#DRIVER_PROPERTY}.
 */
final class NativeToolchain {

    /** The released driver, unpacked for this process once and used by every library it builds. */
    private static Path released;

    private NativeToolchain() {
    }

    static Path driver() throws IOException {
        String version = NativeToolchain.class.getPackage().getImplementationVersion();
        if (version != null && !version.endsWith("-SNAPSHOT")) {
            synchronized (NativeToolchain.class) {
                if (released == null) {
                    released = NativeCompiler.releasedDriver();
                }
                return released;
            }
        }
        String named = System.getProperty(NativeCompiler.DRIVER_PROPERTY);
        if (named == null || !Files.isExecutable(Path.of(named))) {
            throw new IOException("this testkit is not a release" + (version == null ? ""
                    : " (" + version + ")") + ", and is run by the build that made it, which names"
                    + " the driver it built with " + NativeCompiler.DRIVER_PROPERTY + "; outside that"
                    + " build, depend on a released testkit");
        }
        return Path.of(named);
    }
}
