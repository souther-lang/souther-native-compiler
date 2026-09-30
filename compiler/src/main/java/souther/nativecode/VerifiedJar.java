package souther.nativecode;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * A generator's jar whose bytes were just held to their digest, written from those very bytes to a
 * place this command made for it alone: what is loaded is this file, and never the path, the address
 * or the kept copy it was pointed at, each of which could be other bytes by the time a class is read.
 *
 * @param jar    the copy of the bytes that were hashed, which is what is loaded
 * @param ref    what the jar was pointed at by
 * @param sha256 the SHA-256 of those bytes
 */
record VerifiedJar(Path jar, GeneratorRef ref, String sha256) {

    /** Deletes the copy, which nothing else holds. */
    void discard() throws IOException {
        Files.deleteIfExists(jar);
        Files.deleteIfExists(jar.getParent());
    }

    /** What the mark of a binding it wrote records of it. */
    BindingDirectory.Artifact artifact() {
        return switch (ref) {
            case GeneratorRef.Maven maven -> new BindingDirectory.Artifact.Maven(
                    maven.coordinate().toString(), sha256);
            case GeneratorRef.Local local -> new BindingDirectory.Artifact.Local(sha256);
        };
    }
}
