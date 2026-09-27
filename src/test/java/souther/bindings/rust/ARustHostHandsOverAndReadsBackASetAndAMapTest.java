package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Checked;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Rust host hands a set over as a slice of its members and a map as a slice of its entries, each
 * a tuple of its key and its value, and is handed each back as a {@code Vec} of the same: the lists
 * a set and a map cross as. The library makes a set of what it is handed, two members that are one
 * being one, and a map of the entries, the later of two under one key winning. The order one is
 * handed back in is no order the language says anything of, so the host sorts what it prints.
 */
class ARustHostHandsOverAndReadsBackASetAndAMapTest {

    private static final String TALLY = """
            module tally exposing ( Tally, sized, counted, total, tallied )

            data Tally = { tags: Set<String>, counts: Map<String, Int> }

            behavior sized : (s: Set<Int>) -> Int
            let sized (s) = Set.size(s)

            behavior counted : (words: List<String>) -> Map<String, Int>
            let counted (words) =
                List.fold((acc, w) -> Map.updateOrInsert(w, 1, n -> n + 1, acc), Map.empty, words)

            behavior total : (m: Map<String, Int>) -> Int
            let total (m) = Map.fold((acc, k, v) -> acc + v, 0, m)

            behavior tallied : (t: Tally) -> Int
            let tallied (t) = Set.size(t.tags) * 100 + Map.size(t.counts)
            """;

    private static final String HOST = """
            use tally_binding::tally::{self, Tally};
            use tally_binding::Library;

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        println!("sized: {}", tally::sized(run, &[3, 1, 3, 2]).unwrap());
                        let words = ["b".to_owned(), "a".to_owned(), "b".to_owned()];
                        let mut counted = tally::counted(run, &words).unwrap();
                        counted.sort();
                        println!("counted: {counted:?}");
                        let entries = [("a".to_owned(), 1), ("b".to_owned(), 2), ("a".to_owned(), 5)];
                        println!("total: {}", tally::total(run, &entries).unwrap());

                        let tags = ["y".to_owned(), "x".to_owned(), "y".to_owned()];
                        let counts = [("k".to_owned(), 1), ("j".to_owned(), 2)];
                        let held = Tally::new(run, &tags, &counts).unwrap().into_result().unwrap();
                        let mut tags = held.tags();
                        tags.sort();
                        println!("tags: {tags:?}");
                        println!("tallied: {}", tally::tallied(run, held).unwrap());
                        println!("written: {}", held.encode());
                    })
                    .unwrap();
            }
            """;

    @Test
    void aSetAndAMapCrossAsSlicesOfWhatTheyHold(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(Checked.of(List.of(TALLY)), into.resolve("native"));
        RustBindings.Generated binding =
                RustHost.generated(library, into.resolve("binding"), "tally-binding");

        String said = RustHost.ran(into, binding, "tally-binding", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                sized: 3
                counted: [("a", 1), ("b", 2)]
                total: 7
                tags: ["x", "y"]
                tallied: 202
                written: {"tags":["x","y"],"counts":{"j":2,"k":1}}
                """);
    }
}
