package souther.bindings.testkit;

import souther.nativecode.NativeCompiler;
import souther.nativecode.Release;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Where the driver the testkit builds with comes from, decided by the testkit's own version and by
 * nothing a user sets.
 *
 * <p>A released testkit fetches the driver of its release and holds it to the checksum the compiler
 * of that release carries, as the command does; it reads no property. It hands the compiler its own
 * version, and the compiler refuses to be of another release, or to run with an API of one: the
 * three come as one release, and a build resolving versions could otherwise mix them. A testkit that is not a release
 * has no release to fetch from, and is built and run in this repository's reactor, which hands it the
 * driver the same build made, as it hands every test: by {@value NativeCompiler#DRIVER_PROPERTY}.
 */
final class NativeToolchain {

    /**
     * The released driver, unpacked once for this process and used by every library it builds. Its
     * owner is the process: it is closed, and so deleted, when the process ends.
     */
    private static NativeCompiler.ReleasedDriver released;

    private NativeToolchain() {
    }

    static Path driver() throws IOException {
        String version = NativeToolchain.class.getPackage().getImplementationVersion();
        if (Release.is(version)) {
            synchronized (NativeToolchain.class) {
                if (released == null) {
                    NativeCompiler.ReleasedDriver driver = NativeCompiler.releasedDriver(version);
                    Runtime.getRuntime().addShutdownHook(new Thread(() -> {
                        try {
                            driver.close();
                        } catch (IOException ignored) {
                            // Its lock goes with the process, and the next use deletes it.
                        }
                    }, "souther-testkit-driver"));
                    released = driver;
                }
                return released.path();
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
