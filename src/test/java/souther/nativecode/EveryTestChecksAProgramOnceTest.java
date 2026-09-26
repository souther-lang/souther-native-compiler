package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every test checks a program through {@link Checked}, so that a test asking a program several
 * questions checks it once. A helper checking its program for each question it is asked is how the
 * tests came to check one program as many times as they asked about it, in one helper per class;
 * a test written after this one would write the next such helper without anything saying so.
 */
class EveryTestChecksAProgramOnceTest {

    @Test
    void noTestChecksAProgramButThroughChecked() throws IOException {
        List<Path> direct;
        try (Stream<Path> sources = Files.walk(Path.of("src", "test", "java"))) {
            direct = sources
                    .filter(it -> it.toString().endsWith(".java"))
                    .filter(it -> !it.getFileName().toString().equals("Checked.java"))
                    .filter(it -> !it.getFileName().toString()
                            .equals("EveryTestChecksAProgramOnceTest.java"))
                    .filter(it -> read(it).contains("CheckedProgram" + ".of("))
                    .toList();
        }
        assertThat(direct).as("tests checking a program other than through Checked").isEmpty();
    }

    private static String read(Path source) {
        try {
            return Files.readString(source);
        } catch (IOException unread) {
            throw new AssertionError("a test source that cannot be read: " + source, unread);
        }
    }
}
