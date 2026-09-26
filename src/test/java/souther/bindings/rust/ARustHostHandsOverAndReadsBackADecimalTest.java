package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A {@code Decimal} is its integer and its scale to a Rust host, as it is to a PHP one: handed over
 * and read back through the runtime, its scale kept as it was, in a field, an optional and a list
 * as well as on its own. An integer the library would end the process on is refused by the runtime
 * crate before anything reaches the library.
 */
class ARustHostHandsOverAndReadsBackADecimalTest {

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
            use pricing_binding::pricing::{self, Discount, Priced};
            use pricing_binding::{Decimal, Library};

            fn shown(d: &Decimal) -> String {
                format!("{}e-{}", d.unscaled(), d.scale())
            }

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        let amount = Decimal::new("1999", 2).unwrap();
                        let rate = Decimal::new("108", 2).unwrap();
                        println!("taxed: {}", shown(&pricing::taxed(run, &amount, &rate).unwrap()));
                        let xs = [Decimal::of(15, 1), Decimal::of(150, 2), Decimal::of(-3, 0)];
                        println!("total: {}", shown(&pricing::total(run, &xs).unwrap()));

                        let priced = Priced::new(run, &Decimal::new("-0150", 3).unwrap(), "back")
                            .unwrap().into_result().unwrap();
                        println!("priced: {} {}", shown(&priced.amount()), priced.note());
                        println!("priced encoded: {}", priced.encode());

                        let some = Discount::new(run, Some(&rate)).unwrap().into_result().unwrap();
                        let none = Discount::new(run, None).unwrap().into_result().unwrap();
                        println!("rate: {:?} {:?}", some.rate().map(|it| shown(&it)), none.rate());
                        println!("or nought: {} {}",
                            shown(&pricing::orNought(run, some).unwrap()),
                            shown(&pricing::orNought(run, none).unwrap()));
                    })
                    .unwrap();
                println!("refused: {}", Decimal::new("1.5", 0).unwrap_err());
                println!("same amount: {}", Decimal::of(15, 1) == Decimal::of(150, 2));
            }
            """;

    @Test
    void aDecimalIsItsIntegerAndItsScale(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(PRICING)), into.resolve("native"));
        RustBindings.Generated binding =
                RustHost.generated(library, into.resolve("binding"), "pricing-binding");

        String said = RustHost.ran(into, binding, "pricing-binding", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                taxed: 2159e-2
                total: 0e-2
                priced: -150e-3 back
                priced encoded: {"amount":-0.15,"note":"back"}
                rate: Some("108e-2") None
                or nought: 108e-2 0e-0
                refused: a Decimal's integer is an optional '-' and ASCII digits, and not '1.5'
                same amount: false
                """);
    }
}
