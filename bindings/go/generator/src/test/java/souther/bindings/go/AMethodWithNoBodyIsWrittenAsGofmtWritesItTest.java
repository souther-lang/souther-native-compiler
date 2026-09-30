package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The method that marks a member of a union has no body, and gofmt keeps its braces on the line of
 * its header only where the header is shorter than 100 bytes. A member of the union's own package is
 * marked on its own type; a primitive is held by a type named after the union and the member, which
 * is where a long header comes from. The names here put the header of the Int member's at 99 bytes
 * and the Bool member's at 100, so both sides are held: gofmt is run over what was generated and
 * would split a header of 100 written on one line; and one of 99 is on one line, which gofmt would
 * also leave split, so it is read. Nothing is built or run: what is held is the text, which is what
 * the host tests build.
 */
class AMethodWithNoBodyIsWrittenAsGofmtWritesItTest {

    // The union is BoolOrIntOrSettledAfterVeryManyAttemptsNow, 42 bytes, and a primitive member's
    // header is `func (` + union + member + `) is` + union + `()`: 15 + 84 = 99 for Int, 100 for Bool.
    private static final String LEDGER = """
            module ledger exposing ( SettledAfterVeryManyAttemptsNow, attempt )

            data SettledAfterVeryManyAttemptsNow

            behavior attempt : (tries: Int) -> Bool | Int | SettledAfterVeryManyAttemptsNow
            let attempt (tries) =
                if tries > 9 then SettledAfterVeryManyAttemptsNow
                else if tries > 5 then true
                else tries
            """;

    @Test
    void aHeaderOfOneHundredBytesHasItsBracesOnTwoLines(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(Checked.of(List.of(LEDGER)), into.resolve("native"));

        Generated binding = GoHost.generated(library, into.resolve("binding"), "example.com/books");

        GoHost.formatted(binding);
        String module = Files.readString(into.resolve("binding/ledger/module.go"), StandardCharsets.UTF_8);
        assertThat(module)
                .contains("func (BoolOrIntOrSettledAfterVeryManyAttemptsNowInt)"
                        + " isBoolOrIntOrSettledAfterVeryManyAttemptsNow() {}\n")
                .contains("func (BoolOrIntOrSettledAfterVeryManyAttemptsNowBool)"
                        + " isBoolOrIntOrSettledAfterVeryManyAttemptsNow() {\n}\n")
                .contains("func (SettledAfterVeryManyAttemptsNow) isBoolOrIntOrSettledAfterVeryManyAttemptsNow() {}\n");
    }
}
