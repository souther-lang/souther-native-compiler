package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Whether a version is a release is said once ({@link Release#is}), and asked everywhere else. A
 * second statement of it was a second place deciding to behave as a release, and the one that did so
 * without asking {@link Fetching#release} skipped what a release is held to there (that the API
 * beside it is of the same release).
 */
class OneStatementOfWhatAReleaseIsTest {

    @Test
    void noSourceButReleaseSaysWhatASnapshotIs() throws IOException {
        List<String> said = new ArrayList<>();
        for (Path sources : List.of(Repository.file("compiler", "src", "main", "java"),
                Repository.file("bindings", "testkit", "src", "main", "java"))) {
            try (Stream<Path> files = Files.walk(sources)) {
                for (Path file : files.filter(it -> it.toString().endsWith(".java")
                        && !it.getFileName().toString().equals("Release.java")).toList()) {
                    if (Files.readString(file).contains("\"-SNAPSHOT\"")) {
                        said.add(file.getFileName().toString());
                    }
                }
            }
        }
        assertThat(said).as("sources saying what a snapshot is, other than Release").isEmpty();
    }
}
