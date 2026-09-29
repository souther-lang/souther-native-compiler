package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The method that marks a member of a union, or a case of a sum, has no body, and gofmt keeps its
 * braces on the line of its header only where the header is shorter than 100 bytes. A union whose
 * members have long names is where that shows first, as in the cart example's answers. The names
 * here put a case's header at 99 bytes and the next at 100, so both sides are held: gofmt, which
 * the host refuses a binding for where it would change it, splits a header of 100; and one of 99
 * is on one line, which gofmt would also leave split, so it is read.
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

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"

            	"example.com/books"
            	"example.com/books/ledger"
            )

            func main() {
            	library, err := books.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *books.Run) error {
            		for _, tries := range []int64{1, 2} {
            			settled, err := ledger.Settle(r, tries)
            			if err != nil {
            				return err
            			}
            			switch settled.Case().(type) {
            			case ledger.AccountSettlementWithLongNamesSettledOnTheFirstCharge:
            				fmt.Println("first")
            			case ledger.AccountSettlementWithLongNamesSettledOnTheSecondCharge:
            				fmt.Println("second")
            			}
            		}
            		for _, tries := range []int64{3, 10} {
            			attempted, err := ledger.Attempt(r, tries)
            			if err != nil {
            				return err
            			}
            			switch it := attempted.(type) {
            			case ledger.IntOrSettledAfterVeryManyAttemptsInt:
            				fmt.Println(it.Value)
            			case ledger.IntOrSettledAfterVeryManyAttemptsSettledAfterVeryManyAttempts:
            				fmt.Println("many")
            			}
            		}
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aHeaderOfOneHundredBytesHasItsBracesOnTwoLines(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, LEDGER, "example.com/books", HOST);

        assertThat(said).isEqualTo("first\nsecond\n3\nmany\n");
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
