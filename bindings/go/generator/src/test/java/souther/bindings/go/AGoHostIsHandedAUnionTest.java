package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A union no declaration names is an interface the library hands over as the type of the member
 * the case it says is belongs to, a declared member holding its handle and a primitive one Go's
 * own value, and a member sum's cases are told apart by the sum's own.
 */
class AGoHostIsHandedAUnionTest {

    private static final String SHOP = """
            module shop exposing ( Money, Free, Paid, Owed, Settled, charge, quantityOf, flaggedOf,
                                   labelOf )

            data Money = Int
                invariant notNegative = value >= 0

            data Free
            data Paid = { amount: Money }
            data Waived = { reason: String }
            data Owed = { amount: Money, overdue: Bool }
            data Settled = Free | Paid | Waived

            behavior charge : (paid: Int) -> Owed | Settled
            let charge (paid) =
                if paid > 1 then Waived { reason = "goodwill" }
                else if paid > 0 then Free
                else Owed { amount = Money(1), overdue = true }

            behavior quantityOf : (paid: Int) -> Int | Free
            let quantityOf (paid) = if paid > 0 then paid else Free

            behavior flaggedOf : (paid: Int) -> Bool | Free
            let flaggedOf (paid) = if paid > 0 then paid > 1 else Free

            behavior labelOf : (paid: Int) -> String | Free
            let labelOf (paid) = if paid > 0 then "paid" else Free
            """;

    private static final String HOST = """
            package main

            import (
            	"fmt"
            	"os"

            	"example.com/unions"
            	"example.com/unions/shop"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func said(charge shop.OwedOrSettled) string {
            	switch it := charge.(type) {
            	case shop.OwedOrSettledOwed:
            		return fmt.Sprintf("owed %d", it.Value.Amount().Value())
            	case shop.OwedOrSettledSettled:
            		switch settled := it.Value.Case().(type) {
            		case shop.SettledFree:
            			return "free"
            		case shop.SettledPaid:
            			return fmt.Sprintf("paid %d", settled.Value.Amount().Value())
            		case shop.SettledKept:
            			return "a case the model keeps"
            		}
            	}
            	panic("a case the model does not have")
            }

            func main() {
            	library, err := unions.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *unions.Run) error {
            		for _, paid := range []int64{0, 1, 2} {
            			fmt.Printf("charge %d: %s\\n", paid, said(made(shop.Charge(r, paid))))
            		}
            		for _, paid := range []int64{0, 3} {
            			quantity := "free"
            			if it, ok := made(shop.QuantityOf(r, paid)).(shop.FreeOrIntInt); ok {
            				quantity = fmt.Sprint(it.Value)
            			}
            			flagged := "free"
            			if it, ok := made(shop.FlaggedOf(r, paid)).(shop.BoolOrFreeBool); ok {
            				flagged = fmt.Sprint(it.Value)
            			}
            			label := "free"
            			if it, ok := made(shop.LabelOf(r, paid)).(shop.FreeOrStringString); ok {
            				label = it.Value
            			}
            			fmt.Printf("%d: %s %s %s\\n", paid, quantity, flagged, label)
            		}
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aUnionIsAnInterfaceOverTheTypesOfItsMembers(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, SHOP, "example.com/unions", HOST);

        assertThat(said).isEqualTo("""
                charge 0: owed 1
                charge 1: free
                charge 2: a case the model keeps
                0: free free free
                3: 3 true paid
                """);
    }
}
