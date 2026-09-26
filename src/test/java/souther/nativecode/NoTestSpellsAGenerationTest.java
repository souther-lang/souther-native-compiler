package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * No test of either half spells the ABI generation a symbol carries: a harness writes
 * {@code souther@} and is handed on through what writes the generation out, {@link Running#spelt}
 * on this side and {@code support::harness} on the driver's, and an assertion builds a symbol from
 * {@link Running#ABI}. A generation spelt in a test is one more place to change each time it moves,
 * and one a branch written before the move brings back with it.
 *
 * <p>A fixture a test writes out and holds a build to (the manifest of a version) is what a build
 * wrote, and not a test spelling anything, so JSON is not read.
 */
class NoTestSpellsAGenerationTest {

    /** A symbol's generation, spelt: {@code souther}, digits, then what a symbol goes on with. */
    private static final Pattern SPELT = Pattern.compile("souther[0-9]+[._]");

    @Test
    void noTestSourceSpellsTheGeneration() throws IOException {
        List<String> spelt = new ArrayList<>();
        for (Path root : List.of(Path.of("src", "test", "java"),
                Path.of("native", "crates", "compiler", "tests"))) {
            try (Stream<Path> files = Files.walk(root)) {
                for (Path file : files.filter(it -> it.toString().endsWith(".java")
                        || it.toString().endsWith(".rs")).toList()) {
                    List<String> lines = Files.readAllLines(file, StandardCharsets.UTF_8);
                    for (int at = 0; at < lines.size(); at++) {
                        Matcher matcher = SPELT.matcher(lines.get(at));
                        if (matcher.find()) {
                            spelt.add(file + ":" + (at + 1) + ": " + matcher.group());
                        }
                    }
                }
            }
        }

        assertThat(spelt).isEmpty();
    }
}
