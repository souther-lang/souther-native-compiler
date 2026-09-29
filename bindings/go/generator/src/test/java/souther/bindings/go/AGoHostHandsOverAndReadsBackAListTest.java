package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/** A list is a slice, handed over and read back, of what it holds at every depth. */
class AGoHostHandsOverAndReadsBackAListTest {

    private static final String CART = """
            module cart exposing ( OrderLine, Order, PricedCart, Notes, Grid, totalOf, echoed )

            data OrderLine = { sku: String, quantity: Int, unitPrice: Int }
                invariant quantity >= 1

            data Order = { lines: List<OrderLine> }

            data PricedCart = { lines: List<OrderLine>, total: Int }
                invariant List.length(lines) >= 1

            data Notes = { said: List<Option<String>> }

            data Grid = { rows: List<List<Int>> }

            partial let sumFrom (acc: Int, xs: List<OrderLine>, i: Int): Int =
                match List.get(i, xs) with
                    | Some x -> sumFrom(acc + x.quantity * x.unitPrice, xs, i + 1)
                    | None -> acc

            behavior totalOf : (order: Order) -> Int
            let totalOf (order) = sumFrom(0, order.lines, 0)

            behavior echoed : (lines: List<OrderLine>) -> List<OrderLine>
            let echoed (lines) = lines
            """;

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

            	"example.com/cartbinding"
            	"example.com/cartbinding/cart"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            	"github.com/raoh-project/raoh-go"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func skus(lines []cart.OrderLine) []string {
            	out := []string{}
            	for _, it := range lines {
            		out = append(out, it.Sku())
            	}
            	return out
            }

            func main() {
            	library, err := cartbinding.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *cartbinding.Run) error {
            		apple := made(cart.NewOrderLine(r, "apple", 2, 30))
            		pear := made(cart.NewOrderLine(r, "pear", 1, 50))
            		order := made(cart.NewOrder(r, []cart.OrderLine{apple, pear}))
            		fmt.Printf("total: %d\\n", made(cart.TotalOf(r, order)))
            		fmt.Printf("lines: %v\\n", skus(order.Lines()))

            		echoed := made(cart.Echoed(r, []cart.OrderLine{pear, apple, pear}))
            		fmt.Printf("echoed: %v\\n", skus(echoed))
            		fmt.Printf("echoed none: %d\\n", len(made(cart.Echoed(r, nil))))

            		_, err := cart.NewPricedCart(r, nil, 0)
            		issues, _ := errors.AsType[*raoh.Issues](err)
            		fmt.Printf("empty cart: %s\\n", issues.All()[0].Code())

            		said := []souther.Option[string]{souther.Some("gift"), souther.None[string](), souther.Some("")}
            		notes := made(cart.NewNotes(r, said))
            		for _, it := range notes.Said() {
            			value, some := it.Get()
            			fmt.Printf("notes: %v %q\\n", some, value)
            		}

            		grid := made(cart.NewGrid(r, [][]int64{{1, 2, 3}, {}, {4}}))
            		fmt.Printf("grid: %v\\n", grid.Rows())
            		fmt.Printf("grid encoded: %s\\n", grid.Encode())
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aListIsASliceHandedOverAndReadBack(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, CART, "example.com/cartbinding", HOST);

        assertThat(said).isEqualTo("""
                total: 110
                lines: [apple pear]
                echoed: [pear apple pear]
                echoed none: 0
                empty cart: invariant_violation
                notes: true "gift"
                notes: false ""
                notes: true ""
                grid: [[1 2 3] [] [4]]
                grid encoded: {"rows":[[1,2,3],[],[4]]}
                """);
    }
}
