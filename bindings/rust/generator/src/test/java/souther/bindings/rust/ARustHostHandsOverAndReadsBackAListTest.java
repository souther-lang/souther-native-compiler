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
 * A Rust host hands a list over as a slice and is handed one back as a {@code Vec}, of values of a
 * declared type, of optionals and of lists: each built and read through the functions the manifest
 * names for a list of its element's shape, whatever it is a list of.
 */
class ARustHostHandsOverAndReadsBackAListTest {

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
            use cart_binding::cart::{self, Grid, Notes, Order, OrderLine, PricedCart};
            use cart_binding::{Construction, Library};

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        let apple = OrderLine::new(run, "apple", 2, 30).unwrap().into_result().unwrap();
                        let pear = OrderLine::new(run, "pear", 1, 50).unwrap().into_result().unwrap();
                        let order = Order::new(run, &[apple, pear]).unwrap().into_result().unwrap();
                        println!("total: {}", cart::totalOf(run, order).unwrap());
                        let skus: Vec<String> = order.lines().iter().map(|it| it.sku()).collect();
                        println!("lines: {skus:?}");

                        let echoed = cart::echoed(run, &[pear, apple, pear]).unwrap();
                        let skus: Vec<String> = echoed.iter().map(|it| it.sku()).collect();
                        println!("echoed: {skus:?}");
                        println!("echoed none: {}", cart::echoed(run, &[]).unwrap().len());

                        match PricedCart::new(run, &[], 0).unwrap() {
                            Construction::Value(_) => println!("empty cart: built"),
                            Construction::Rejected(issue) => println!("empty cart: {}", issue.code()),
                        }

                        let said = [Some("gift".to_owned()), None, Some(String::new())];
                        let notes = Notes::new(run, &said).unwrap().into_result().unwrap();
                        println!("notes: {:?}", notes.said());

                        let rows = [vec![1, 2, 3], vec![], vec![4]];
                        let grid = Grid::new(run, &rows).unwrap().into_result().unwrap();
                        println!("grid: {:?}", grid.rows());
                        println!("grid encoded: {}", grid.encode());
                    })
                    .unwrap();
            }
            """;

    @Test
    void aListIsASliceHandedOverAndAVecHandedBack(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(CART)), into.resolve("native"));
        Generated binding =
                RustHost.generated(library, into.resolve("binding"), "cart-binding");

        String said = RustHost.ran(into, binding, "cart-binding", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                total: 110
                lines: ["apple", "pear"]
                echoed: ["pear", "apple", "pear"]
                echoed none: 0
                empty cart: invariant_violation
                notes: [Some("gift"), None, Some("")]
                grid: [[1, 2, 3], [], [4]]
                grid encoded: {"rows":[[1,2,3],[],[4]]}
                """);
    }
}
