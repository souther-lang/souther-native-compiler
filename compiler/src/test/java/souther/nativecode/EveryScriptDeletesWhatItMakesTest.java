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
 * A script deletes what it makes however it ends. A directory deleted by a line after its use is left
 * wherever the script stops before that line, which under {@code set -e} is any failure; so each
 * directory a script makes with {@code mktemp} is either deleted by a {@code trap} on {@code EXIT}
 * set on the very next line, or made inside one that is.
 */
class EveryScriptDeletesWhatItMakesTest {

    private static final Pattern MADE = Pattern.compile("^\\s*(?:local\\s+)?(\\w+)=\"\\$\\(mktemp (.*)\\)\"");

    @Test
    void everyDirectoryAScriptMakesIsDeletedByATrapOrInsideOne() throws IOException {
        List<String> left = new ArrayList<>();
        try (Stream<Path> scripts = Files.list(Repository.file("scripts"))) {
            for (Path script : scripts.filter(Files::isRegularFile).sorted().toList()) {
                List<String> lines = Files.readAllLines(script, StandardCharsets.UTF_8);
                List<String> trapped = new ArrayList<>();
                for (int at = 0; at < lines.size(); at++) {
                    Matcher made = MADE.matcher(lines.get(at));
                    if (!made.find()) {
                        continue;
                    }
                    String name = made.group(1);
                    boolean inside = trapped.stream().anyMatch(it -> made.group(2).contains("$" + it + "/")
                            || made.group(2).contains("${" + it + "}/"));
                    String next = at + 1 < lines.size() ? lines.get(at + 1).strip() : "";
                    boolean deleted = next.startsWith("trap ") && next.contains("$" + name)
                            && next.endsWith("EXIT");
                    if (deleted) {
                        trapped.add(name);
                    } else if (!inside) {
                        left.add(script.getFileName() + ":" + (at + 1) + ": " + lines.get(at).strip());
                    }
                }
            }
        }
        assertThat(left).as("directories a script makes and may leave behind").isEmpty();
    }
}
