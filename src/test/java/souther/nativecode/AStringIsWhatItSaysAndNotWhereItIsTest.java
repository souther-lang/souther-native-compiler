package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Text, which is the first value here whose meaning is not what it is held as.
 *
 * <p>An {@code Int} and a {@code Bool} are equal exactly when the machine words holding them are,
 * so until now a comparison could be emitted without asking what was being compared. A string is
 * an address, and two strings saying one text are two addresses. So the questions a representation
 * owes — where it lives, what a comparison of two of them is — are answered here rather than
 * settled by what a value happens to be held in.
 *
 * <p>The rows are the oracle, as they are everywhere else: a row that ran and whose answer kept it
 * is one the JVM answered, so a native run put to the same row is held to what the JVM said without
 * this file writing down what either of them should say. That matters most for the order, which is
 * by scalar value and is not the order of a JVM string's UTF-16 code units.
 */
class AStringIsWhatItSaysAndNotWhereItIsTest {

    /**
     * Text joined, compared and ordered, with the rows stating what each comes to.
     *
     * <p>A join is the one way a run makes a string here, so a row over one is a row over a string
     * the arena holds rather than one the object carries — and comparing that against a literal is
     * what says the two are one kind of value.
     */
    private static final String TEXT = """
            module text exposing ( joined, same, before )

            behavior joined : (a: String, b: String) -> String
            let joined (a, b) = a ++ b

            behavior same : (a: String, b: String) -> Bool
            let same (a, b) = a == b

            behavior before : (a: String, b: String) -> Bool
            let before (a, b) = a < b

            behavior joinIs : (a: String, b: String, whole: String) -> Bool
            let joinIs (a, b, whole) = (a ++ b) == whole

            example joined
                | "two words" : ("ab", "cd") -> "abcd"
                | "nothing before it" : ("", "cd") -> "cd"
                | "nothing after it" : ("ab", "") -> "ab"
                | "nothing at all" : ("", "") -> ""
                | "a newline and a quote" : ("a\\n", "\\"b") -> "a\\n\\"b"
                | "a mark after a letter composes with it" : ("e", "\u0301") -> "\u00E9"

            example same
                | "one text twice" : ("abc", "abc") -> true
                | "two texts" : ("abc", "abd") -> false
                | "one a prefix of the other" : ("ab", "abc") -> false
                | "nothing and nothing" : ("", "") -> true

            example before
                | "by the first letter" : ("a", "b") -> true
                | "the other way" : ("b", "a") -> false
                | "a prefix comes first" : ("ab", "abc") -> true
                | "nothing comes before everything" : ("", "a") -> true
                | "neither comes before itself" : ("a", "a") -> false
                | "before the basic plane's end, and past it" : ("￥", "𠮷") -> true
                | "past the basic plane, and before its end" : ("𠮷", "￥") -> false

            example joinIs
                | "a join against a literal" : ("ab", "cd", "abcd") -> true
                | "a join against another text" : ("ab", "cd", "abce") -> false
            """;

    /**
     * Every row, run through the entry the object carries for it.
     *
     * <p>Which is what holds the native answers to the JVM's. The ordering rows are the ones to read
     * twice: {@code 𠮷} is U+20BB7 and {@code ￥} is U+FFE5, so by scalar value — and by the UTF-8
     * bytes, which are in the same order — {@code ￥} comes first. A JVM string's own order, by
     * UTF-16 code unit, puts {@code 𠮷} first, since it begins with the unit D842, and an
     * implementation that compared those would fail here and nowhere else in this file.
     */
    @Test
    void everyRowOverTextHoldsWhenTheNativeObjectAnswersIt() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(TEXT);
    }

    /**
     * Two strings the run never saw written down, compared.
     *
     * <p>The rows above hand over literals, which the object carries. This hands over text on the
     * command line, so what is compared is two strings the runtime made — at two addresses, out of
     * bytes that arrived from outside. A comparison of the addresses would say these are different.
     */
    @Test
    void twoStringsMadeApartOutOfOneTextAreEqual() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(TEXT));

        Running running = Running.of(program);
        assertThat(answering(running, program, "same", text("abc"), text("abc")))
                .isEqualTo(new ObservedValue.Bool(true));
        assertThat(answering(running, program, "same", text(""), text("")))
                .isEqualTo(new ObservedValue.Bool(true));
        assertThat(answering(running, program, "same", text("abc"), text("abd")))
                .isEqualTo(new ObservedValue.Bool(false));
    }

    /**
     * A string answered out of the object, read back as the text it says.
     *
     * <p>Handed over and answered, so what crosses is text in both directions. The newline and the
     * nought are in it because a string is bytes: one that ended at the first nought, or that was
     * read back a line at a time, would be wrong in a way that looks like the program having
     * answered something else.
     */
    @Test
    void textCrossesBothWaysWhateverBytesItHolds() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(TEXT));

        Running running = Running.of(program);
        assertThat(answering(running, program, "joined", text("ab"), text("cd")))
                .isEqualTo(new ObservedValue.Text("abcd"));
        assertThat(answering(running, program, "joined", text(""), text("")))
                .isEqualTo(new ObservedValue.Text(""));
        assertThat(answering(running, program, "joined", text("a\nb"), text("\u0000c")))
                .isEqualTo(new ObservedValue.Text("a\nb\u0000c"));
        assertThat(answering(running, program, "joined", text("𠮷"), text("￥")))
                .isEqualTo(new ObservedValue.Text("𠮷￥"));
    }

    private static ObservedValue answering(Running running, CheckedProgram program, String name,
                                           ObservedValue... given) throws Exception {
        CheckedModule module = program.modules().getFirst();
        for (CheckedBehavior behavior : module.behaviors()) {
            if (behavior.name().name().equals(name)) {
                return running.answering(module, behavior, List.of(given));
            }
        }
        throw new AssertionError("no behavior called " + name);
    }

    private static ObservedValue text(String said) {
        return new ObservedValue.Text(said);
    }
}
