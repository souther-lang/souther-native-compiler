package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;

import java.io.IOException;
import java.util.Arrays;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The halves meeting: a program checked here, written out, driven through the process boundary,
 * and an object back.
 *
 * <p>What the object answers when it is linked and run is asked on the other side, where there is
 * a linker. This asks only that the two halves reach each other.
 */
class AnAdditionCompilesToAnObjectTest {

    /** The first bytes of a Mach-O and of an ELF object. Whichever this machine writes. */
    private static final byte[] MACH_O = {(byte) 0xcf, (byte) 0xfa, (byte) 0xed, (byte) 0xfe};
    private static final byte[] ELF = {0x7f, 'E', 'L', 'F'};

    @Test
    void aCheckedProgramCrossesToTheDriverAndAnObjectComesBack()
            throws IOException, InterruptedException {
        byte[] object = NativeCompiler.compile(CheckedProgram.of(List.of("""
                module calculation

                behavior add : (a: Int, b: Int) -> Int

                let add (a, b) = a + b
                """)));

        assertThat(object).isNotEmpty();
        byte[] head = Arrays.copyOf(object, MACH_O.length);
        assertThat(Arrays.equals(head, MACH_O) || Arrays.equals(head, ELF))
                .as("an object file, and what came back starts %s", Arrays.toString(head))
                .isTrue();
    }
}
