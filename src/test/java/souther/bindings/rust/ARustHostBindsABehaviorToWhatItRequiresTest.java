package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.Generated;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Rust host implements a behavior the model asks a host for as a trait, makes an implementation
 * into what the library calls it through, binds a behavior requiring it to that, and calls it: the
 * way the PHP binding's classes do, and the JVM backend's host.
 *
 * <p>What binding checks is what Rust's types check: a missing implementation does not compile,
 * and never answers {@code INJECTION_UNBOUND}. What an implementation answers is made in the run of
 * the call that reached it. A failure it answers comes back out of that call as the host's own, and
 * a panic in it is raised again there, having never unwound through the library.
 */
class ARustHostBindsABehaviorToWhatItRequiresTest {

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
            use billing::catalog::{Price, PriceOf, PriceOfImplementation};
            use billing::shop::{
                Both, DiscountFor, DiscountForImplementation, LineOf, Priced, Quote, Resold, Total, Twice,
            };
            use billing::wholesale;
            use billing::{Failure, HostError, Library, Run};
            use std::panic::{self, AssertUnwindSafe};

            /// A price list with a dependency of its own, as a container would wire one.
            struct Listed(i64);

            impl PriceOf for Listed {
                fn apply<'run>(&self, run: &mut Run<'run>, sku: String) -> Result<Price<'run>, HostError> {
                    if sku == "lost" {
                        panic!("the price list is gone");
                    }
                    let length = i64::try_from(sku.len())?;
                    Ok(Price::new(run, self.0 * length)?.into_result().map_err(|it| it.code().to_owned())?)
                }
            }

            struct Off(i64);

            impl DiscountFor for Off {
                fn apply<'run>(&self, _run: &mut Run<'run>, sku: String) -> Result<i64, HostError> {
                    if sku == "none" { Err("no discount for that".into()) } else { Ok(self.0) }
                }
            }

            struct MarkedUp;

            impl wholesale::PriceOf for MarkedUp {
                fn apply<'run>(&self, _run: &mut Run<'run>, price: Price<'run>) -> Result<i64, HostError> {
                    Ok(price.value() + 1)
                }
            }

            fn main() {
                panic::set_hook(Box::new(|_| {}));
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                let prices = PriceOfImplementation::new(&library, Listed(3));
                let off = DiscountForImplementation::new(&library, Off(1));
                let marked = wholesale::PriceOfImplementation::new(&library, MarkedUp);
                let quote = Quote::bind(&library, &prices, &off);
                library
                    .run(|run| {
                        println!("quote: {}", quote.call(run, "ab", 2).unwrap());
                        println!("total: {}", Total::bind(&library, &quote).call(run, "ab").unwrap());
                        println!("both: {}", Both::bind(&library, &quote, &prices).call(run, "ab").unwrap());
                        println!("twice: {}", Twice::new(&library).call(run, 4).unwrap());
                        println!("priced: {}", Priced::bind(&library, &prices).call(run, "abc").unwrap());
                        // Two requirements of one name, from two modules, taken by their places.
                        let resold = Resold::bind(&library, &prices, &marked);
                        println!("resold: {}", resold.call(run, "ab").unwrap());
                        let line = LineOf::bind(&library, &prices).call(run, "abc").unwrap();
                        println!("line: {} at {}", line.sku(), line.amount());
                        // Two implementations of one behavior, bound at two places, each its own.
                        let dear = PriceOfImplementation::new(&library, Listed(100));
                        let dearer = Quote::bind(&library, &dear, &off);
                        println!("each its own: {} {}", quote.call(run, "ab", 1).unwrap(),
                            dearer.call(run, "ab", 1).unwrap());
                        match quote.call(run, "none", 1) {
                            Err(Failure::Host(failure)) => println!("failed: {failure}"),
                            other => println!("failed: {:?}", other.map(|_| ())),
                        }
                        let panicked = panic::catch_unwind(AssertUnwindSafe(|| quote.call(run, "lost", 1)));
                        let payload = panicked.expect_err("the panic comes back out of the call");
                        println!("panicked: {}", payload.downcast_ref::<&str>().unwrap());
                        println!("still: {}", quote.call(run, "ab", 2).unwrap());
                    })
                    .unwrap();
            }
            """;

    @Test
    void aRustHostImplementsBindsAndCallsABehavior(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(
                Checked.of(List.of(CATALOG, WHOLESALE, SHOP)), into.resolve("native"));
        Generated binding =
                RustHost.generated(library, into.resolve("binding"), "billing");

        String said = RustHost.ran(into, binding, "billing", HOST,
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
