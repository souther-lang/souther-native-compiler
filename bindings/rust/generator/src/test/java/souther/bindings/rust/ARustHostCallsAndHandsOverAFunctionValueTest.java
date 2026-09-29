package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.Generated;
import souther.nativecode.Documents;
import souther.nativecode.NativeCompiler;

import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A function value is an enum of its type to a Rust host: one the library made, which it calls
 * with Rust values, or a function of its own, which it hands over where a function value is taken
 * and the library calls. What the host's function answers as a failure comes back out of the call
 * into the library that reached it, a computation that ends without a value is a failure, and a
 * function value made in a run is called in a run inside it.
 *
 * <p>The library is built from the document the driver's own tests of function values are held to
 * ({@link Documents#FUNCTIONS}), as the PHP binding's test of them is: no source publishes a
 * function value yet.
 */
class ARustHostCallsAndHandsOverAFunctionValueTest {

    private static final String HOST = """
            use calling::m::{self, FnIntToInt};
            use calling::{Failure, Library};

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        let bump = m::bump(run).unwrap();
                        println!("bump: {}", bump.call(run, 3).unwrap());
                        let twice = m::twice(run).unwrap();
                        println!("twice bump: {}", twice.call(run, bump.clone(), 1).unwrap());
                        let tripled = FnIntToInt::host(|_, x| Ok(x * 3));
                        println!("twice hosted: {}", twice.call(run, tripled.clone(), 2).unwrap());
                        println!("hosted alone: {}", tripled.call(run, 4).unwrap());
                        let refusing = FnIntToInt::host(|_, x| Err(format!("refused {x}").into()));
                        match twice.call(run, refusing, 2) {
                            Err(Failure::Host(failure)) => println!("failed: {failure}"),
                            other => println!("failed: {:?}", other.map(|_| ())),
                        }
                        match m::overflow(run).unwrap().call(run, 1) {
                            Err(Failure::Abort(abort)) => println!("overflow: {}", abort.name().unwrap()),
                            other => println!("overflow: {:?}", other.map(|_| ())),
                        }
                        println!("pairing: {:?}", m::pairing(run).unwrap().call(run, -4).unwrap());
                        let deep = m::deep(run).unwrap();
                        for x in [1, 0, -1] {
                            println!("deep {x}: {:?}", deep.call(run, x).unwrap());
                        }
                        let meet = m::meet(run).unwrap();
                        println!("meet: {:?} {:?}", meet.call(run, (4, Some(7))).unwrap(),
                            meet.call(run, (4, None)).unwrap());
                        let lifted = m::lifted(run).unwrap().call(run, 10).unwrap();
                        println!("lifted: {}", lifted.call(run, 1).unwrap());

                        // One function of the host's handed over again and again in a run is one
                        // function value, which the library keeps calling.
                        for x in 0..1000 {
                            twice.call(run, tripled.clone(), x).unwrap();
                        }

                        // What was made in this run is called in a run inside it.
                        let inside = run.scope(|inner| twice.call(inner, tripled.clone(), 5).unwrap());
                        println!("inside: {inside}");
                    })
                    .unwrap();
            }
            """;

    @Test
    void aFunctionValueIsAnEnumOfTheLibrarysAndTheHostsOwn(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                Documents.library(Documents.FUNCTIONS, into.resolve("native"));
        Generated binding =
                RustHost.generated(library, into.resolve("binding"), "calling");

        String said = RustHost.ran(into, binding, "calling", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                bump: 8
                twice bump: 11
                twice hosted: 18
                hosted alone: 12
                failed: refused 2
                overflow: REQUIRED_FORM_HAS_NO_PLACE
                pairing: (-4, false)
                deep 1: Some(Some(1))
                deep 0: Some(None)
                deep -1: None
                meet: Some(7) None
                lifted: 11
                inside: 45
                """);
    }
}
