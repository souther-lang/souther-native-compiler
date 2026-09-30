package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Rust application uses a model through the crate generated from its library's manifest, as the
 * PHP one does through its binding: it loads the library by path, opens a run, builds values
 * through their constructors, calls behaviors, reads a published value, tells a sum's cases apart,
 * and reads a value out of its external form.
 *
 * <p>What PHP checks while the application runs, Rust checks when it is built: a value used after
 * its run or on another thread does not compile, which the runtime crate's own tests hold. What is
 * left to a run is a second root run of one library, refused where it is opened.
 */
class ARustHostCallsALibraryThroughItsBindingTest {

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
            use acme::shop::{self, Line, Money, Order, Outcome, OutcomeCase};
            use acme::{Construction, Library, Reading};

            fn said(outcome: Outcome<'_>) -> String {
                match outcome.case() {
                    OutcomeCase::Free(_) => "free".to_owned(),
                    OutcomeCase::Paid(paid) => format!("paid {}", paid.amount().value()),
                    OutcomeCase::Owed(owed) => {
                        format!("owed {} overdue {}", owed.amount().value(), owed.overdue())
                    }
                    OutcomeCase::Kept(_) => "a case the model keeps".to_owned(),
                }
            }

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        let price = Money::new(run, 3).unwrap().into_result().unwrap();
                        let line = Line::new(run, price, 2, Some("gift")).unwrap().into_result().unwrap();
                        println!("line: {} x {}, {:?}", line.price().value(), line.quantity(), line.note());
                        let plain = Line::new(run, price, 1, None).unwrap().into_result().unwrap();
                        println!("plain: {:?}", plain.note());
                        match Line::new(run, price, 0, None).unwrap() {
                            Construction::Value(_) => println!("none: built"),
                            Construction::Rejected(issue) => println!("none: {}", issue.code()),
                        }
                        match Money::new(run, -1).unwrap() {
                            Construction::Value(_) => println!("negative: built"),
                            Construction::Rejected(issue) => println!("negative: {}", issue.code()),
                        }

                        for paid in [0, 2, 6, 7] {
                            let outcome = shop::settle(run, line, paid).unwrap();
                            let owing = shop::owing(run, outcome).unwrap();
                            println!("settle {paid}: {}, owing {owing}", said(outcome));
                        }
                        let nothing = Money::new(run, 0).unwrap().into_result().unwrap();
                        let free = Line::new(run, nothing, 1, None).unwrap().into_result().unwrap();
                        println!("free: {}", said(shop::settle(run, free, 0).unwrap()));

                        println!("still owing: {}", shop::stillOwing(run, line, 1).unwrap());
                        println!("squared: {}", shop::squared(run, 7).unwrap());
                        println!("standard: {}", shop::standardPrice(run).unwrap().value());

                        let order = Order::new(run, line, true).unwrap().into_result().unwrap();
                        println!("encoded: {}", order.encode());
                        match Order::decode(run, &order.encode()).unwrap() {
                            Reading::Value(read) => println!(
                                "read back: {} placed {}", read.line().quantity(), read.placed()),
                            Reading::Issues(issues) => println!("read back: {issues:?}"),
                        }
                        match Line::decode(run, r#"{"price": -1, "note": 3}"#).unwrap() {
                            Reading::Value(_) => println!("wrong: read"),
                            Reading::Issues(issues) => {
                                for issue in issues.iter() {
                                    println!("wrong: {} at {:?}", issue.code(), issue.path().segments());
                                }
                            }
                        }
                        match Line::decode(run, "{").unwrap() {
                            Reading::Value(_) => println!("broken: read"),
                            Reading::Issues(issues) => println!("broken: {}", issues.iter().next().unwrap().code()),
                        }

                        // A run inside this one: what was made outside is handed to a computation
                        // inside, and what is made inside is answered out as a number.
                        let inside = run.scope(|inner| {
                            let outcome = shop::settle(inner, line, 1).unwrap();
                            shop::owing(inner, outcome).unwrap()
                        });
                        println!("inside: {inside}, then {}", line.quantity());

                        // A second root run of the library while this one is open.
                        println!("again: {}", library.run(|_| ()).unwrap_err());
                    })
                    .unwrap();
                println!("after: {}", library.run(|run| shop::squared(run, 3).unwrap()).unwrap());
            }
            """;

    private static final String ANSWERED = """
            line: 3 x 2, Some("gift")
            plain: None
            none: invariant_violation
            negative: invariant_violation
            settle 0: owed 6 overdue true, owing 6
            settle 2: owed 4 overdue false, owing 4
            settle 6: paid 6, owing 0
            settle 7: a case the model keeps, owing 0
            free: free
            still owing: 5
            squared: 49
            standard: 3
            encoded: {"line":{"price":3,"quantity":2,"note":"gift"},"placed":true}
            read back: 2 placed true
            wrong: out_of_range at ["price"]
            wrong: missing_field at ["quantity"]
            wrong: type_mismatch at ["note"]
            broken: invalid_format
            inside: 5, then 2
            again: a run of this library is already open on this thread; a run inside it is opened from it with `scope`
            after: 9
            """;

    @Test
    void aRustApplicationUsesTheModelThroughItsGeneratedCrate(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(SHOP)), into.resolve("native"));
        Generated binding = RustHost.generated(library, into.resolve("acme"), "acme");

        String said = RustHost.ran(into, binding, "acme", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo(ANSWERED);
    }
}
