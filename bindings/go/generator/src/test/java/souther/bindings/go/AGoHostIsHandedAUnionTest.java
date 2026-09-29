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
            module shop exposing ( Money, Free, Paid, Owed, Settled, charge, chargedFree,
                                   quantityOf, flaggedOf, labelOf, doubledQuantity, rated )

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

            behavior chooseCharge : (paid: Int) -> Owed | Settled

            behavior chargedFree : (paid: Int) -> Bool
                depends on chooseCharge
            let chargedFree (paid, chooseCharge) = match chooseCharge(paid) with
                | Free -> true
                | Paid -> false
                | Waived -> false
                | Owed -> false

            behavior quantityOf : (paid: Int) -> Int | Free
            let quantityOf (paid) = if paid > 0 then paid else Free

            behavior flaggedOf : (paid: Int) -> Bool | Free
            let flaggedOf (paid) = if paid > 0 then paid > 1 else Free

            behavior labelOf : (paid: Int) -> String | Free
            let labelOf (paid) = if paid > 0 then "paid" else Free

            behavior chooseQuantity : (paid: Int) -> Int | Free

            behavior doubledQuantity : (paid: Int) -> Int
                depends on chooseQuantity
            let doubledQuantity (paid, chooseQuantity) = match chooseQuantity(paid) with
                | Int as n -> n * 2
                | Free -> -1

            behavior rated : (amount: Decimal) -> Decimal | Free
            let rated (amount) = if amount == 0m then Free else amount
            """;

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

            	"example.com/unions"
            	"example.com/unions/shop"

            	"github.com/raoh-project/raoh-go"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            // freeFromTwo chooses a charge as a host would: free from two paid, owed before.
            type freeFromTwo struct{}

            func (freeFromTwo) Apply(r *unions.Run, paid int64) (shop.OwedOrSettled, error) {
            	if paid >= 2 {
            		free, err := shop.NewFree(r)
            		if err != nil {
            			return nil, err
            		}
            		return shop.OwedOrSettledSettled{Value: shop.SettledFromFree(free)}, nil
            	}
            	amount, err := shop.NewMoney(r, 1)
            	if err != nil {
            		return nil, err
            	}
            	owed, err := shop.NewOwed(r, amount, true)
            	if err != nil {
            		return nil, err
            	}
            	return shop.OwedOrSettledOwed{Value: owed}, nil
            }

            // halved chooses a quantity, and has nothing to say where there is none.
            type halved struct{}

            func (halved) Apply(r *unions.Run, paid int64) (shop.FreeOrInt, error) {
            	if paid > 0 {
            		return shop.FreeOrIntInt{Value: paid / 2}, nil
            	}
            	return nil, errors.New("no run to make one in")
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
            		for _, amount := range []string{"0", "1.25"} {
            			switch it := made(shop.Rated(r, raoh.MustDecimal(amount))).(type) {
            			case shop.DecimalOrFreeDecimal:
            				fmt.Printf("rated: %se-%d\\n", it.Value.Unscaled(), it.Value.Scale())
            			case shop.DecimalOrFreeFree:
            				fmt.Printf("rated: free\\n")
            			}
            		}
            		charged := shop.BindChargedFree(r, shop.ImplementChooseCharge(r, freeFromTwo{}))
            		fmt.Printf("charged free: %v %v\\n", made(charged.Call(r, 1)), made(charged.Call(r, 2)))
            		doubled := shop.BindDoubledQuantity(r, shop.ImplementChooseQuantity(r, halved{}))
            		fmt.Printf("doubled: %d\\n", made(doubled.Call(r, 9)))
            		_, err := doubled.Call(r, 0)
            		fmt.Printf("doubled free: %v\\n", err)
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
                rated: free
                rated: 125e-2
                charged free: false true
                doubled: 8
                doubled free: a host implementation failed: no run to make one in
                """);
    }
}
