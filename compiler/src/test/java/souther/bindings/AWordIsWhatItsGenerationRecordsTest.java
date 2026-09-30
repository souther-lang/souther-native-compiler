package souther.bindings;

import org.junit.jupiter.api.Test;
import souther.nativecode.ManifestReader;
import souther.nativecode.Repository;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What each word is on the machine is the ABI's, and a generator writes a word's type from the
 * API's {@link Manifest.Word#representation}: so the API's is held here to the record of the
 * generation the command reads, line by line, and a word the record has and the API does not, or
 * the other way, fails it.
 */
class AWordIsWhatItsGenerationRecordsTest {

    private static final Pattern WORD = Pattern.compile("^word (\\w+): (\\w+), (datum|handle)$");

    @Test
    void everyWordIsTheRepresentationItsGenerationRecords() throws Exception {
        Path record = Repository.file("native", "crates", "abi", "generations",
                ManifestReader.ABI + ".txt");
        Map<String, String> recorded = new TreeMap<>();
        for (String line : Files.readAllLines(record, StandardCharsets.UTF_8)) {
            Matcher it = WORD.matcher(line);
            if (it.matches()) {
                recorded.put(it.group(1).toUpperCase(Locale.ROOT),
                        it.group(2).toUpperCase(Locale.ROOT));
            }
        }
        Map<String, String> stated = new TreeMap<>();
        for (Manifest.Word word : Manifest.Word.values()) {
            stated.put(word.name(), word.representation().name());
        }
        assertThat(stated).isEqualTo(recorded);
    }
}
