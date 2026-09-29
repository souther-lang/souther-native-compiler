package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.Generated;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The method that marks a member of a union, or a case of a sum, has no body, and gofmt keeps its
 * braces on the line of its header only where the header is shorter than 100 bytes. A union whose
 * members have long names is where that shows first, as in the cart example's answers. The names
 * here put a case's header at 99 bytes and the next at 100, so both sides are held: gofmt is run
 * over what was generated and would split a header of 100 written on one line; and one of 99 is on
 * one line, which gofmt would also leave split, so it is read. Nothing is built or run: what is held is the
 * text, which is what the host tests build.
 */
class AMethodWithNoBodyIsWrittenAsGofmtWritesItTest {

    // AccountSettlementWithLongNames is 30 bytes, and its cases 23 and 24: a case's header is
    // `func (` + sum + case + `) is` + sum + `Case()`, 16 + 60 + 23 = 99 and 100.
    private static final String LEDGER = """
            module ledger exposing ( AccountSettlementWithLongNames, SettledOnTheFirstCharge,
                                     SettledOnTheSecondCharge, SettledAfterVeryManyAttempts, settle,
                                     attempt )

            data SettledOnTheFirstCharge
            data SettledOnTheSecondCharge
            data SettledAfterVeryManyAttempts
            data AccountSettlementWithLongNames = SettledOnTheFirstCharge | SettledOnTheSecondCharge

            behavior settle : (tries: Int) -> AccountSettlementWithLongNames
            let settle (tries) = if tries > 1 then SettledOnTheSecondCharge else SettledOnTheFirstCharge

            behavior attempt : (tries: Int) -> Int | SettledAfterVeryManyAttempts
            let attempt (tries) = if tries > 9 then SettledAfterVeryManyAttempts else tries
            """;

    @Test
    void aHeaderOfOneHundredBytesHasItsBracesOnTwoLines(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(Checked.of(List.of(LEDGER)), into.resolve("native"));

        Generated binding = GoHost.generated(library, into.resolve("binding"), "example.com/books");

        GoHost.formatted(binding);
        String module = Files.readString(into.resolve("binding/ledger/module.go"), StandardCharsets.UTF_8);
        assertThat(module)
                .contains("func (AccountSettlementWithLongNamesSettledOnTheFirstCharge)"
                        + " isAccountSettlementWithLongNamesCase() {}\n")
                .contains("func (AccountSettlementWithLongNamesSettledOnTheSecondCharge)"
                        + " isAccountSettlementWithLongNamesCase() {\n}\n")
                .contains("func (IntOrSettledAfterVeryManyAttemptsInt) isIntOrSettledAfterVeryManyAttempts() {}\n")
                .contains("func (IntOrSettledAfterVeryManyAttemptsSettledAfterVeryManyAttempts)"
                        + " isIntOrSettledAfterVeryManyAttempts() {\n}\n");
    }
}
