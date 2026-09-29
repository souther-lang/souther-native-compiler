package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Decimal is Raoh's, its integer and its scale as the language says them, and a scale is kept as
 * it was written: 1.50 is 150 at scale 2 and 1.5 is 15 at scale 1.
 */
class AGoHostHandsOverAndReadsBackADecimalTest {

    private static final String PRICING = """
            module pricing exposing ( Priced, Discount, taxed, total, orNought )

            data Priced = { amount: Decimal, note: String }

            data Discount = { rate: Decimal? }

            behavior taxed : (amount: Decimal, rate: Decimal) -> Decimal
            let taxed (amount, rate) = Decimal.round(2, HALF_UP, amount * rate)

            behavior total : (xs: List<Decimal>) -> Decimal
            let total (xs) = List.sum(xs)

            behavior orNought : (d: Discount) -> Decimal
            let orNought (d) = match d.rate with
                | Some x -> x
                | None -> 0m
            """;

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"

            	"example.com/pricingbinding"
            	"example.com/pricingbinding/pricing"

            	"github.com/raoh-project/raoh-go"
            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func shown(d raoh.Decimal) string {
            	return fmt.Sprintf("%se-%d", d.Unscaled(), d.Scale())
            }

            func main() {
            	library, err := pricingbinding.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *pricingbinding.Run) error {
            		amount := raoh.MustDecimal("19.99")
            		rate := raoh.MustDecimal("1.08")
            		fmt.Printf("taxed: %s\\n", shown(made(pricing.Taxed(r, amount, rate))))
            		xs := []raoh.Decimal{raoh.MustDecimal("1.5"), raoh.MustDecimal("1.50"), raoh.MustDecimal("-3")}
            		fmt.Printf("total: %s\\n", shown(made(pricing.Total(r, xs))))

            		priced := made(pricing.NewPriced(r, raoh.MustDecimal("-0.150"), "back"))
            		fmt.Printf("priced: %s %s\\n", shown(priced.Amount()), priced.Note())
            		fmt.Printf("priced encoded: %s\\n", priced.Encode())

            		some := made(pricing.NewDiscount(r, souther.Some(rate)))
            		none := made(pricing.NewDiscount(r, souther.None[raoh.Decimal]()))
            		given, present := some.Rate().Get()
            		_, absent := none.Rate().Get()
            		fmt.Printf("rate: %s %v %v\\n", shown(given), present, absent)
            		fmt.Printf("or nought: %s %s\\n", shown(made(pricing.OrNought(r, some))), shown(made(pricing.OrNought(r, none))))
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            	fmt.Printf("same amount: %v\\n", raoh.MustDecimal("1.5").Cmp(raoh.MustDecimal("1.50")) == 0)
            }
            """;

    @Test
    void aDecimalIsItsIntegerAndItsScale(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, PRICING, "example.com/pricingbinding", HOST);

        assertThat(said).isEqualTo("""
                taxed: 2159e-2
                total: 0e-2
                priced: -150e-3 back
                priced encoded: {"amount":-0.15,"note":"back"}
                rate: 108e-2 true false
                or nought: 108e-2 0e-0
                same amount: true
                """);
    }
}
