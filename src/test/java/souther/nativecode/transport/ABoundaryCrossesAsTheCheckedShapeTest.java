package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a behavior's boundary and a declaration's fields carry crosses as the checker settled it.
 *
 * <p>Not the type alone. How a value is written depends on where it stands, and the checker has
 * already answered that for every position a boundary has: what an answer leaves as, what a field
 * carries, and how a set of alternatives travels. A reader on the far side given only the types
 * would have to answer it again, which is the one thing the program API is there to spare it.
 */
class ABoundaryCrossesAsTheCheckedShapeTest {

    private static final String SHOP = """
            module shop exposing ( openDoor, hold, bill, lengthOf, Door, Open, Closed, Phase, Pending, Holder,
                Issued, UnknownSku, NotFound )

            data Closed
            data Open = { since: Int }
            data Door = Open | Closed
            data Phase = Pending | Closed
            data Holder = { state: Closed, note: String?, phase: Phase }

            data Issued = { amount: Int }

            behavior openDoor : (n: Int) -> Door
                constructs Open

            let openDoor (n) = if n > 0 then Open { since = n } else Closed

            behavior hold : (n: Int) -> Holder
                constructs Holder

            let hold (n) = Holder { state = Closed, note = None, phase = Pending }

            behavior bill : (n: Int) -> Issued | UnknownSku
                constructs Issued

            let bill (n) = if n > 0 then Issued { amount = n } else UnknownSku

            behavior lengthOf : (n: Int) -> Int | NotFound

            let lengthOf (n) = if n > 0 then n else NotFound
            """;

    private static String written() {
        return ProgramWriter.written(CheckedProgram.of(List.of(SHOP)));
    }

    @Test
    void anAnswerOfADeclaredTypeCrossesAsTheNominalShapeTheCheckerSettled() {
        assertThat(written()).contains(
                "\"name\":\"openDoor\",\"is\":\"body\",\"inputs\":[{\"is\":\"scalar\",\"scalar\":\"INT\"}],"
                        + "\"output\":{\"is\":\"nominal\",\"declared\":\"shop.Door\"}}");
    }

    @Test
    void aFieldCarriesItsCodecShapeWithItsName() {
        assertThat(written()).contains(
                "\"name\":\"Holder\",\"by\":\"amodule\",\"is\":\"product\",\"fields\":["
                        + "{\"name\":\"state\",\"codec\":{\"is\":\"named\",\"declared\":\"shop.Closed\"}},"
                        + "{\"name\":\"note\",\"codec\":{\"is\":\"optionof\","
                        + "\"present\":{\"is\":\"scalar\",\"scalar\":\"STRING\"}}},"
                        + "{\"name\":\"phase\",\"codec\":{\"is\":\"named\",\"declared\":\"shop.Phase\"}}]");
    }

    @Test
    void aSumCarriesTheFormItsAlternativesTravelIn() {
        String written = written();

        assertThat(written).contains(
                "\"name\":\"Door\",\"by\":\"amodule\",\"is\":\"sum\",\"cases\":["
                        + "{\"is\":\"declared\",\"declared\":\"shop.Open\"},"
                        + "{\"is\":\"declared\",\"declared\":\"shop.Closed\"}],"
                        + "\"form\":{\"is\":\"discriminated\",\"tag\":\"type\",\"contents\":\"value\"}}");
        // One unit, a case of two sums, and each sum answers for its own form: the unit's form
        // is not what decides it.
        assertThat(written).contains(
                "\"name\":\"Phase\",\"by\":\"amodule\",\"is\":\"sum\",\"cases\":["
                        + "{\"is\":\"declared\",\"declared\":\"shop.Pending\"},"
                        + "{\"is\":\"declared\",\"declared\":\"shop.Closed\"}],"
                        + "\"form\":{\"is\":\"enumeration\"}}");
    }

    @Test
    void anAnswerNobodyNamedCarriesItsCasesBesideItsType() {
        assertThat(written()).contains(
                "\"name\":\"bill\",\"is\":\"body\",\"inputs\":[{\"is\":\"scalar\",\"scalar\":\"INT\"}],"
                        + "\"output\":{\"is\":\"cases\",\"type\":{\"union\":["
                        + "{\"is\":\"declared\",\"declared\":\"shop.Issued\"},"
                        + "{\"is\":\"declared\",\"declared\":\"shop.UnknownSku\"}]},"
                        + "\"cases\":[{\"is\":\"declared\",\"declared\":\"shop.Issued\"},"
                        + "{\"is\":\"declared\",\"declared\":\"shop.UnknownSku\"}],"
                        + "\"form\":{\"is\":\"discriminated\",\"tag\":\"type\",\"contents\":\"value\"}}");
    }

    /**
     * A primitive standing as a member of an answer crosses as the case it is. Whether a value of
     * that union can be laid out here is the driver's question, and it is not the writer's to
     * refuse the document over.
     */
    @Test
    void aPrimitiveMemberOfAnAnswerCrossesAsTheCaseItIs() {
        assertThat(written()).contains(
                "\"cases\":[{\"is\":\"primitive\",\"prim\":\"INT\"},"
                        + "{\"is\":\"declared\",\"declared\":\"shop.NotFound\"}]");
    }

    @Test
    void aCollectionTypeCrossesWholeRatherThanRefusingTheDocument() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of("""
                module lists exposing ( none, Bag )

                data Bag = { items: List<Int> }

                behavior none : (n: Int) -> Bag
                """)));

        assertThat(written).contains(
                "{\"name\":\"items\",\"codec\":{\"is\":\"listof\","
                        + "\"element\":{\"is\":\"scalar\",\"scalar\":\"INT\"}}}");
    }
}
