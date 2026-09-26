package souther.nativecode.transport;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a type holds its values to crosses as the checker elaborated it: each clause's name and its
 * condition as {@code Core}, over the bindings the fields carry.
 */
class AClauseCrossesAsTheConditionTheCheckerElaboratedTest {

    private static final String OWING = """
            module owing exposing ( Positive, Span, Labelled, Plain, holds )

            data Positive = Int
                invariant positive = value > 0

            data Span = { lo: Int, hi: Int }
                invariant lo + 1 <= hi

            data Labelled = { ...Span, label: String }

            data Plain = { n: Int }

            // What names the types, so the document carries them: a declaration crosses where
            // something in the program reaches it.
            behavior holds : (p: Positive, s: Span, l: Labelled, q: Plain) -> Int
            """;

    private static String written() {
        return ProgramWriter.written(Checked.of(List.of(OWING)));
    }

    /** A clause reads a field through the number the field is written with. */
    @Test
    void aClauseReadsAFieldThroughTheBindingTheFieldCarries() {
        assertThat(written()).contains(
                "\"is\":\"newtype\",\"field\":{\"name\":\"value\",\"binding\":0,"
                        + "\"codec\":{\"is\":\"scalar\",\"scalar\":\"INT\"}},"
                        + "\"invariants\":[{\"name\":\"positive\",\"condition\":"
                        + "{\"core\":\"binary\",\"op\":\"GT\",\"reading\":{\"is\":\"astheystand\"},"
                        + "\"left\":{\"core\":\"read\",\"binding\":0,");
    }

    /**
     * A clause written without a name crosses with none, and what it can end without a value for
     * is asked of the program the way it is of a body's.
     */
    @Test
    void anUnnamedClauseCrossesWithNoNameAndWithWhatItsSitesCanEndFor() {
        assertThat(written()).contains(
                "\"invariants\":[{\"name\":null,\"condition\":{\"core\":\"binary\",\"op\":\"LE\",\"reading\":{\"is\":\"astheystand\"},"
                        + "\"left\":{\"core\":\"binary\",\"op\":\"ADD\",\"reading\":{\"is\":\"astheystand\"},");
        assertThat(written()).contains("\"aborts\":[\"REQUIRED_FORM_HAS_NO_PLACE\"]");
    }

    /** A type that takes another in holds its values to the clauses of what it took in. */
    @Test
    void aSpreadCarriesTheClausesOfWhatItTakesIn() {
        String written = written();
        int labelled = written.indexOf("\"name\":\"Labelled\"");
        assertThat(labelled).isNotNegative();
        // Up to the declaration written after it: each one opens with its module.
        String declaration = written.substring(labelled, written.indexOf("{\"module\":", labelled));

        assertThat(declaration).contains("{\"name\":\"lo\",\"binding\":0,");
        assertThat(declaration).contains("{\"name\":\"hi\",\"binding\":1,");
        assertThat(declaration).contains("\"invariants\":[{\"name\":null,\"condition\":");
    }

    /** A type that states nothing crosses with no clause. */
    @Test
    void aTypeThatStatesNothingCrossesWithNoClause() {
        assertThat(written()).contains(
                "\"name\":\"Plain\",\"by\":\"amodule\",\"is\":\"product\",\"fields\":["
                        + "{\"name\":\"n\",\"binding\":0,"
                        + "\"codec\":{\"is\":\"scalar\",\"scalar\":\"INT\"}}],\"invariants\":[]}");
    }
}
