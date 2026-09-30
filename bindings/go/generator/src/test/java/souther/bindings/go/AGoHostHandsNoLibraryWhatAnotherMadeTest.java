package souther.bindings.go;

import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;

import souther.nativecode.Documents;
import souther.nativecode.NativeCompiler;


import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What one library made is an address in its arena, which another library would read as its own.
 * Two libraries built from one program have the same binding and are two runtimes, so a value, a
 * function value or what stands for a behavior that one made is refused where it is handed to the
 * other, before the call is made. Two handles on one file are one runtime.
 */
class AGoHostHandsNoLibraryWhatAnotherMadeTest {

    private static final String SHOP = """
            module shop exposing ( Price, Priced, Maybe, doubled, counted, orNought, twice, quote )

            data Price = Int
                invariant notNegative = value >= 0

            data Priced = { price: Price }

            data Maybe = { price: Price? }

            behavior doubled : (price: Price) -> Int
            let doubled (price) = price.value * 2

            behavior counted : (prices: List<Price>) -> Int
            let counted (prices) = List.length(prices)

            behavior orNought : (it: Maybe) -> Int
            let orNought (it) = match it.price with
                | Some p -> p.value
                | None -> 0

            behavior twice : (n: Int) -> Int
            let twice (n) = n * 2

            behavior discountFor : (sku: String) -> Int

            behavior quote : (sku: String) -> Int
                depends on discountFor
            let quote (sku, discountFor) = 10 - discountFor(sku)
            """;

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

            	"example.com/foreign"
            	"example.com/foreign/shop"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            type off struct{}

            func (off) Apply(r *foreignbinding.Run, sku string) (int64, error) { return 1, nil }

            func said[T any](value T, err error) string {
            	switch {
            	case errors.Is(err, souther.ErrForeignHandle):
            		return "refused"
            	case err != nil:
            		return "error: " + err.Error()
            	}
            	return fmt.Sprintf("Ok(%v)", value)
            }

            func main() {
            	a := made(foreignbinding.Load(os.Args[1]))
            	b := made(foreignbinding.Load(os.Args[2]))
            	// The same file as a, loaded again.
            	again := made(foreignbinding.Load(os.Args[1]))
            	err := a.Run(func(ra *foreignbinding.Run) error {
            		price := made(shop.NewPrice(ra, 3))
            		offA := shop.ImplementDiscountFor(ra, off{})
            		quoteA := shop.BindQuote(ra, offA)
            		err := b.Run(func(rb *foreignbinding.Run) error {
            			fmt.Printf("value: %s\\n", said(shop.Doubled(rb, price)))
            			fmt.Printf("in a field: %s\\n", said(shop.NewPriced(rb, price)))
            			fmt.Printf("in a list: %s\\n", said(shop.Counted(rb, []shop.Price{price})))
            			fmt.Printf("in an optional: %s\\n", said(shop.NewMaybe(rb, souther.Some(price))))
            			none := made(shop.NewMaybe(rb, souther.None[shop.Price]()))
            			fmt.Printf("none: %s\\n", said(shop.OrNought(rb, none)))
            			fmt.Printf("bound by a: %s\\n", said(quoteA.Call(rb, "x")))
            			fmt.Printf("bound to a's: %s\\n", said(shop.BindQuote(rb, offA).Call(rb, "x")))
            			offB := shop.ImplementDiscountFor(rb, off{})
            			fmt.Printf("bound to b's: %s\\n", said(shop.BindQuote(rb, offB).Call(rb, "x")))
            			return nil
            		})
            		if err != nil {
            			return err
            		}
            		fmt.Printf("in a: %s\\n", said(shop.Doubled(ra, price)))
            		fmt.Printf("another handle on a: %v\\n", errors.Is(again.Run(func(*foreignbinding.Run) error { return nil }), souther.ErrAlreadyRunning))
            		fmt.Printf("bound to a's in a: %s\\n", said(shop.BindQuote(ra, offA).Call(ra, "x")))
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    private static final String CALLING = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"

            	"example.com/foreign"
            	"example.com/foreign/m"

            	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
            )

            func made[T any](value T, err error) T {
            	if err != nil {
            		panic(err)
            	}
            	return value
            }

            func said[T any](value T, err error) string {
            	switch {
            	case errors.Is(err, souther.ErrForeignHandle):
            		return "refused"
            	case err != nil:
            		return "error: " + err.Error()
            	}
            	return fmt.Sprintf("Ok(%v)", value)
            }

            func main() {
            	a := made(foreignbinding.Load(os.Args[1]))
            	b := made(foreignbinding.Load(os.Args[2]))
            	err := a.Run(func(ra *foreignbinding.Run) error {
            		bump := made(m.Bump(ra))
            		err := b.Run(func(rb *foreignbinding.Run) error {
            			twice := made(m.Twice(rb))
            			fmt.Printf("called: %s\\n", said(bump.Call(rb, 1)))
            			fmt.Printf("handed over: %s\\n", said(twice.Call(rb, bump, 1)))
            			hosted := m.HostFnIntToInt(func(r *foreignbinding.Run, x int64) (int64, error) { return x + 1, nil })
            			fmt.Printf("a host's: %s\\n", said(twice.Call(rb, hosted, 1)))
            			return nil
            		})
            		if err != nil {
            			return err
            		}
            		fmt.Printf("in a: %s\\n", said(bump.Call(ra, 1)))
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aValueOrABindingAnotherLibraryMadeIsRefusedBeforeTheCall(@TempDir Path into)
            throws Exception {
        TestLibrary a = SoutherBindingTest.compile(into.resolve("a"), SHOP);
        TestLibrary b = SoutherBindingTest.compile(into.resolve("b"), SHOP);
        Generated binding = GoHost.generated(a, into.resolve("binding"), "example.com/foreignbinding");

        String said = GoHost.ran(into, binding, "example.com/foreignbinding",
                HOST.replace("\"example.com/foreign\"", "\"example.com/foreignbinding\"")
                        .replace("\"example.com/foreign/shop\"", "\"example.com/foreignbinding/shop\""),
                List.of(a.library().toString(), b.library().toString(), a.library().toString()));

        assertThat(said).isEqualTo("""
                value: refused
                in a field: refused
                in a list: refused
                in an optional: refused
                none: Ok(0)
                bound by a: refused
                bound to a's: refused
                bound to b's: Ok(9)
                in a: Ok(6)
                another handle on a: true
                bound to a's in a: Ok(9)
                """);
    }

    @Test
    void aFunctionValueAnotherLibraryMadeIsRefusedBeforeTheCall(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library a = Documents.library(Documents.FUNCTIONS, into.resolve("a"));
        NativeCompiler.Library b = Documents.library(Documents.FUNCTIONS, into.resolve("b"));
        Generated binding = GoHost.generated(a, into.resolve("binding"), "example.com/foreignbinding");

        String said = GoHost.ran(into, binding, "example.com/foreignbinding",
                CALLING.replace("\"example.com/foreign\"", "\"example.com/foreignbinding\"")
                        .replace("\"example.com/foreign/m\"", "\"example.com/foreignbinding/m\""),
                List.of(a.library().toString(), b.library().toString()));

        assertThat(said).isEqualTo("""
                called: refused
                handed over: refused
                a host's: Ok(3)
                in a: Ok(6)
                """);
    }
}
