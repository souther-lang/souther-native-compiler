package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.Generated;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Go application uses a model through the packages generated from its library's manifest, as the
 * PHP and the Rust ones do: it loads the library by path, opens a run, builds values through their
 * constructors, calls behaviors, reads a published value, and reads a value out of its external
 * form.
 *
 * <p>What Rust checks when it is built, Go checks as the application runs, which the runtime
 * module's own tests hold. What is left to a run is a second root run of one library, refused where
 * it is opened.
 */
class AGoHostCallsALibraryThroughItsBindingTest {

    private static final String SHOP = """
            module shop exposing ( Money, Line, Order, Free, Paid, Owed, Settled, Outcome,
                                   settle, owing, stillOwing : Int, squared, standardPrice )

            data Money = Int
                invariant notNegative = value >= 0

            data Line = { price: Money, quantity: Int, note: String? }
                invariant some = quantity > 0

            data Order = { line: Line, placed: Bool }

            data Free
            data Paid = { amount: Money }
            data Waived = { reason: String }
            data Owed = { amount: Money, overdue: Bool }
            data Settled = Free | Paid | Waived
            data Outcome = Settled | Owed

            behavior settle : (line: Line, paid: Int) -> Outcome
            let settle (line, paid) = {
                let due = line.price.value * line.quantity
                if due == 0 then Free
                else if paid > due then Waived { reason = "overpaid" }
                else if paid == due then Paid { amount = Money(due) }
                else Owed { amount = Money(due - paid), overdue = paid == 0 }
            }

            behavior owing : (outcome: Outcome) -> Int
            let owing (outcome) = match outcome with
                | Owed as o -> o.amount.value
                | Settled -> 0

            behavior stillOwing = settle >-> owing

            behavior squared : (n: Int) -> Int
            let squared (n) = n * n

            let standardPrice = Money(3)
            """;

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

            	"example.com/acme"
            	"example.com/acme/shop"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            	"github.com/raoh-project/raoh-go"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func code(err error) string {
            	if issues, ok := errors.AsType[*raoh.Issues](err); ok {
            		return issues.All()[0].Code()
            	}
            	return "failure: " + err.Error()
            }

            func main() {
            	library, err := acme.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *acme.Run) error {
            		price := made(shop.NewMoney(r, 3))
            		line := made(shop.NewLine(r, price, 2, souther.Some("gift")))
            		note, _ := line.Note().Get()
            		fmt.Printf("line: %d x %d, %q\\n", line.Price().Value(), line.Quantity(), note)
            		plain := made(shop.NewLine(r, price, 1, souther.None[string]()))
            		_, some := plain.Note().Get()
            		fmt.Printf("plain: %v\\n", some)
            		_, err := shop.NewLine(r, price, 0, souther.None[string]())
            		fmt.Printf("none: %s\\n", code(err))
            		_, err = shop.NewMoney(r, -1)
            		fmt.Printf("negative: %s\\n", code(err))

            		for _, paid := range []int64{0, 2, 6, 7} {
            			outcome := made(shop.Settle(r, line, paid))
            			fmt.Printf("settle %d: owing %d\\n", paid, made(shop.Owing(r, outcome)))
            		}

            		fmt.Printf("still owing: %d\\n", made(shop.StillOwing(r, line, 1)))
            		fmt.Printf("squared: %d\\n", made(shop.Squared(r, 7)))
            		fmt.Printf("standard: %d\\n", made(shop.StandardPrice(r)).Value())

            		order := made(shop.NewOrder(r, line, true))
            		fmt.Printf("encoded: %s\\n", order.Encode())
            		read := made(shop.DecodeOrder(r, []byte(order.Encode())))
            		fmt.Printf("read back: %d placed %v\\n", read.Line().Quantity(), read.Placed())
            		_, err = shop.DecodeLine(r, []byte(`{"price": -1, "note": 3}`))
            		issues, _ := errors.AsType[*raoh.Issues](err)
            		for _, issue := range issues.All() {
            			fmt.Printf("wrong: %s at %v\\n", issue.Code(), issue.Path().Segments())
            		}
            		_, err = shop.DecodeLine(r, []byte("{"))
            		fmt.Printf("broken: %s\\n", code(err))

            		// A run inside this one: what was made outside is handed to a computation
            		// inside, and what is made inside is answered out as a number.
            		var inside int64
            		err = r.Scope(func(inner *acme.Run) error {
            			inside = made(shop.Owing(inner, made(shop.Settle(inner, line, 1))))
            			return nil
            		})
            		fmt.Printf("inside: %d, then %d\\n", inside, line.Quantity())

            		// A second root run of the library while this one is open.
            		fmt.Printf("again: %v\\n", library.Run(func(*acme.Run) error { return nil }))
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *acme.Run) error {
            		fmt.Printf("after: %d\\n", made(shop.Squared(r, 3)))
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    private static final String ANSWERED = """
            line: 3 x 2, "gift"
            plain: false
            none: invariant_violation
            negative: invariant_violation
            settle 0: owing 6
            settle 2: owing 4
            settle 6: owing 0
            settle 7: owing 0
            still owing: 5
            squared: 49
            standard: 3
            encoded: {"line":{"price":3,"quantity":2,"note":"gift"},"placed":true}
            read back: 2 placed true
            wrong: out_of_range at [price]
            wrong: missing_field at [quantity]
            wrong: type_mismatch at [note]
            broken: invalid_format
            inside: 5, then 2
            again: a run of this library is already open on this thread; a run inside it is opened from it with Scope
            after: 9
            """;

    @Test
    void aGoApplicationUsesTheModelThroughItsGeneratedPackages(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into.resolve("native"));
        Generated binding = GoHost.generated(library, into.resolve("acme"), "example.com/acme");

        String said = GoHost.ran(into, binding, "example.com/acme", HOST, List.of(library.library().toString()));

        assertThat(said).isEqualTo(ANSWERED);
    }
}
