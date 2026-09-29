package souther.nativecode;

import java.io.IOException;
import java.io.ByteArrayInputStream;
import java.net.URI;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.DirectoryNotEmptyException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Comparator;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * The driver, with what it links beside it, as a release publishes it for one platform.
 *
 * <p>The driver reads the runtime archive and the file of what linking it needs from beside itself,
 * so what is fetched is the three together, and is kept in one directory. The bundle is held to the
 * SHA-256 this compiler was released with, since a release asset is not something that cannot be
 * replaced; and holds exactly those three files, so that an archive that names a path outside its own
 * directory cannot write there.
 */
final class NativeBundle {

    static final String DRIVER = "souther-native-driver";
    static final String ARCHIVE = "libsouther_native_runtime.a";
    static final String REQUIREMENTS = "libsouther_native_runtime.link";
    private static final Set<String> FILES = Set.of(DRIVER, ARCHIVE, REQUIREMENTS);

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

    /** The driver for this platform, fetched and kept if it is not. */
    static Path locate(Fetching fetching) throws NotFetched {
        String version = fetching.release();
        String platform = platform();
        Path dir = fetching.cache().resolve("native").resolve(version).resolve(platform);
        if (kept(dir)) {
            return dir.resolve(DRIVER);
        }
        if (fetching.offline()) {
            throw new NotFetched("the driver for " + platform + " " + version
                    + " is not kept, and this is offline");
        }
        String expected = fetching.checksums().get(platform);
        if (expected == null) {
            throw new NotFetched("this compiler was released with no checksum for the driver for "
                    + platform + ", so it will not take one");
        }
        String base = fetching.releases().toString().replaceAll("/+$", "");
        URI bundle = URI.create(base + "/v" + version + "/souther-native-" + version + "-"
                + platform + ".zip");
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
        try {
            Files.createDirectories(dir.getParent());
            Path staging = Files.createTempDirectory(dir.getParent(), platform + ".partial");
            try {
                unpack(bytes, staging);
                place(staging, dir);
            } finally {
                delete(staging);
            }
        } catch (NotFetched e) {
            throw e;
        } catch (IOException e) {
            throw new NotFetched("could not keep " + bundle + ": " + e.getMessage(), e);
        }
        return dir.resolve(DRIVER);
    }

    private static boolean kept(Path dir) {
        return Files.isExecutable(dir.resolve(DRIVER)) && Files.isRegularFile(dir.resolve(ARCHIVE))
                && Files.isRegularFile(dir.resolve(REQUIREMENTS));
    }

    private static void unpack(byte[] bundle, Path into) throws IOException {
        Set<String> seen = new HashSet<>();
        try (ZipInputStream zip = new ZipInputStream(new ByteArrayInputStream(bundle))) {
            for (ZipEntry entry = zip.getNextEntry(); entry != null; entry = zip.getNextEntry()) {
                String name = entry.getName();
                if (entry.isDirectory() || !FILES.contains(name) || !seen.add(name)) {
                    throw new NotFetched("the bundle holds \"" + name + "\", which is not one of "
                            + "the three files it is to hold, once each: refused");
                }
                Files.write(into.resolve(name), zip.readAllBytes());
            }
        }
        if (!seen.equals(FILES)) {
            throw new NotFetched("the bundle holds " + seen + ", and is to hold " + FILES);
        }
        if (!into.resolve(DRIVER).toFile().setExecutable(true, false)) {
            throw new NotFetched("the driver cannot be made executable in " + into);
        }
    }

    /** Puts what was unpacked where it is kept, in one move, unless another command already did. */
    private static void place(Path staging, Path dir) throws IOException {
        try {
            Files.move(staging, dir, StandardCopyOption.ATOMIC_MOVE);
        } catch (FileAlreadyExistsException | DirectoryNotEmptyException e) {
            if (!kept(dir)) {
                throw e;
            }
        } catch (AtomicMoveNotSupportedException e) {
            throw new NotFetched("the cache cannot be written in one move: " + e.getMessage(), e);
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
