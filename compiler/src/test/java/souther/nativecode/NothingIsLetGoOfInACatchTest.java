package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What the command acquires on the way is let go of through {@link Holding}, and never in a
 * {@code catch}. A {@code catch} lets go only on the throwables it names, and what it did not name, an
 * {@link Error} of a generator's own or a type added later, left behind the copy of a jar, a class
 * loader, or a directory being written (a generator's {@code AssertionError} left a {@code .writing-}
 * directory beside where its binding was to go). Scanned over the command's and the testkit's
 * sources.
 */
class NothingIsLetGoOfInACatchTest {

    /** A catch whose block, past any comment, begins by letting go of something. */
    private static final Pattern LETTING_GO = Pattern.compile(
            "catch \\([^)]*\\) \\{\\n(?:\\s*//.*\\n)*\\s*[^\\n]*"
                    + "(?:abandon|abandoned|discard|\\.close|delete|deleteIfExists|remove)\\(");

    @Test
    void noCatchLetsGoOfWhatWasAcquired() throws IOException {
        List<String> found = new ArrayList<>();
        for (Path sources : List.of(Repository.file("compiler", "src", "main", "java"),
                Repository.file("bindings", "testkit", "src", "main", "java"))) {
            try (Stream<Path> files = Files.walk(sources)) {
                for (Path file : files.filter(it -> it.toString().endsWith(".java")).toList()) {
                    Matcher matcher = LETTING_GO.matcher(Files.readString(file));
                    while (matcher.find()) {
                        found.add(file.getFileName() + ": " + matcher.group().strip());
                    }
                }
            }
        }
        assertThat(found).as("catches that let go of what a step acquired; hold it instead").isEmpty();
    }
}
