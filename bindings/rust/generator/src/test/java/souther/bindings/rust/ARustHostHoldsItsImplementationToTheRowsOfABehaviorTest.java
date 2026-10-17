package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;
import souther.nativecode.Generated;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Rust host lists the rows of a behavior it implements and runs one at a time against its own
 * implementation, as a JVM host does through {@code BoundExamples}: the binding lists, the host
 * owns the loop, and the library says whether an answer keeps a row.
 *
 * <p>The answers are chosen where anything but the language's equality would say otherwise: a
 * decimal at another scale than the row states and a set in another order keep their rows, which
 * the texts written of them would not, and a case stated with nothing under it is kept by the case
 * whatever it holds. Beside each, an implementation answering what does not keep the row is told
 * so, with its answer in external form. A row whose answer is owed comes back with the answer and
 * no verdict, and an implementation that answers a failure ends the row, which is not a row it
 * failed to keep.
 */
class ARustHostHoldsItsImplementationToTheRowsOfABehaviorTest {

    private static final String HELD = """
            module held exposing ( Price, Tags, Found, Missing, Outcome, priced, tagged, looked, counted )

            data Price = { amount: Decimal }
            data Tags = { tags: Set<Int> }
            data Found = { id: Int }
            data Missing
            data Outcome = Found | Missing

            behavior priced : (n: Int) -> Price
            behavior tagged : (n: Int) -> Tags
            behavior looked : (n: Int) -> Outcome
            behavior counted : (n: Int) -> Int

            example priced
                | "a price \\"as written\\"" : (1) -> Price { amount = 1.50m }
                | (2) -> Price { amount = 2.00m }

            example tagged
                | "tags as a set" : (1) -> Tags { tags = Set.fromList([1, 2]) }

            example looked
                | "found, whatever it holds" : (1) -> Found

            example counted
                | "three" : (1) -> 3
                | (2) -> <?>
            """;

    private static final String HOST = """
            use rows::held::{
                Counted, CountedImplementation, Found, Looked, LookedImplementation, Missing, Outcome,
                Price, Priced, PricedImplementation, Tagged, TaggedImplementation, Tags,
            };
            use rows::{Decimal, HostError, Library, Run, RowKind};
            use std::panic::{self, AssertUnwindSafe};

            /// What answers each behavior as its rows say, though not as the rows write it.
            struct Right;

            /// What answers each behavior otherwise than its rows say.
            struct Wrong;

            /// What answers nothing.
            struct Empty;

            fn issue(issue: rows::raoh::Issue) -> HostError {
                issue.code().to_owned().into()
            }

            impl Priced for Right {
                fn apply<'run>(&self, run: &mut Run<'run>, n: i64) -> Result<Price<'run>, HostError> {
                    // 1.5 and 2, where the rows state 1.50 and 2.00.
                    let amount = if n == 1 { Decimal::of(15, 1) } else { Decimal::of(2, 0) };
                    Price::new(run, &amount)?.into_result().map_err(issue)
                }
            }

            impl Tagged for Right {
                fn apply<'run>(&self, run: &mut Run<'run>, _n: i64) -> Result<Tags<'run>, HostError> {
                    Tags::new(run, &[2, 1])?.into_result().map_err(issue)
                }
            }

            impl Looked for Right {
                fn apply<'run>(&self, run: &mut Run<'run>, n: i64) -> Result<Outcome<'run>, HostError> {
                    Ok(Found::new(run, n + 6)?.into_result().map_err(issue)?.into())
                }
            }

            impl Counted for Right {
                fn apply<'run>(&self, _run: &mut Run<'run>, n: i64) -> Result<i64, HostError> {
                    Ok(if n == 1 { 3 } else { 5 })
                }
            }

            impl Priced for Wrong {
                fn apply<'run>(&self, run: &mut Run<'run>, n: i64) -> Result<Price<'run>, HostError> {
                    let amount = if n == 1 { Decimal::of(151, 2) } else { Decimal::of(2, 0) };
                    Price::new(run, &amount)?.into_result().map_err(issue)
                }
            }

            impl Tagged for Wrong {
                fn apply<'run>(&self, run: &mut Run<'run>, _n: i64) -> Result<Tags<'run>, HostError> {
                    Tags::new(run, &[2])?.into_result().map_err(issue)
                }
            }

            impl Looked for Wrong {
                fn apply<'run>(&self, run: &mut Run<'run>, _n: i64) -> Result<Outcome<'run>, HostError> {
                    Ok(Missing::new(run)?.into_result().map_err(issue)?.into())
                }
            }

            impl Counted for Wrong {
                fn apply<'run>(&self, _run: &mut Run<'run>, _n: i64) -> Result<i64, HostError> {
                    Ok(4)
                }
            }

            impl Priced for Empty {
                fn apply<'run>(&self, _run: &mut Run<'run>, _n: i64) -> Result<Price<'run>, HostError> {
                    Err("the shelf is empty".into())
                }
            }

            impl Counted for Empty {
                fn apply<'run>(&self, _run: &mut Run<'run>, _n: i64) -> Result<i64, HostError> {
                    Err("nothing to count".into())
                }
            }

            /// Each row of `$type`, run against `$implementation` and said as what it came to.
            macro_rules! said {
                ($run:expr, $who:literal, $type:ident, $implementation:expr) => {
                    for row in $type::examples() {
                        println!("{} {row}: {}", $who, $implementation.evaluate($run, row));
                    }
                };
            }

            fn main() {
                panic::set_hook(Box::new(|_| {}));
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                for row in PricedImplementation::examples()
                    .iter()
                    .chain(TaggedImplementation::examples())
                    .chain(LookedImplementation::examples())
                    .chain(CountedImplementation::examples())
                {
                    println!("{} {} {:?} {:?}", row.behavior(), row.at(), row.identity(), row.kind());
                }
                let right = (
                    PricedImplementation::new(&library, Right),
                    TaggedImplementation::new(&library, Right),
                    LookedImplementation::new(&library, Right),
                    CountedImplementation::new(&library, Right),
                );
                let wrong = (
                    PricedImplementation::new(&library, Wrong),
                    TaggedImplementation::new(&library, Wrong),
                    LookedImplementation::new(&library, Wrong),
                    CountedImplementation::new(&library, Wrong),
                );
                let empty = (
                    PricedImplementation::new(&library, Empty),
                    CountedImplementation::new(&library, Empty),
                );
                library
                    .run(|run| {
                        // What a test of the implementation asserts: each row stating an answer kept.
                        for row in PricedImplementation::examples() {
                            if row.kind() == RowKind::Asserted {
                                right.0.evaluate(run, row).assert_kept(row);
                            }
                        }
                        said!(run, "right", PricedImplementation, right.0);
                        said!(run, "right", TaggedImplementation, right.1);
                        said!(run, "right", LookedImplementation, right.2);
                        said!(run, "right", CountedImplementation, right.3);
                        said!(run, "wrong", PricedImplementation, wrong.0);
                        said!(run, "wrong", TaggedImplementation, wrong.1);
                        said!(run, "wrong", LookedImplementation, wrong.2);
                        said!(run, "wrong", CountedImplementation, wrong.3);
                        said!(run, "empty", PricedImplementation, empty.0);
                        said!(run, "empty", CountedImplementation, empty.1);
                        let row = &PricedImplementation::examples()[0];
                        let refused = panic::catch_unwind(AssertUnwindSafe(|| {
                            right.1.evaluate(run, row);
                        }));
                        let said = refused.expect_err("a row of another behavior is refused");
                        println!("refused: {}", said.downcast_ref::<String>().unwrap());
                        let failed = panic::catch_unwind(AssertUnwindSafe(|| {
                            wrong.0.evaluate(run, row).assert_kept(row);
                        }));
                        let said = failed.expect_err("a row not kept fails what asserts it");
                        println!("asserted: {}", said.downcast_ref::<String>().unwrap());
                    })
                    .unwrap();
            }
            """;

    @Test
    void aRustHostRunsEachRowAgainstItsImplementationAndTheLibraryHoldsTheAnswer(
            @TempDir Path into) throws Exception {
        TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), HELD);
        Generated binding = RustHost.generated(library, into.resolve("binding"), "rows");

        String said = RustHost.ran(into, binding, "rows", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                held.priced 0 Named("a price \\"as written\\"") Asserted
                held.priced 1 Unnamed(2) Asserted
                held.tagged 0 Named("tags as a set") Asserted
                held.looked 0 Named("found, whatever it holds") Asserted
                held.counted 0 Named("three") Asserted
                held.counted 1 Unnamed(2) Owed
                right held.priced a price "as written": kept, answering {"amount":1.5}
                right held.priced row 2: kept, answering {"amount":2}
                right held.tagged tags as a set: kept, answering {"tags":[1,2]}
                right held.looked found, whatever it holds: kept, answering {"type":"Found","id":7}
                right held.counted three: kept, answering 3
                right held.counted row 2: owed, answering 5
                wrong held.priced a price "as written": not kept, answering {"amount":1.51}
                wrong held.priced row 2: kept, answering {"amount":2}
                wrong held.tagged tags as a set: not kept, answering {"tags":[2]}
                wrong held.looked found, whatever it holds: not kept, answering {"type":"Missing"}
                wrong held.counted three: not kept, answering 4
                wrong held.counted row 2: owed, answering 4
                empty held.priced a price "as written": ended without an answer: a host implementation failed: the shelf is empty
                empty held.priced row 2: ended without an answer: a host implementation failed: the shelf is empty
                empty held.counted three: ended without an answer: a host implementation failed: nothing to count
                empty held.counted row 2: ended without an answer: a host implementation failed: nothing to count
                refused: held.priced a price "as written" is not a row of `held.tagged`
                asserted: held.priced a price "as written": not kept, answering {"amount":1.51}
                """);
    }
}
