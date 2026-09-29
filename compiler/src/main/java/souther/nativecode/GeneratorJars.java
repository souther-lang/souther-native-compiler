package souther.nativecode;

import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

/**
 * The jar of a generator the catalog names, kept where the command finds it next time.
 *
 * <p>Only what the catalog names is fetched, at this compiler's own version and from the repository
 * this is configured with, so a flag on a command line cannot make the compiler run anything else.
 * The jar is held to the SHA-256 this compiler was released with, and to nothing that the repository
 * says of itself: a checksum served beside a jar is served by whoever serves the jar, and Maven
 * Central does not require one.
 */
final class GeneratorJars {

    private GeneratorJars() {
    }

    /** The jar of the generator that brings {@code kind}, fetched if it is not kept. */
    static Path fetch(Fetching fetching, KnownBindings.Kind kind) throws NotFetched {
        String version = fetching.release();
        String[] coordinates = kind.artifact().split(":");
        String name = coordinates[1] + "-" + version + ".jar";
        Path kept = fetching.cache().resolve("generators").resolve(name);
        if (Files.isRegularFile(kept)) {
            return kept;
        }
        if (fetching.offline()) {
            throw new NotFetched(kind.artifact() + ":" + version
                    + " is not kept, and this is offline");
        }
        String expected = fetching.checksums().get(ReleaseChecksums.generator(kind.id()));
        if (expected == null) {
            throw new NotFetched("this compiler was released with no checksum for the " + kind.display()
                    + " generator, so it will not take one");
        }
        String base = fetching.maven().toString().replaceAll("/+$", "");
        URI jar = URI.create(base + "/" + coordinates[0].replace('.', '/') + "/" + coordinates[1]
                + "/" + version + "/" + name);
        try {
            byte[] bytes = fetching.downloads().get(jar);
            if (!expected.equals(Fetching.sha256(bytes))) {
                throw new NotFetched(jar + " does not match the checksum this compiler was released"
                        + " with: refused, and not kept");
            }
            Files.createDirectories(kept.getParent());
            Path partial = Files.createTempFile(kept.getParent(), name, ".partial");
            try {
                Files.write(partial, bytes);
                Files.move(partial, kept, StandardCopyOption.ATOMIC_MOVE,
                        StandardCopyOption.REPLACE_EXISTING);
            } finally {
                Files.deleteIfExists(partial);
            }
            return kept;
        } catch (NotFetched e) {
            throw e;
        } catch (IOException e) {
            throw new NotFetched("could not fetch " + jar + ": " + e.getMessage(), e);
        }
    }
}
