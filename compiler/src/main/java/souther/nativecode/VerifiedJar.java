package souther.nativecode;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * A generator's jar whose bytes were just held to their digest, at a place nothing but this command
 * writes to: what is loaded is this file, never the path or the address it was pointed at.
 *
 * @param jar      the bytes that were hashed, and that are loaded
 * @param ref      what the jar was pointed at by
 * @param sha256   the SHA-256 of {@code jar}
 * @param snapshot whether {@code jar} is a copy made for this command alone, which {@link #discard}
 *                 deletes; a jar kept in the cache is not one
 */
record VerifiedJar(Path jar, GeneratorRef ref, String sha256, boolean snapshot) {

    /** Deletes the jar where it is this command's own copy, and leaves a kept one. */
    void discard() throws IOException {
        if (snapshot) {
            Files.deleteIfExists(jar);
            Files.deleteIfExists(jar.getParent());
        }
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
