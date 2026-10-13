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
 * A value a Rust host builds with {@code new} and a clause refuses comes back as the issue the
 * type's {@code decode} answers for the same value: the constraint a clause is, with what Raoh's
 * own constraint says of it, or the type and the clause. So which rule the value broke reaches a
 * host whichever way it built the value, and of a type with two clauses it says which.
 */
class ARustHostIsToldWhichClauseItsValueBreaksTest {

    private static final String MEMBERS = """
            module members exposing ( MemberId, Title, Line )

            data MemberId = String
                invariant shaped = String.matches("M[0-9]{5}", value)

            data Title = String
                invariant named = String.length(String.trim(value)) >= 1

            data Line = { quantity: Int, price: Int }
                invariant counted = quantity >= 1
                invariant priced = price >= 0
            """;

    private static final String HOST = """
            use members_binding::members::{Line, MemberId, Title};
            use members_binding::{Construction, Library, Reading};

            fn compared<T, U>(what: &str, built: Construction<T>, read: Reading<U>) {
                let Construction::Rejected(built) = built else { panic!("{what} was built") };
                let Reading::Issues(read) = read else { panic!("{what} was read") };
                let same = read.as_slice() == std::slice::from_ref(&built);
                println!("{what}: {} {:?} {same}", built.code(), built.meta());
            }

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        let built = MemberId::new(run, "x1").unwrap();
                        compared("shaped", built, MemberId::decode(run, "\\"x1\\"").unwrap());
                        let built = Title::new(run, "  ").unwrap();
                        compared("named", built, Title::decode(run, "\\"  \\"").unwrap());
                        let built = Line::new(run, 0, 1).unwrap();
                        let read = Line::decode(run, r#"{"quantity":0,"price":1}"#).unwrap();
                        compared("counted", built, read);
                        let built = Line::new(run, 1, -1).unwrap();
                        let read = Line::decode(run, r#"{"quantity":1,"price":-1}"#).unwrap();
                        compared("priced", built, read);
                        match MemberId::new(run, "M12345").unwrap() {
                            Construction::Value(id) => println!("held: {}", id.encode()),
                            Construction::Rejected(issue) => panic!("M12345 refused: {issue:?}"),
                        }
                    })
                    .unwrap();
            }
            """;

    @Test
    void aConstructionIsRefusedWithTheIssueItsDecoderAnswers(@TempDir Path into) throws Exception {
        TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), MEMBERS);
        Generated binding = RustHost.generated(library, into.resolve("binding"), "members-binding");

        String said = RustHost.ran(into, binding, "members-binding", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                shaped: invalid_format {"pattern": String("M[0-9]{5}")} true
                named: invariant_violation {"clause": String("named"), "module": String("members"), "type": String("Title")} true
                counted: invariant_violation {"clause": String("counted"), "module": String("members"), "type": String("Line")} true
                priced: invariant_violation {"clause": String("priced"), "module": String("members"), "type": String("Line")} true
                held: "M12345"
                """);
    }
}
