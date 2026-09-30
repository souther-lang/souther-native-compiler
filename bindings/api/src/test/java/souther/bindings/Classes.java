package souther.bindings;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.net.URISyntaxException;
import java.nio.file.FileSystem;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Where the classes a type was loaded with are, as a directory to walk: the build's own output
 * directory, or the root of the jar it was packed into. Which one a module's tests are handed depends
 * on how far the reactor went before them ({@code mvn test} hands a directory, {@code mvn verify} a
 * jar), so every test that walks classes goes through here, and none reads a code source itself.
 */
public final class Classes {

    /** Each jar opened, open for as long as the tests run, since a walk hands its paths on. */
    private static final Map<Path, FileSystem> OPENED = new ConcurrentHashMap<>();

    private Classes() {
    }

    /** The root of what {@code anchor} was loaded from. */
    public static Path of(Class<?> anchor) {
        Path location;
        try {
            location = Path.of(anchor.getProtectionDomain().getCodeSource().getLocation().toURI());
        } catch (URISyntaxException e) {
            throw new IllegalStateException(e);
        }
        if (Files.isDirectory(location)) {
            return location;
        }
        return OPENED.computeIfAbsent(location, jar -> {
            try {
                return FileSystems.newFileSystem(jar);
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }).getPath("/");
    }

    /** The binary name of the class {@code file} under {@code root} holds. */
    public static String nameOf(Path root, Path file) {
        return root.relativize(file).toString().replace(file.getFileSystem().getSeparator(), ".")
                .replaceAll("\\.class$", "");
    }
}
