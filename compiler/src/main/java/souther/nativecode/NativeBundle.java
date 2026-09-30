package souther.nativecode;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.FileTime;
import java.time.Duration;
import java.time.Instant;
import java.util.Comparator;
import java.util.Locale;
import java.util.Set;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * The driver, with what it links beside it, as a release publishes it for one platform.
 *
 * <p>The driver reads the runtime archive and the file of what linking it needs from beside itself,
 * so what is fetched is the three together. The bundle is held to the SHA-256 this compiler was
 * released with, since a release asset is not something that cannot be replaced; and holds exactly
 * those three files, so that an archive that names a path outside its own directory cannot write
 * there.
 *
 * <p>What is kept is the bundle as it was fetched, and nothing made from it: {@code
 * native/<version>/souther-native-<version>-<platform>.zip}, read and held to its checksum again
 * each time it is used, and fetched again, or refused offline, where it no longer matches. What runs
 * is never the kept file. The three files are unpacked from the bytes that were just hashed into a
 * directory this process made for itself, under {@code run/}, and deleted when the process ends, so
 * that what runs is what was hashed, as it is for a generator's jar ({@link GeneratorArtifacts}). A
 * directory left there by a process that did not end cleanly is deleted a day later.
 */
final class NativeBundle {

    static final String DRIVER = "souther-native-driver";
    static final String ARCHIVE = "libsouther_native_runtime.a";
    static final String REQUIREMENTS = "libsouther_native_runtime.link";
    private static final Set<String> FILES = Set.of(DRIVER, ARCHIVE, REQUIREMENTS);

    /** How long a directory a process unpacked into is left before another deletes it. */
    private static final Duration LEFT = Duration.ofDays(1);

    /** The directories this process unpacked into, deleted when it ends. */
    private static final Set<Path> UNPACKED = java.util.concurrent.ConcurrentHashMap.newKeySet();

    static {
        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            for (Path directory : UNPACKED) {
                try {
                    delete(directory);
                } catch (IOException ignored) {
                    // Left for the next process to delete, a day on.
                }
            }
        }, "souther-native-bundle-cleanup"));
    }

    /**
     * The platforms a release publishes a bundle for, which the release workflow builds and
     * {@code scripts/package-native-bundle.sh} accepts, by the names a bundle carries.
     */
    static final Set<String> PLATFORMS = Set.of("linux-x86_64", "linux-aarch64", "macos-x86_64",
            "macos-aarch64");

    private NativeBundle() {
    }

    /** The name of the platform this runs on, as a bundle is named for it. */
    static String platform() throws NotFetched {
        return platform(System.getProperty("os.name"), System.getProperty("os.arch"));
    }

    static String platform(String os, String arch) throws NotFetched {
        String system = os.toLowerCase(Locale.ROOT).contains("mac") ? "macos"
                : os.toLowerCase(Locale.ROOT).contains("linux") ? "linux" : null;
        String machine = switch (arch.toLowerCase(Locale.ROOT)) {
            case "aarch64", "arm64" -> "aarch64";
            case "amd64", "x86_64" -> "x86_64";
            default -> null;
        };
        if (system == null || machine == null) {
            throw new NotFetched("there is no driver published for " + os + " on " + arch);
        }
        return system + "-" + machine;
    }

    /**
     * The driver for this platform, in a directory of this process's own unpacked from the kept
     * bundle, which is fetched first where it is not kept or no longer matches.
     */
    static Path locate(Fetching fetching) throws NotFetched {
        String version = fetching.release();
        String platform = platform();
        String expected = fetching.checksums().get(ReleaseChecksums.bundle(platform));
        if (expected == null) {
            throw new NotFetched("this compiler was released with no checksum for the driver for "
                    + platform + ", so it will not take one");
        }
        String name = "souther-native-" + version + "-" + platform + ".zip";
        Path kept = fetching.cache().resolve("native").resolve(version).resolve(name);
        byte[] bytes = kept(kept, expected);
        if (bytes == null) {
            if (fetching.offline()) {
                throw new NotFetched("the driver for " + platform + " " + version
                        + " is not kept, and this is offline");
            }
            bytes = fetched(fetching, kept, expected,
                    URI.create(fetching.releases().toString().replaceAll("/+$", "") + "/v" + version
                            + "/" + name));
        }
        try (Holding held = new Holding()) {
            Path run = Files.createDirectories(fetching.cache().resolve("run"));
            leftBefore(run, Instant.now().minus(LEFT));
            Path directory = Files.createTempDirectory(run, "native-");
            held.hold(directory, () -> delete(directory));
            unpack(bytes, directory);
            UNPACKED.add(directory);
            held.handOver();
            return directory.resolve(DRIVER);
        } catch (NotFetched e) {
            throw e;
        } catch (IOException e) {
            throw new NotFetched("could not unpack the driver for " + platform + ": "
                    + e.getMessage(), e);
        }
    }

    /**
     * The bytes of the bundle kept at {@code kept}, where they still match {@code expected}; null
     * where none is kept, and where one that no longer matches was, which is deleted.
     */
    private static byte[] kept(Path kept, String expected) throws NotFetched {
        try {
            if (!Files.isRegularFile(kept)) {
                return null;
            }
            byte[] bytes = Files.readAllBytes(kept);
            if (Fetching.sha256(bytes).equals(expected)) {
                return bytes;
            }
            // Not what was fetched: never run, and asked for again.
            Files.delete(kept);
            return null;
        } catch (IOException e) {
            throw new NotFetched("could not read the kept " + kept + ": " + e.getMessage(), e);
        }
    }

    /** The bundle at {@code bundle}, held to {@code expected}, and kept at {@code kept}. */
    private static byte[] fetched(Fetching fetching, Path kept, String expected, URI bundle)
            throws NotFetched {
        byte[] bytes;
        try {
            bytes = fetching.downloads().get(bundle);
        } catch (IOException e) {
            throw new NotFetched("could not fetch " + bundle + ": " + e.getMessage(), e);
        }
        if (!Fetching.sha256(bytes).equals(expected)) {
            throw new NotFetched(bundle + " does not match the checksum this compiler was released"
                    + " with: refused, and not kept");
        }
        // A bundle holding what it is not to is refused and not kept.
        try {
            files(bytes);
        } catch (NotFetched e) {
            throw e;
        } catch (IOException e) {
            throw new NotFetched("could not read " + bundle + ": " + e.getMessage(), e);
        }
        try {
            Files.createDirectories(kept.getParent());
            Path partial = Files.createTempFile(kept.getParent(), kept.getFileName().toString(),
                    ".partial");
            try (Holding held = new Holding()) {
                held.hold(partial, () -> Files.deleteIfExists(partial));
                Files.write(partial, bytes);
                Files.move(partial, kept, StandardCopyOption.ATOMIC_MOVE,
                        StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (IOException e) {
            throw new NotFetched("could not keep " + bundle + ": " + e.getMessage(), e);
        }
        return bytes;
    }

    /** Deletes what a process unpacked into {@code run} and left there before {@code before}. */
    private static void leftBefore(Path run, Instant before) {
        try (Stream<Path> left = Files.list(run)) {
            for (Path directory : left.toList()) {
                try {
                    FileTime modified = Files.getLastModifiedTime(directory);
                    if (modified.toInstant().isBefore(before)) {
                        delete(directory);
                    }
                } catch (IOException ignored) {
                    // Another process deleting it too, or one that cannot be: tried again next time.
                }
            }
        } catch (IOException ignored) {
            // Nothing to delete that can be listed.
        }
    }

    /**
     * The three files {@code bundle} holds, by name, or why it is not a bundle: an entry that is not
     * one of them, one of them twice, or one missing.
     */
    private static java.util.Map<String, byte[]> files(byte[] bundle) throws IOException {
        java.util.Map<String, byte[]> files = new java.util.HashMap<>();
        try (ZipInputStream zip = new ZipInputStream(new ByteArrayInputStream(bundle))) {
            for (ZipEntry entry = zip.getNextEntry(); entry != null; entry = zip.getNextEntry()) {
                String name = entry.getName();
                if (entry.isDirectory() || !FILES.contains(name) || files.containsKey(name)) {
                    throw new NotFetched("the bundle holds \"" + name + "\", which is not one of "
                            + "the three files it is to hold, once each: refused");
                }
                files.put(name, zip.readAllBytes());
            }
        }
        if (!files.keySet().equals(FILES)) {
            throw new NotFetched("the bundle holds " + files.keySet() + ", and is to hold " + FILES);
        }
        return files;
    }

    private static void unpack(byte[] bundle, Path into) throws IOException {
        for (java.util.Map.Entry<String, byte[]> file : files(bundle).entrySet()) {
            Files.write(into.resolve(file.getKey()), file.getValue());
        }
        if (!into.resolve(DRIVER).toFile().setExecutable(true, false)) {
            throw new NotFetched("the driver cannot be made executable in " + into);
        }
    }

    private static void delete(Path directory) throws IOException {
        if (!Files.exists(directory)) {
            return;
        }
        try (Stream<Path> walked = Files.walk(directory)) {
            for (Path each : walked.sorted(Comparator.reverseOrder()).toList()) {
                Files.deleteIfExists(each);
            }
        }
    }
}
