package souther.nativecode;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.channels.OverlappingFileLockException;
import java.nio.file.StandardOpenOption;
import java.util.List;
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
 * directory of the use's own, under {@code run/}, so that what runs is what was hashed, as it is for
 * a generator's jar ({@link GeneratorArtifacts}). The directory has an owner from the moment it is
 * made ({@link Holding}), which locks it while it is in use and deletes it when done. One left by a
 * process that did not end cleanly is deleted by another once its lock can be taken, which is only
 * once nothing holds it.
 */
final class NativeBundle {

    static final String DRIVER = "souther-native-driver";
    static final String ARCHIVE = "libsouther_native_runtime.a";
    static final String REQUIREMENTS = "libsouther_native_runtime.link";
    private static final Set<String> FILES = Set.of(DRIVER, ARCHIVE, REQUIREMENTS);

    /** The file each directory a use unpacks into holds, locked for as long as it is in use. */
    static final String LOCK = ".in-use";

    /**
     * How old a directory has to be before another process asks whether its user is gone: long enough
     * that a directory is locked before anyone asks, between its making and its lock.
     */
    private static final Duration SETTLED = Duration.ofMinutes(10);

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
     * The bundle of this release for this platform, kept, and fetched first where it is not kept or
     * no longer matches: what {@code --fetch} prepares, which is the cache and nothing that runs.
     */
    static Path kept(Fetching fetching) throws NotFetched {
        return verified(fetching).kept();
    }

    /**
     * The driver for this platform, unpacked from the bytes of the kept bundle just held to their
     * checksum into a directory of this use's own, which {@code held} owns: it holds the directory's
     * lock while it is in use, and deletes the directory when it lets go. There is no driver without
     * an owner.
     */
    static Path unpacked(Holding held, Fetching fetching) throws NotFetched {
        Verified bundle = verified(fetching);
        try {
            Path run = Files.createDirectories(fetching.cache().resolve("run"));
            abandonedIn(run);
            Path directory = Files.createTempDirectory(run, "native-");
            held.hold(directory, () -> delete(directory));
            FileChannel channel = FileChannel.open(directory.resolve(LOCK),
                    StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
            held.hold(channel, channel::close);
            FileLock lock = channel.lock();
            // Deleted while it is still locked, so that no other process takes it between the two.
            held.hold(lock, () -> {
                try {
                    delete(directory);
                } finally {
                    lock.release();
                }
            });
            unpack(bundle.bytes(), directory);
            return directory.resolve(DRIVER);
        } catch (NotFetched e) {
            throw e;
        } catch (IOException e) {
            throw new NotFetched("could not unpack the driver: " + e.getMessage(), e);
        }
    }

    /** The kept bundle and its bytes, held to the checksum just now. */
    private record Verified(Path kept, byte[] bytes) {
    }

    private static Verified verified(Fetching fetching) throws NotFetched {
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
        return new Verified(kept, bytes);
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

    /**
     * Deletes what a use unpacked into {@code run} and whose user is gone. A directory's user holds
     * its lock for as long as it uses it, whatever its age, so a directory is deleted only where its
     * lock can be taken, which is where no process holds it, and only once it is old enough to have
     * been locked. Its age alone says nothing of whether a process that runs for days still runs it.
     */
    private static void abandonedIn(Path run) {
        Instant settled = Instant.now().minus(SETTLED);
        List<Path> directories;
        try (Stream<Path> listed = Files.list(run)) {
            directories = listed.toList();
        } catch (IOException e) {
            return;
        }
        for (Path directory : directories) {
            try {
                if (Files.getLastModifiedTime(directory).toInstant().isAfter(settled)) {
                    continue;
                }
                Path lock = directory.resolve(LOCK);
                if (!Files.exists(lock)) {
                    // Made and never locked by a process that ended between the two.
                    delete(directory);
                    continue;
                }
                try (FileChannel channel = FileChannel.open(lock, StandardOpenOption.WRITE)) {
                    FileLock taken = channel.tryLock();
                    if (taken == null) {
                        continue;
                    }
                    try {
                        delete(directory);
                    } finally {
                        taken.release();
                    }
                } catch (OverlappingFileLockException e) {
                    // This process uses it.
                }
            } catch (IOException ignored) {
                // Deleted by another process as well, or not to be deleted now: asked again later.
            }
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
