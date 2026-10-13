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
 * PHP's command line writes what went wrong to its standard output unless it is told otherwise, and
 * a script that keeps that output in a variable, {@code said="$(php ...)"}, keeps the error there
 * too: under {@code set -e} the script ends at the failing PHP and says nothing of why, which is how
 * a publication was once refused with an empty reason. So every PHP whose output a script keeps
 * sends its errors to standard error, where they reach whoever reads the script's log.
 */
class EveryPhpAScriptKeepsTheOutputOfSaysWhatWentWrongTest {

    /** A substitution's PHP: {@code $(php ...} or {@code $(cd ... && php ...}. */
    private static final Pattern KEPT = Pattern.compile("\\$\\((?:cd [^)&]*&&\\s*)?php\\b([^\\n]*)");

    @Test
    void everyPhpWhoseOutputIsKeptWritesItsErrorsToStandardError() throws IOException {
        List<String> kept = new ArrayList<>();
        List<String> silent = new ArrayList<>();
        try (Stream<Path> scripts = Files.list(Repository.file("scripts"))) {
            for (Path script : scripts.sorted().toList()) {
                Matcher php = KEPT.matcher(Files.readString(script));
                while (php.find()) {
                    String said = script.getFileName() + ": " + php.group().strip();
                    kept.add(said);
                    if (!php.group(1).contains("-d display_errors=stderr")) {
                        silent.add(said);
                    }
                }
            }
        }

        assertThat(kept).as("the scripts keep the output of some PHP").isNotEmpty();
        assertThat(silent).isEmpty();
    }
}
