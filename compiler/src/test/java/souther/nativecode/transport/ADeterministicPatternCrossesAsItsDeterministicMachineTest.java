package souther.nativecode.transport;

import net.unit8.notation199x.pattern.PatternParser;
import net.unit8.notation199x.pattern.PatternRead;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A pattern crosses as the image notation-199x writes of its machine, and from 0.2.0 that is the
 * deterministic machine, as P2, wherever there is one whose image fits: the runtime reads it in time
 * linear in the image and walks it one row at a time, where a P1 image of the shape's machine has it
 * hold a set of states at every step. The runtime's crate reads both formats, so this holds which one
 * the Java library writes, and not whether the runtime can read it.
 */
class ADeterministicPatternCrossesAsItsDeterministicMachineTest {

    @Test
    void aPatternWithADeterministicMachineCrossesAsP2() {
        String written = "[a-z]+(-[a-z]+)*";
        PatternRead read = PatternParser.read(written);
        assertThat(read).isInstanceOf(PatternRead.Read.class);

        String image = PatternImages.of(written, ((PatternRead.Read) read).meaning());

        assertThat(image).startsWith("P2,");
    }
}
