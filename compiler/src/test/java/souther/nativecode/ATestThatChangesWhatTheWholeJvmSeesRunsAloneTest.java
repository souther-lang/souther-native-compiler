package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Test classes run at once (the surefire configuration in the root pom), so a class that changes
 * what every class in the JVM sees is marked {@code @Isolated} and runs with nothing beside it: one
 * that sets or clears a system property, or reads the temporary directory, where any command that
 * loads a generator's jar leaves and removes copies. A class that reaches such a change through a
 * helper of the test sources, or a helper of a helper, is held to the same.
 *
 * <p>Without this, the class added next that clears a property would pass alone and fail some runs
 * in CI, when another class happened to read the property in between.
 */
class ATestThatChangesWhatTheWholeJvmSeesRunsAloneTest {

    private static final Pattern CHANGES = Pattern.compile(
            "System\\.(?:setProperty|clearProperty)\\(|\"java\\.io\\.tmpdir\"");

    private static final Pattern ISOLATED = Pattern.compile("^@Isolated\\s*$", Pattern.MULTILINE);

    @Test
    void everyClassThatChangesWhatTheJvmSeesIsIsolated() {
        Map<String, String> sources = sources();
        Set<String> changing = new TreeSet<>();
        sources.forEach((name, text) -> {
            if (CHANGES.matcher(text).find()) {
                changing.add(name);
            }
        });
        // What reaches a class that changes it changes it too, through as many helpers as it takes.
        boolean grew = true;
        while (grew) {
            grew = false;
            for (Map.Entry<String, String> source : sources.entrySet()) {
                if (!changing.contains(source.getKey()) && changing.stream()
                        .anyMatch(it -> Pattern.compile("\\b" + it + "\\b").matcher(source.getValue()).find())) {
                    changing.add(source.getKey());
                    grew = true;
                }
            }
        }
        List<String> beside = new ArrayList<>();
        for (String name : changing) {
            if (name.endsWith("Test") && !ISOLATED.matcher(sources.get(name)).find()) {
                beside.add(name);
            }
        }
        assertThat(changing).as("classes found changing what the JVM sees").isNotEmpty();
        assertThat(beside).as("test classes that change what the JVM sees and are not @Isolated").isEmpty();
    }

    /** Every class of the test sources by its simple name, and its text: two of a name, both texts. */
    private static Map<String, String> sources() {
        Map<String, String> sources = new LinkedHashMap<>();
        for (Path tests : Repository.testSources()) {
            try (Stream<Path> files = Files.walk(tests)) {
                for (Path file : files.filter(it -> it.toString().endsWith(".java")).toList()) {
                    String name = file.getFileName().toString().replaceFirst("\\.java$", "");
                    if (!name.equals(ATestThatChangesWhatTheWholeJvmSeesRunsAloneTest.class.getSimpleName())) {
                        sources.merge(name, Files.readString(file), String::concat);
                    }
                }
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }
        return sources;
    }
}
