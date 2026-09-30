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
 * Every test that walks the classes a type was loaded from finds them through
 * {@link souther.bindings.Classes}, which reads a directory and a jar alike. What a module's tests are
 * handed of another module is its output directory under {@code mvn test} and its jar under
 * {@code mvn verify}, and a test that read the code source as a directory passed under the one and
 * failed under the other.
 */
class EveryTestWalksClassesThroughClassesTest {

    @Test
    void noTestReadsACodeSourceButThroughClasses() throws IOException {
        List<Path> direct = new ArrayList<>();
        for (Path tests : Repository.testSources()) {
            try (Stream<Path> sources = Files.walk(tests)) {
                sources.filter(it -> it.toString().endsWith(".java"))
                        .filter(it -> !it.getFileName().toString().equals("Classes.java"))
                        .filter(it -> !it.getFileName().toString()
                                .equals("EveryTestWalksClassesThroughClassesTest.java"))
                        .filter(it -> read(it).contains("getCodeSource" + "()"))
                        .forEach(direct::add);
            }
        }
        assertThat(direct).as("tests reading a code source other than through Classes").isEmpty();
    }

    private static String read(Path source) {
        try {
            return Files.readString(source);
        } catch (IOException unread) {
            throw new AssertionError("a test source that cannot be read: " + source, unread);
        }
    }
}
