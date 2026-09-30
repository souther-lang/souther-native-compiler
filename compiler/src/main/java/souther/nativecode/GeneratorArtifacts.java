package souther.nativecode;

import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

/**
 * A generator's jar as the command runs it: its bytes held to their digest, at a place only this
 * command writes, before anything the jar says of itself is read.
 *
 * <p>A jar fetched from a repository is held to the SHA-256 it was named with, and to nothing the
 * repository says of it: a checksum served beside a jar is served by whoever serves the jar. What
 * matches is kept under its coordinate and its digest, so that other bytes under the same coordinate
 * are another artifact, and a kept jar is hashed again each time it is used; one that no longer
 * matches is dropped and fetched again, or refused offline. A jar on this machine is copied first
 * and the copy hashed and loaded, so a jar replaced between the hashing and the loading is not what
 * runs.
 */
final class GeneratorArtifacts {

    private GeneratorArtifacts() {
    }

    /** The jar {@code ref} points at, verified. */
    static VerifiedJar materialize(Fetching fetching, GeneratorRef ref) throws NotFetched {
        return switch (ref) {
            case GeneratorRef.Maven maven -> fetched(fetching, maven);
            case GeneratorRef.Local local -> snapshot(local);
        };
    }

    private static VerifiedJar fetched(Fetching fetching, GeneratorRef.Maven maven)
            throws NotFetched {
        MavenCoordinate coordinate = maven.coordinate();
        Path kept = fetching.cache().resolve("generators").resolve(coordinate.group().replace('.', '/'))
                .resolve(coordinate.artifact()).resolve(coordinate.version())
                .resolve(maven.sha256() + ".jar");
        try {
            if (Files.isRegularFile(kept)) {
                if (Fetching.sha256(Files.readAllBytes(kept)).equals(maven.sha256())) {
                    return new VerifiedJar(kept, maven, maven.sha256(), false);
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
            try {
                Files.write(partial, bytes);
                Files.move(partial, kept, StandardCopyOption.ATOMIC_MOVE,
                        StandardCopyOption.REPLACE_EXISTING);
            } finally {
                Files.deleteIfExists(partial);
            }
        } catch (IOException e) {
            throw new NotFetched("could not keep " + jar + ": " + e.getMessage(), e);
        }
        return new VerifiedJar(kept, maven, maven.sha256(), false);
    }

    private static VerifiedJar snapshot(GeneratorRef.Local local) throws NotFetched {
        Path copy;
        try {
            Path directory = Files.createTempDirectory("souther-generator-");
            copy = directory.resolve("generator.jar");
            try {
                Files.copy(local.path(), copy);
            } catch (IOException e) {
                Files.deleteIfExists(directory);
                throw e;
            }
        } catch (NoSuchFileException e) {
            throw new NotFetched("there is no jar at " + local.path(), e);
        } catch (IOException e) {
            throw new NotFetched("could not read " + local.path() + ": " + e.getMessage(), e);
        }
        try {
            return new VerifiedJar(copy, local, Fetching.sha256(Files.readAllBytes(copy)), true);
        } catch (IOException e) {
            try {
                new VerifiedJar(copy, local, "", true).discard();
            } catch (IOException ignored) {
                e.addSuppressed(ignored);
            }
            throw new NotFetched("could not read the copy of " + local.path() + ": " + e.getMessage(), e);
        }
    }
}
