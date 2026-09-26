package souther.nativecode.transport;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A call reaching another module's published value crosses tagged {@code "publishedvalue"}, never
 * {@code "value"} — the distinction {@code Reaches::Value}/{@code Reaches::PublishedValue} exist to
 * hold apart, and one no earlier transport test put through a real cross-module call to fix (review
 * of #20). A typo on either side of that spelling would fail this the same way any other vocabulary
 * mismatch does: the reading half refuses a tag it does not carry.
 *
 * <p>{@code reader.g} takes nothing and answers with {@code publisher.ys}. Because the call crosses
 * a module boundary it is written with zero arguments — unlike a same-module reach to a value
 * ({@link AValueThatHandsOverAnotherValueCrossesTest}), which carries its handovers as ordinary call
 * arguments: nothing of {@code ys}'s own dependencies is {@code reader}'s to supply, since it never
 * holds a copy of {@code ys} to run.
 */
class APublishedValueCrossesAsItsOwnReachTest {

    private static final String PUBLISHER = """
            module publisher exposing ( Box, ys )

            data Box = { n: Int }

            let ks = Box { n = 42 }

            let ys = ks
            """;

    private static final String READER = """
            module reader exposing ( g )

            import publisher ( Box, ys )

            behavior g : () -> Box
            let g = ys
            """;

    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "published_value.transport.json");

    @Test
    void aCallToAnotherModulesValueCrossesAsPublishedValueWithZeroArguments() {
        String written = ProgramWriter.written(Checked.of(List.of(PUBLISHER, READER)));

        // Both wire spellings appear in this document, and they must not be interchangeable: the
        // publisher's own entry reaches ys and ks as ordinary same-module values ("value"), while
        // reader's behavior — the crossing call this test is about — reaches ys as "publishedvalue"
        // and with no arguments, since none of ys's own handovers are reader's to supply.
        assertThat(written).contains(
                "\"declared\":\"reader.g\",\"parameters\":[],\"publication\":\"published\","
                        + "\"body\":{\"core\":\"let\",\"binding\":0,\"binds\":{\"declared\":\"publisher.Box\"},"
                        + "\"value\":{\"core\":\"call\","
                        + "\"reaches\":{\"is\":\"publishedvalue\",\"module\":\"publisher\","
                        + "\"name\":\"ys\"},\"arguments\":[]");
    }

    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(Checked.of(List.of(PUBLISHER, READER))));
    }
}
