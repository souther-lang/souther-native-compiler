package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Go host lists the rows of a behavior it implements and evaluates each against an implementation
 * of its own, one row at a time, in a run it opens for that row, as a test that loops over the rows
 * with {@code t.Run} would.
 *
 * <p>Whether an answer keeps a row is the library's to say, and the rows are chosen where the
 * external form would say otherwise: a decimal at another scale than the row writes keeps it, and a
 * row stating a case with nothing under it is kept by that case holding anything. An owed row comes
 * back answered with no verdict, and an implementation that fails ends the row, which is not an
 * answer that did not keep it.
 */
class AGoHostHoldsItsImplementationToTheRowsOfABehaviorTest {

    private static final String SHOP = """
            module shop exposing ( Quote, Priced, Unpriced, quoted )

            data Priced = { amount: Decimal }
            data Unpriced
            data Quote = Priced | Unpriced

            behavior quoted : (n: Int) -> Quote

            example quoted
                | "a price at the scale it was written" : (1) -> Priced { amount = 1.50m }
                | (2) -> Unpriced
                | "priced, whatever it costs" : (3) -> Priced
                | (4) -> <?>
            """;

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

            	"example.com/quotes"
            	"example.com/quotes/shop"

            	"github.com/raoh-project/raoh-go"
            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            // listed answers each n with what the price list says of it.
            type listed map[int64]string

            func (l listed) Apply(r *quotes.Run, n int64) (shop.Quote, error) {
            	amount, ok := l[n]
            	if !ok {
            		unpriced, err := shop.NewUnpriced(r)
            		if err != nil {
            			return shop.Quote{}, err
            		}
            		return shop.QuoteFromUnpriced(unpriced), nil
            	}
            	priced, err := shop.NewPriced(r, raoh.MustDecimal(amount))
            	if err != nil {
            		return shop.Quote{}, err
            	}
            	return shop.QuoteFromPriced(priced), nil
            }

            // lost has no price list to answer from.
            type lost struct{}

            func (lost) Apply(*quotes.Run, int64) (shop.Quote, error) {
            	return shop.Quote{}, errors.New("no price list")
            }

            // evaluated says what came of each row against impl, each in a run of its own.
            func evaluated(library *quotes.Library, name string, impl shop.Quoted) {
            	for _, row := range shop.QuotedExamples() {
            		err := library.Run(func(r *quotes.Run) error {
            			outcome := row.Evaluate(r, impl)
            			fmt.Printf("%s %d %q (%s): %v; failed %v\\n", name, row.At(), row.Shown(), row.Kind(),
            				outcome, outcome.Failed())
            			return nil
            		})
            		if err != nil {
            			panic(err)
            		}
            	}
            }

            func main() {
            	library, err := quotes.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	for _, row := range shop.QuotedExamples() {
            		name, named := row.Identity().Name()
            		fmt.Printf("row %d: %q named %v\\n", row.At(), name, named)
            	}
            	evaluated(library, "right", listed{1: "1.5", 3: "9.99", 4: "4"})
            	evaluated(library, "wrong", listed{1: "1.51", 2: "1"})
            	evaluated(library, "lost", lost{})
            	// A row is a type a host can name, and what ended it is what a call would come to.
            	var first quotes.Row[shop.Quoted] = shop.QuotedExamples()[0]
            	err = library.Run(func(r *quotes.Run) error {
            		ended := first.Evaluate(r, lost{})
            		_, answered := ended.Answer()
            		var host *souther.HostError
            		fmt.Printf("ended by the host: %v, answered %v\\n", errors.As(ended.Err(), &host) && host.Err.Error() == "no price list", answered)
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void eachRowIsHeldToWhatTheImplementationAnswersByTheLibrary(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, SHOP, "example.com/quotes", HOST);

        assertThat(said).isEqualTo("""
                row 0: "a price at the scale it was written" named true
                row 1: "" named false
                row 2: "priced, whatever it costs" named true
                row 3: "" named false
                right 0 "a price at the scale it was written" (asserted): kept, answered {"type":"Priced","amount":1.5}; failed false
                right 1 "row 2" (asserted): kept, answered {"type":"Unpriced"}; failed false
                right 2 "priced, whatever it costs" (asserted): kept, answered {"type":"Priced","amount":9.99}; failed false
                right 3 "row 4" (owed): answered {"type":"Priced","amount":4}; failed false
                wrong 0 "a price at the scale it was written" (asserted): not kept, answered {"type":"Priced","amount":1.51}; failed true
                wrong 1 "row 2" (asserted): not kept, answered {"type":"Priced","amount":1}; failed true
                wrong 2 "priced, whatever it costs" (asserted): not kept, answered {"type":"Unpriced"}; failed true
                wrong 3 "row 4" (owed): answered {"type":"Unpriced"}; failed false
                lost 0 "a price at the scale it was written" (asserted): ended: a host implementation failed: no price list; failed true
                lost 1 "row 2" (asserted): ended: a host implementation failed: no price list; failed true
                lost 2 "priced, whatever it costs" (asserted): ended: a host implementation failed: no price list; failed true
                lost 3 "row 4" (owed): ended: a host implementation failed: no price list; failed true
                ended by the host: true, answered false
                """);
    }
}
