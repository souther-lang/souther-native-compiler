package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Go application implements the behaviors the library asks a host to implement, binds a
 * behavior to what stands for each behavior it requires, and calls it: a wiring a container would
 * write, in a run, since what stands for a behavior is held until the run it was made in ends.
 *
 * <p>An error an implementation answers comes back out of the call that reached it, and so does a
 * panic, which is raised again where the library has returned and never unwinds through it.
 */
class AGoHostBindsABehaviorToWhatItRequiresTest {

    private static final String CATALOG = """
            module catalog exposing ( Price, priceOf )

            data Price = Int
                invariant notNegative = value >= 0

            behavior priceOf : (sku: String) -> Price
            """;

    private static final String WHOLESALE = """
            module wholesale exposing ( priceOf )

            import catalog ( Price )

            behavior priceOf : (price: Price) -> Int
            """;

    private static final String SHOP = """
            module shop exposing ( Line, quote, total, both, twice, priced : Int, resold : Int,
                                   lineOf )

            import catalog ( Price, priceOf )
            import wholesale

            behavior discountFor : (sku: String) -> Int

            behavior quote : (sku: String, count: Int) -> Int
                depends on priceOf, discountFor
            let quote (sku, count, priceOf, discountFor) =
                priceOf(sku).value * count - discountFor(sku)

            behavior total : (sku: String) -> Int
                depends on quote
            let total (sku, quote) = quote(sku, 2) + 1

            behavior both : (sku: String) -> Int
                depends on quote, priceOf
            let both (sku, quote, priceOf) = quote(sku, 1) + priceOf(sku).value

            behavior twice : (n: Int) -> Int
            let twice (n) = n * 2

            behavior valued : (price: Price) -> Int
            let valued (price) = price.value * 10

            behavior priced = priceOf >-> valued

            behavior resold = catalog.priceOf >-> wholesale.priceOf

            data Line = { sku: String, amount: Int }

            behavior lineOf : (sku: String) -> Line
                depends on priceOf
            let lineOf (sku, priceOf) = Line { sku = sku, amount = priceOf(sku).value }
            """;

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

            	"example.com/billing"
            	"example.com/billing/catalog"
            	"example.com/billing/shop"
            	"example.com/billing/wholesale"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            // listed is a price list, as a container would wire one.
            type listed struct{ factor int64 }

            func (l listed) Apply(r *billing.Run, sku string) (catalog.Price, error) {
            	if sku == "lost" {
            		panic("the price list is gone")
            	}
            	return catalog.NewPrice(r, l.factor*int64(len(sku)))
            }

            type off struct{ amount int64 }

            func (o off) Apply(r *billing.Run, sku string) (int64, error) {
            	if sku == "none" {
            		return 0, errors.New("no discount for that")
            	}
            	return o.amount, nil
            }

            type markedUp struct{}

            func (markedUp) Apply(r *billing.Run, price catalog.Price) (int64, error) {
            	return price.Value() + 1, nil
            }

            func main() {
            	library, err := billing.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *billing.Run) error {
            		prices := catalog.ImplementPriceOf(r, listed{3})
            		discount := shop.ImplementDiscountFor(r, off{1})
            		marked := wholesale.ImplementPriceOf(r, markedUp{})
            		quote := shop.BindQuote(r, prices, discount)
            		fmt.Printf("quote: %d\\n", made(quote.Call(r, "ab", 2)))
            		fmt.Printf("total: %d\\n", made(shop.BindTotal(r, quote).Call(r, "ab")))
            		fmt.Printf("both: %d\\n", made(shop.BindBoth(r, quote, prices).Call(r, "ab")))
            		fmt.Printf("twice: %d\\n", made(shop.Twice(r, 4)))
            		fmt.Printf("priced: %d\\n", made(shop.BindPriced(r, prices).Call(r, "abc")))
            		// Two requirements of one name, from two modules, taken by their places.
            		fmt.Printf("resold: %d\\n", made(shop.BindResold(r, prices, marked).Call(r, "ab")))
            		line := made(shop.BindLineOf(r, prices).Call(r, "abc"))
            		fmt.Printf("line: %s at %d\\n", line.Sku(), line.Amount())
            		// Two implementations of one behavior, bound at two places, each its own.
            		dear := catalog.ImplementPriceOf(r, listed{100})
            		dearer := shop.BindQuote(r, dear, discount)
            		fmt.Printf("each its own: %d %d\\n", made(quote.Call(r, "ab", 1)), made(dearer.Call(r, "ab", 1)))
            		_, err := quote.Call(r, "none", 1)
            		var host *souther.HostError
            		if errors.As(err, &host) {
            			fmt.Printf("failed: %v\\n", host.Err)
            		}
            		func() {
            			defer func() { fmt.Printf("panicked: %v\\n", recover()) }()
            			_, _ = quote.Call(r, "lost", 1)
            		}()
            		fmt.Printf("still: %d\\n", made(quote.Call(r, "ab", 2)))
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aGoHostImplementsBindsAndCallsABehavior(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(
                Checked.of(List.of(CATALOG, WHOLESALE, SHOP)), into.resolve("native"));
        var binding = GoHost.generated(library, into.resolve("binding"), "example.com/billing");

        String said = GoHost.ran(into, binding, "example.com/billing", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                quote: 11
                total: 12
                both: 11
                twice: 8
                priced: 90
                resold: 7
                line: abc at 9
                each its own: 5 199
                failed: no discount for that
                panicked: the price list is gone
                still: 11
                """);
    }
}
