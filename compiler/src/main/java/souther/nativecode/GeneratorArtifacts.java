package souther.nativecode;

import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

/**
 * A generator's jar as the command runs it: its bytes read once, held to their digest, and written
 * from those same bytes to a copy of this command's own, before anything the jar says of itself is
 * read.
 *
 * <p>A jar fetched from a repository is held to the SHA-256 it was named with, and to nothing the
 * repository says of it: a checksum served beside a jar is served by whoever serves the jar. What
 * matches is kept under its coordinate and its digest, so that other bytes under the same coordinate
 * are another artifact, and a kept jar is read and hashed again each time it is used; one that no
 * longer matches is dropped and fetched again, or refused offline. A jar on this machine is read and
 * hashed the same way, with no digest to hold it to.
 *
 * <p>Neither the kept jar nor the jar on this machine is what is loaded. A class loader reads its jar
 * when it needs a class, which can be long after the jar was hashed, and by then another process can
 * have replaced the file. What is loaded is always the private copy written from the bytes that were
 * hashed ({@link VerifiedJar}), so remote and local jars are run, and let go of, the one way.
 */
final class GeneratorArtifacts {

    private GeneratorArtifacts() {
    }

    /**
     * The jar {@code maven} names, kept, and fetched first where it is not kept or no longer matches:
     * what {@code --fetch} prepares, which is the cache and nothing that runs.
     */
    static Path kept(Fetching fetching, GeneratorRef.Maven maven) throws NotFetched {
        fetched(fetching, maven);
        return keptAt(fetching, maven);
    }

    /**
     * The jar {@code ref} points at, verified, as a copy of this command's own, which {@code held}
     * owns and deletes when it lets go. There is no copy without an owner.
     */
    static VerifiedJar materialize(Holding held, Fetching fetching, GeneratorRef ref)
            throws NotFetched {
        byte[] bytes = switch (ref) {
            case GeneratorRef.Maven maven -> fetched(fetching, maven);
            case GeneratorRef.Local local -> read(local);
        };
        try {
            Path directory = Files.createTempDirectory("souther-generator-");
            VerifiedJar jar = new VerifiedJar(directory.resolve("generator.jar"), ref,
                    Fetching.sha256(bytes));
            held.hold(jar, jar::discard);
            Files.write(jar.jar(), bytes);
            return jar;
        } catch (IOException e) {
            throw new NotFetched("could not copy " + ref + " to load it: " + e.getMessage(), e);
        }
    }

    private static Path keptAt(Fetching fetching, GeneratorRef.Maven maven) {
        MavenCoordinate coordinate = maven.coordinate();
        return fetching.cache().resolve("generators").resolve(coordinate.group().replace('.', '/'))
                .resolve(coordinate.artifact()).resolve(coordinate.version())
                .resolve(maven.sha256() + ".jar");
    }

    private static byte[] read(GeneratorRef.Local local) throws NotFetched {
        try {
            return Files.readAllBytes(local.path());
        } catch (NoSuchFileException e) {
            throw new NotFetched("there is no jar at " + local.path(), e);
        } catch (IOException e) {
            throw new NotFetched("could not read " + local.path() + ": " + e.getMessage(), e);
        }
    }

    /** The bytes of the jar {@code maven} names, kept or fetched, and matching its digest. */
    private static byte[] fetched(Fetching fetching, GeneratorRef.Maven maven) throws NotFetched {
        MavenCoordinate coordinate = maven.coordinate();
        Path kept = keptAt(fetching, maven);
        try {
            if (Files.isRegularFile(kept)) {
                byte[] bytes = Files.readAllBytes(kept);
                if (Fetching.sha256(bytes).equals(maven.sha256())) {
                    return bytes;
                }
                // Not what was kept: another process, or the disk, changed it. Never run, and asked
                // for again as if it had never been kept.
                Files.delete(kept);
            }
        } catch (IOException e) {
            throw new NotFetched("could not read the kept " + kept + ": " + e.getMessage(), e);
        }
        if (fetching.offline()) {
            throw new NotFetched(coordinate + " is not kept, and this is offline");
        }
        String base = fetching.maven().toString().replaceAll("/+$", "");
        URI jar = URI.create(base + "/" + coordinate.path());
        byte[] bytes;
        try {
            bytes = fetching.downloads().get(jar);
        } catch (IOException e) {
            throw new NotFetched("could not fetch " + jar + ": " + e.getMessage(), e);
        }
        if (!Fetching.sha256(bytes).equals(maven.sha256())) {
            throw new NotFetched(jar + " does not match the SHA-256 it was named with: refused, and"
                    + " not kept");
        }
        try {
            Files.createDirectories(kept.getParent());
            Path partial = Files.createTempFile(kept.getParent(), maven.sha256(), ".partial");
            try (Holding held = new Holding()) {
                held.hold(partial, () -> Files.deleteIfExists(partial));
                Files.write(partial, bytes);
                Files.move(partial, kept, StandardCopyOption.ATOMIC_MOVE,
                        StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (IOException e) {
            throw new NotFetched("could not keep " + jar + ": " + e.getMessage(), e);
        }
        return bytes;
    }
}
