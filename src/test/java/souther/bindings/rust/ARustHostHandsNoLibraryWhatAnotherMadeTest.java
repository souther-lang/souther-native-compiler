package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;
import souther.nativecode.Documents;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What one library made is handed to no computation of another, however it is reached: a value, a
 * value inside another, a list or an optional, a function value, a behavior bound by it, and a
 * behavior bound to an implementation it made. Each is refused before anything reaches the library,
 * as the call's {@code Failure::Foreign}.
 *
 * <p>The types cannot say it. A lifetime says for how long a value is good and not which arena it
 * stands in, and a root run of one library opened inside a root run of another relates the two by
 * lifetimes as two runs of one library are related, so a value of the one is typed as one the other
 * may be handed. Two libraries here are the same model built twice: two files, two runtimes, one
 * binding. Two handles on one of them are one runtime, and what one made the other is handed.
 */
class ARustHostHandsNoLibraryWhatAnotherMadeTest {

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
            use foreign::shop::{self, DiscountFor, DiscountForImplementation, Maybe, Price, Priced, Quote, Twice};
            use foreign::{Failure, HostError, Library, Run};

            struct Off;

            impl DiscountFor for Off {
                fn apply<'run>(&self, _run: &mut Run<'run>, _sku: String) -> Result<i64, HostError> {
                    Ok(1)
                }
            }

            fn said<T: std::fmt::Debug>(called: Result<T, Failure>) -> String {
                match called {
                    Err(Failure::Foreign) => "refused".to_owned(),
                    other => format!("{other:?}"),
                }
            }

            fn main() {
                let mut paths = std::env::args().skip(1);
                // SAFETY: both are the library the binding was generated from, built twice.
                let a = unsafe { Library::load(paths.next().unwrap()) }.expect("a loads");
                let b = unsafe { Library::load(paths.next().unwrap()) }.expect("b loads");
                // SAFETY: the same file as `a`, loaded again.
                let again = unsafe { Library::load(paths.next().unwrap()) }.expect("a loads again");
                let off_a = DiscountForImplementation::new(&a, Off);
                let off_b = DiscountForImplementation::new(&b, Off);
                a.run(|ra| {
                    let price = Price::new(ra, 3).unwrap().into_result().unwrap();
                    b.run(|rb| {
                        println!("value: {}", said(shop::doubled(rb, price)));
                        println!("in a field: {}", said(Priced::new(rb, price).map(|_| ())));
                        println!("in a list: {}", said(shop::counted(rb, &[price])));
                        println!("in an optional: {}", said(Maybe::new(rb, Some(price)).map(|_| ())));
                        let none = Maybe::new(rb, None).unwrap().into_result().unwrap();
                        println!("none: {}", said(shop::orNought(rb, none)));
                        println!("bound by a: {}", said(Twice::new(&a).call(rb, 1)));
                        println!("bound to a's: {}", said(Quote::bind(&b, &off_a).call(rb, "x")));
                        println!("bound to b's: {}", said(Quote::bind(&b, &off_b).call(rb, "x")));
                    })
                    .unwrap();
                    println!("in a: {}", said(shop::doubled(ra, price)));
                    println!("another handle on a: {}", said(Twice::new(&again).call(ra, 2)));
                    println!("bound to a's in a: {}", said(Quote::bind(&again, &off_a).call(ra, "x")));
                })
                .unwrap();
            }
            """;

    private static final String CALLING = """
            use calling::m::{self, FnIntToInt};
            use calling::{Failure, Library};

            fn said<T: std::fmt::Debug>(called: Result<T, Failure>) -> String {
                match called {
                    Err(Failure::Foreign) => "refused".to_owned(),
                    other => format!("{other:?}"),
                }
            }

            fn main() {
                let mut paths = std::env::args().skip(1);
                // SAFETY: both are the library the binding was generated from, built twice.
                let a = unsafe { Library::load(paths.next().unwrap()) }.expect("a loads");
                let b = unsafe { Library::load(paths.next().unwrap()) }.expect("b loads");
                a.run(|ra| {
                    let bump = m::bump(ra).unwrap();
                    b.run(|rb| {
                        let twice = m::twice(rb).unwrap();
                        println!("called: {}", said(bump.call(rb, 1)));
                        println!("handed over: {}", said(twice.call(rb, bump.clone(), 1)));
                        let hosted = FnIntToInt::host(|_, x| Ok(x + 1));
                        println!("a host's: {}", said(twice.call(rb, hosted, 1)));
                    })
                    .unwrap();
                    println!("in a: {}", said(bump.call(ra, 1)));
                })
                .unwrap();
            }
            """;

    @Test
    void aValueOrABindingAnotherLibraryMadeIsRefusedBeforeTheCall(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library a = NativeCompiler.library(Checked.of(List.of(SHOP)),
                into.resolve("a"));
        NativeCompiler.Library b = NativeCompiler.library(Checked.of(List.of(SHOP)),
                into.resolve("b"));
        RustBindings.Generated binding = RustHost.generated(a, into.resolve("binding"), "foreign");

        String said = RustHost.ran(into, binding, "foreign", HOST, List.of(a.library().toString(),
                b.library().toString(), a.library().toString()));

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
                another handle on a: Ok(4)
                bound to a's in a: Ok(9)
                """);
    }

    @Test
    void aFunctionValueAnotherLibraryMadeIsRefusedBeforeTheCall(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library a = Documents.library(Documents.FUNCTIONS, into.resolve("a"));
        NativeCompiler.Library b = Documents.library(Documents.FUNCTIONS, into.resolve("b"));
        RustBindings.Generated binding = RustHost.generated(a, into.resolve("binding"), "calling");

        String said = RustHost.ran(into, binding, "calling", CALLING,
                List.of(a.library().toString(), b.library().toString()));

        assertThat(said).isEqualTo("""
                called: refused
                handed over: refused
                a host's: Ok(3)
                in a: Ok(6)
                """);
    }

    /**
     * A function value the library made cannot be put under another function type. What the
     * library answered is one address, and the code it holds first is of the type it was made
     * for: a call through another type's function would run that code with words it does not
     * take. The address is therefore held where only the library's own conversions reach it, and
     * what a caller sees of a function value is a type of its own, made by the library or from a
     * function of the host's.
     *
     * <p>Two host programs, one for each way to reach it: reading the address out of one type, and
     * putting one into another. Each is refused for that reason and not for a name that is not
     * there, which the reason each names holds them to; the same programs built against a crate that
     * had the address public are the ones this test exists to refuse.
     */
    @Test
    void aFunctionValueCannotBeUnderAnotherFunctionType(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = Documents.library(Documents.FUNCTIONS, into.resolve("a"));
        RustBindings.Generated binding = RustHost.generated(library, into.resolve("binding"),
                "calling");
        String reading = """
                use calling::m::{self, FnIntToInt};
                use calling::Library;

                fn main() {
                    let path = std::env::args().nth(1).unwrap();
                    // SAFETY: the library the binding was generated from.
                    let library = unsafe { Library::load(path) }.unwrap();
                    library.run(|run| {
                        let bump: FnIntToInt<'_> = m::bump(run).unwrap();
                        let FnIntToInt(_held) = bump;
                    }).unwrap();
                }
                """;
        String putting = """
                use calling::m::{self, FnIntToInt, FnIntToFnIntToInt};
                use calling::Library;

                fn main() {
                    let path = std::env::args().nth(1).unwrap();
                    // SAFETY: the library the binding was generated from.
                    let library = unsafe { Library::load(path) }.unwrap();
                    library.run(|run| {
                        let bump: FnIntToInt<'_> = m::bump(run).unwrap();
                        let _ = FnIntToFnIntToInt(bump.0);
                    }).unwrap();
                }
                """;

        // The names exist: the same programs without the reach into the value build, so what is
        // refused below is the reach and not a name.
        RustHost.ran(into, binding, "calling", reading.replace("let FnIntToInt(_held) = bump;",
                "let _ = &bump;"), List.of(library.library().toString()));
        RustHost.ran(into, binding, "calling", putting
                .replace("use calling::m::{self, FnIntToInt, FnIntToFnIntToInt};",
                        "use calling::m::{self, FnIntToInt};")
                .replace("let _ = FnIntToFnIntToInt(bump.0);", "let _ = &bump;"),
                List.of(library.library().toString()));

        assertThat(RustHost.refused(into, binding, "calling", reading)).contains("private fields");
        assertThat(RustHost.refused(into, binding, "calling", putting))
                .contains("field `0` of struct `FnIntToInt` is private");
    }
}
