package souther.bindings.php;

import org.junit.jupiter.api.Test;
import souther.bindings.Manifest.Word;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What PHP's FFI makes room of for a word is what the header the build wrote calls that word:
 * {@code c_word} in {@code interface.rs}, which is what the declarations are written from. The two
 * are written in two languages, and a spelling copied from one to the other is held to it here
 * rather than trusted, as a word PHP makes no room of is held to having none.
 */
class RoomIsMadeOfWhatTheHeaderCallsAWordTest {

    private static final Path INTERFACE =
            Path.of("native", "crates", "compiler", "src", "interface.rs");

    @Test
    void everyWordPhpMakesRoomOfIsSpeltAsTheHeaderSpellsIt() throws Exception {
        Map<Word, String> header = spelt();
        assertThat(header.keySet()).containsExactlyInAnyOrder(Word.values());
        for (Word word : Word.values()) {
            String room = roomOf(word);
            if (room != null) {
                assertThat(room).as("room of %s", word).isEqualTo(header.get(word));
            }
        }
        // An address the declarations are written through is never room PHP makes to be written.
        header.forEach((word, c) -> assertThat(c.contains("*") && roomOf(word) != null)
                .as("room of %s", word).isFalse());
    }

    /** What PHP makes room of for {@code word}, or null where it makes none. */
    private static String roomOf(Word word) {
        try {
            return Crossing.storage(word);
        } catch (IllegalArgumentException none) {
            return null;
        }
    }

    /** Each word {@code c_word} names, as the Java enum spells it, with what it is called. */
    private static Map<Word, String> spelt() throws Exception {
        String source = Files.readString(INTERFACE);
        int at = source.indexOf("fn c_word(");
        assertThat(at).as("c_word in %s", INTERFACE).isNotNegative();
        String body = source.substring(at, source.indexOf("\n}\n", at));
        Map<Word, String> spelt = new LinkedHashMap<>();
        Matcher arm = Pattern.compile("Word::(\\w+) => \"([^\"]+)\"").matcher(body);
        while (arm.find()) {
            spelt.put(Word.valueOf(arm.group(1).toUpperCase(Locale.ROOT)), arm.group(2));
        }
        return spelt;
    }
}
