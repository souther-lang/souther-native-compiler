package souther.bindings.rust;

import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;



import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A tuple is a Rust tuple of its members, and an optional an {@code Option} at every depth: an
 * optional of an optional holding nothing is {@code Some(None)}, and one that holds nothing is
 * {@code None}, which Rust tells apart as the model does with nothing added to either.
 */
class ARustHostReadsATupleAndAnOptionalAtAnyDepthTest {

    private static final String SHAPE = """
            module shape exposing ( Line, pair, noted, blank, beyond, pairs )

            data Line = { quantity: Int, note: String? }

            let pair: (Int, Bool) = (3, true)

            let noted = List.get(0, [Line { quantity = 1, note = "gift" }.note])

            let blank = List.get(0, [Line { quantity = 1, note = None }.note])

            let beyond = List.get(1, [Line { quantity = 1, note = None }.note])

            let pairs = [(1, "a"), (2, "b")]
            """;

    private static final String HOST = """
            use shapes::shape;
            use shapes::Library;

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        println!("pair: {:?}", shape::pair(run).unwrap());
                        println!("noted: {:?}", shape::noted(run).unwrap());
                        println!("blank: {:?}", shape::blank(run).unwrap());
                        println!("beyond: {:?}", shape::beyond(run).unwrap());
                        println!("pairs: {:?}", shape::pairs(run).unwrap());
                        let line = shape::Line::new(run, 2, Some("gift")).unwrap().into_result().unwrap();
                        println!("line: {} {:?}", line.quantity(), line.note());
                    })
                    .unwrap();
            }
            """;

    @Test
    void aTupleAndAnOptionalAreRustsOwnAtEveryDepth(@TempDir Path into) throws Exception {
        TestLibrary library =
                SoutherBindingTest.compile(into.resolve("native"), SHAPE);
        Generated binding = RustHost.generated(library, into.resolve("binding"),
                "shapes");

        String said = RustHost.ran(into, binding, "shapes", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                pair: (3, true)
                noted: Some(Some("gift"))
                blank: Some(None)
                beyond: None
                pairs: [(1, "a"), (2, "b")]
                line: 2 Some("gift")
                """);
    }
}
