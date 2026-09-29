package souther.nativecode.transport;

import org.junit.jupiter.api.Test;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Each kind of node {@link ProgramWriter} writes is spelt in one place, and what a node can end with
 * is written through one place too.
 *
 * <p>A node is written for what a body holds and for what a row states, and the two have to cross
 * as one node for the backend to lower them as one. Spelt twice, they agree only until one of the
 * two is changed. An abort set spelt out by hand is the same thing for the question every node
 * answers: an answer written where it is needed rather than asked of the program or passed through
 * the one writer of it.
 *
 * <p>Read off the source, because what is held is how the writer is written and not what any one
 * document it writes happens to contain.
 */
class EveryNodeIsSpeltInOnePlaceTest {

    private static final Path SOURCE =
            Path.of("src/main/java/souther/nativecode/transport/ProgramWriter.java");

    @Test
    void noKindOfNodeIsSpeltTwice() throws Exception {
        Matcher kinds = Pattern.compile("\\\\\"core\\\\\":\\\\\"([a-z_]+)\\\\\"")
                .matcher(Files.readString(SOURCE));
        Map<String, Integer> spelt = new TreeMap<>();
        while (kinds.find()) {
            spelt.merge(kinds.group(1), 1, Integer::sum);
        }
        assertThat(spelt).as("the kinds of node the writer spells").isNotEmpty();

        List<String> twice = new ArrayList<>();
        spelt.forEach((kind, times) -> {
            if (times > 1) {
                twice.add(kind + " (" + times + " times)");
            }
        });
        assertThat(twice).as("kinds of node spelt in more than one place").isEmpty();
    }

    @Test
    void noAbortSetIsSpeltByHand() throws Exception {
        assertThat(Files.readString(SOURCE))
                .as("an abort set is written by spelled(AbortSet), never as a literal array")
                .doesNotContain("aborts\\\":[");
    }
}
