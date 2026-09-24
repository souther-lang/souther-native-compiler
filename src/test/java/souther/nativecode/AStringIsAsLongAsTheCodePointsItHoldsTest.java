package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * {@code String.length} counts code points, which is what the language counts a string in: not
 * the bytes the string is held in, and not the characters a reader sees.
 *
 * <p>Where the two counts differ is where a clause over a length says something different, so the
 * clauses are asked with text whose bytes and code points disagree.
 */
class AStringIsAsLongAsTheCodePointsItHoldsTest {

    private static final String SOURCE = """
            module texts exposing ( len, emailed, found )

            data Email = String
                invariant String.length(value) >= 3

            data Missing

            behavior len : (s: String) -> Int
            let len (s) = String.length(s)

            behavior emailed : (s: String) -> Int
            let emailed (s) = String.length(Email(s).value)

            behavior found : (s: String, there: Bool) -> Int
            let found (s, there) = {
                let answered: String | Missing = s
                let nothing: String | Missing = Missing
                match (if there then answered else nothing) with
                    | String as t -> String.length(t)
                    | Missing -> -1
            }
            """;

    /**
     * A character past the basic plane is one code point and four bytes, and a flag is the two
     * regional indicators it is made of.
     */
    @Test
    void aLengthIsCountedInCodePoints() throws Exception {
        assertLength("", 0);
        assertLength("cart", 4);
        assertLength("日本語", 3);
        assertLength("𠮷", 1);
        assertLength("🇯🇵", 2);
    }

    /**
     * A clause over a length counts each character as one: three characters hold {@code >= 3}
     * whatever they take to write, and two do not, though two Japanese characters are six bytes.
     */
    @Test
    void aClauseOverALengthCountsEachCharacterAsOne() throws Exception {
        assertThat(outcome("emailed", new ObservedValue.Text("あいう")))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(3)));
        assertThat(outcome("emailed", new ObservedValue.Text("あい")))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.INVARIANT_NOT_HELD));
        assertThat(outcome("emailed", new ObservedValue.Text("a@b")))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(3)));
        assertThat(outcome("emailed", new ObservedValue.Text("ab")))
                .isEqualTo(new RunOutcome.Aborted(AbortKind.INVARIANT_NOT_HELD));
    }

    /**
     * A string standing as a case of a union beside a declared case is carried and read back as
     * the string it was, and the declared case is still told apart from it.
     */
    @Test
    void aStringStandsAsACaseOfAUnion() throws Exception {
        assertThat(outcome("found", new ObservedValue.Text("日本"), new ObservedValue.Bool(true)))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(2)));
        assertThat(outcome("found", new ObservedValue.Text("日本"), new ObservedValue.Bool(false)))
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(-1)));
    }

    /** The cart's identifiers, each stating a length, hold natively with non-ASCII text in them. */
    @Test
    void theIdentifiersRowsHold() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds("""
                module ids exposing ( identified )

                data Email = String
                    invariant String.length(value) >= 3
                data UserId = String
                    invariant String.length(value) > 0

                behavior identified : (user: String, email: String) -> Int
                let identified (user, email) =
                    String.length(UserId(user).value) + String.length(Email(email).value)

                example identified
                    | "an identifier of one character is one" : ("u", "a@b") -> 4
                    | "each character counts once whatever it takes to write" : ("利用者", "太郎@例") -> 7
                """);
    }

    private static void assertLength(String text, long counted) throws Exception {
        assertThat(outcome("len", new ObservedValue.Text(text)))
                .as("the length of %s", text)
                .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(counted)));
    }

    private static RunOutcome outcome(String behavior, ObservedValue... handed) throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SOURCE));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior reached = module.behaviors().stream()
                .filter(it -> it.name().name().equals(behavior))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + behavior));
        return running.answeredOrEnded(module, reached, List.of(handed));
    }
}
