package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.program.CheckedProgram;

import java.util.List;
import java.util.Map;
import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value of a type another build declares is read by that build's object: how a declaration is
 * read is its build's, and for a type built from fields that build is also the one running the
 * clauses and so the one that can say which of them did not hold.
 *
 * <p>The constructor another build calls says only that a clause did not hold, which is what a body
 * is told. A document is told which, and where: the issue carries the type, its module and the
 * clause's name, at the path of the value inside the document being read here. So what crosses is
 * the reading itself, and the two objects are linked here to put that to the run — including a
 * case the declaring module keeps, which this build cannot name and reads through a sum it can.
 */
class AValueOfATypeAnotherBuildDeclaresIsReadThereTest {

    private static final String BUILT_BEFORE = """
            module lib.money exposing ( Money, Price, Open, Closed, Door, Settled )

            data Money = Int
                invariant notNegative = value >= 0

            data Price = { amount: Money }

            data Closed
            data Open = { since: Money }
            data Door = Open | Closed

            data Waived = { reason: Money }
            data Settled = Closed | Waived
            """;

    private static final String READING = """
            module app.order exposing ( Order )
            import lib.money ( Money, Price, Door, Settled )

            data Order = { price: Price, door: Door, settled: Settled, count: Int }
                invariant counted = count > 0
            """;

    @Test
    void aValueOfATypeAnotherBuildDeclaresIsReadByThatBuild() throws Exception {
        String harness = new Decoding().type("app.order", "Order")
                .row("whole", "Order", """
                        {"price":{"amount":3},"door":{"type":"Open","since":2},\
                        "settled":{"type":"Waived","reason":1},"count":1}""")
                .row("theirs", "Order", """
                        {"price":{"amount":-1},"door":{"type":"Open","since":-2},\
                        "settled":{"type":"Waived","reason":-3},"count":0}""")
                .row("ours", "Order", """
                        {"price":{"amount":1},"door":{"type":"Closed"},\
                        "settled":{"type":"Closed"},"count":0}""")
                .harness();

        String said = AValueIsReadFromTheFormItIsWrittenInTest.run(program(),
                List.of(new NativeArtifacts.Bytes(builtBefore())), harness);

        assertThat(said).isEqualTo("""
                whole: value {"price":{"amount":3},"door":{"type":"Open","since":2},"settled":{"type":"Waived","reason":1},"count":1}
                theirs: issues [@/price/amount invariant_violation module=lib.money type=Money clause=notNegative] [@/door/since invariant_violation module=lib.money type=Money clause=notNegative] [@/settled/reason invariant_violation module=lib.money type=Money clause=notNegative]
                ours: issues [@ invariant_violation module=app.order type=Order clause=counted]
                """);
    }

    /**
     * What reads a value of a type is that type's build's, and no copy of it: a set of alternatives
     * and a unit, which run no clause, as much as a type built from fields.
     */
    @Test
    void theReaderOfATypeIsDefinedByTheBuildThatDeclaresIt() throws Exception {
        Set<String> here = NativeArtifacts.built(program()).defined();
        Set<String> there = NativeArtifacts.built(CheckedProgram.of(List.of(BUILT_BEFORE))).defined();

        List<String> theirs = List.of(reader("lib.money", "Money"), reader("lib.money", "Price"),
                reader("lib.money", "Open"), reader("lib.money", "Waived"),
                reader("lib.money", "Closed"), reader("lib.money", "Door"),
                reader("lib.money", "Settled"));
        assertThat(there).containsAll(theirs);
        assertThat(here).doesNotContainAnyElementsOf(theirs);
        assertThat(here).contains(reader("app.order", "Order"));
    }

    private static String reader(String module, String name) {
        return Running.PREFIX + "souther" + Running.ABI + "." + module + "$read$" + name;
    }

    private static byte[] builtBefore() throws Exception {
        return NativeArtifacts.object(CheckedProgram.of(List.of(BUILT_BEFORE)));
    }

    private static CheckedProgram program() {
        Map<String, ClassFileImage> published = Compiler.compile(BUILT_BEFORE);
        return CheckedProgram.of(List.of(READING), ModulePath.of(published));
    }
}
