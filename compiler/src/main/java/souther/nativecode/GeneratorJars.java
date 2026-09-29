package souther.nativecode;

import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;

/**
 * The jar of a generator the catalog names, kept where the command finds it next time.
 *
 * <p>Only what the catalog names is fetched, at this compiler's own version and from the repository
 * this is configured with, so a flag on a command line cannot make the compiler run anything else.
 * What Maven publishes beside a jar is its checksum, and what is compared is the jar against that:
 * it finds a jar that came damaged, and says nothing of a repository that serves both wrong, which
 * is what the repository being Maven Central, and being fixed once published, is for.
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
        String base = fetching.maven().toString().replaceAll("/+$", "");
        URI jar = URI.create(base + "/" + coordinates[0].replace('.', '/') + "/" + coordinates[1]
                + "/" + version + "/" + name);
        try {
            byte[] bytes = fetching.downloads().get(jar);
            String said = new String(fetching.downloads().get(URI.create(jar + ".sha256")),
                    StandardCharsets.UTF_8).trim().split("\\s+")[0].toLowerCase(Locale.ROOT);
            if (!said.equals(Fetching.sha256(bytes))) {
                throw new NotFetched(jar + " does not match its checksum: refused, and not kept");
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
