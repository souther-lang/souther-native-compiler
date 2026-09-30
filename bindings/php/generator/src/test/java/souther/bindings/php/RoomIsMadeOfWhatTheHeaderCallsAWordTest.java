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
import souther.nativecode.Repository;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What PHP's FFI makes room of for a word is what the header the build wrote calls that word:
 * {@code c_word} in {@code interface.rs}, which is what the declarations are written from, and
 * which spells a number as the C type of its representation in the ABI crate
 * ({@code HostWord::representation}, {@code Representation::c}). The two
 * are written in two languages, and a spelling copied from one to the other is held to it here
 * rather than trusted, as a word PHP makes no room of is held to having none.
 */
class RoomIsMadeOfWhatTheHeaderCallsAWordTest {

    private static final Path INTERFACE =
            Repository.file("native", "crates", "compiler", "src", "interface.rs");

    private static final Path ABI = Repository.file("native", "crates", "abi", "src", "lib.rs");

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
        String body = body(Files.readString(INTERFACE), "fn c_word(", "");
        Map<Word, String> spelt = new LinkedHashMap<>();
        Matcher arm = Pattern.compile("Word::(\\w+) => \"([^\"]+)\"").matcher(body);
        while (arm.find()) {
            spelt.put(word(arm.group(1)), arm.group(2));
        }
        // The numbers, spelt as their representation is: one arm naming them all.
        Matcher numbers = Pattern.compile("((?:Word::\\w+\\s*\\|\\s*)*Word::\\w+)\\s*=>\\s*\\{\\s*"
                + "HostWord::from\\(word\\)\\.representation\\(\\)\\.c\\(\\)").matcher(body);
        assertThat(numbers.find()).as("c_word spells numbers by their representation").isTrue();
        Map<String, String> representation = representations();
        Matcher each = Pattern.compile("Word::(\\w+)").matcher(numbers.group(1));
        while (each.find()) {
            spelt.put(word(each.group(1)), representation.get(each.group(1)));
        }
        return spelt;
    }

    /** The C type of each word's representation, by the word's name in Rust. */
    private static Map<String, String> representations() throws Exception {
        String source = Files.readString(ABI);
        Map<String, String> c = new LinkedHashMap<>();
        Matcher type = Pattern.compile("Representation::(\\w+) => \"([^\"]+)\"")
                .matcher(body(source, "pub const fn c(self)", "    "));
        while (type.find()) {
            c.put(type.group(1), type.group(2));
        }
        Map<String, String> of = new LinkedHashMap<>();
        Matcher arm = Pattern.compile("((?:HostWord::\\w+\\s*\\|?\\s*)+)=>\\s*Representation::(\\w+)")
                .matcher(body(source, "pub const fn representation(self)", "    "));
        while (arm.find()) {
            Matcher named = Pattern.compile("HostWord::(\\w+)").matcher(arm.group(1));
            while (named.find()) {
                of.put(named.group(1), c.get(arm.group(2)));
            }
        }
        return of;
    }

    /** The function {@code opening} begins, to the brace that closes it at {@code indent}. */
    private static String body(String source, String opening, String indent) {
        int at = source.indexOf(opening);
        assertThat(at).as("%s", opening).isNotNegative();
        return source.substring(at, source.indexOf("\n" + indent + "}\n", at));
    }

    private static Word word(String rust) {
        return Word.valueOf(rust.toUpperCase(Locale.ROOT));
    }
}
