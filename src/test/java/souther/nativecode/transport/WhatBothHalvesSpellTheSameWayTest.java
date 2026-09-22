package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.compiler.program.DeclaredBy;
import souther.compiler.program.Publication;
import souther.compiler.types.BinOp;
import souther.compiler.types.Type;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every member of every vocabulary this writer spells, held to what the driver reads it as.
 *
 * <p>A vocabulary the language closed is spelt twice, once per half, and neither copy holds the
 * other to anything. Two members spelt differently on the two sides is loud: the driver refuses a
 * word it does not read and says the halves disagree. One member spelt as another member of the
 * same vocabulary is not loud at all — the document reads, and what it means is not what it says.
 *
 * <p>Which is a real gap and not a hypothetical one. Of what crosses today, only a handful of
 * members are put through both halves by a program some other test compiles: {@code ADD},
 * {@code SUB}, {@code MUL}, the comparisons, {@code AND}, {@code OR}, {@code INT}, {@code BOOL},
 * {@code STRING}, and both publications. {@code DIV}, {@code CONCAT}, seven primitives and the
 * provenance of a declaration the language gives were each written on both sides and read by
 * nothing.
 *
 * <p>So the writer's copy is written out here and the driver's test reads it back, which is what
 * the addition fixture already is. This half asserts the file is what this writer writes; the other
 * half asserts each word at each place is read as the member it names. A spelling that moves on
 * either side is then red on the other, and a member added upstream stops this compiling before it
 * can be spelt at all.
 */
class WhatBothHalvesSpellTheSameWayTest {

    /**
     * The document the driver's test reads. Written to a file rather than described there again,
     * for the reason the addition fixture is.
     */
    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "vocabularies.transport.json");

    @Test
    void theFixtureTheDriverIsHeldToIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.vocabularies());
    }

    /**
     * Every member of each of them, and not however many happen to be written.
     *
     * <p>The switch that spells one is exhaustive, so a member added upstream stops this compiling.
     * What that does not catch is a vocabulary written out of a list rather than out of the switch —
     * which this does, by asking the language how many members it has.
     */
    @Test
    void everyMemberOfEachVocabularyIsSpelt() {
        String written = ProgramWriter.vocabularies();

        assertThat(words(written, "op")).hasSize(BinOp.values().length);
        assertThat(words(written, "prim")).hasSize(Type.Prim.values().length);
        assertThat(words(written, "publication")).hasSize(Publication.values().length);
        assertThat(words(written, "declaredby")).hasSize(DeclaredBy.values().length);
    }

    /** No member of one vocabulary is spelt the way another member of it is. */
    @Test
    void noTwoMembersOfOneVocabularyAreSpeltAlike() {
        String written = ProgramWriter.vocabularies();

        for (String vocabulary : new String[]{"op", "prim", "publication", "declaredby"}) {
            assertThat(words(written, vocabulary))
                    .as("the spellings of %s in %s", vocabulary, written)
                    .doesNotHaveDuplicates();
        }
    }

    private static String[] words(String written, String vocabulary) {
        int at = written.indexOf("\"" + vocabulary + "\":[");
        assertThat(at).as("%s is among %s", vocabulary, written).isNotNegative();
        int from = written.indexOf('[', at) + 1;
        return written.substring(from, written.indexOf(']', from)).split(",");
    }
}
