package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.program.CheckedProgram;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A behavior with no body is supplied by whoever links the program, and a library is a link of it:
 * what supplies one goes into the library beside the program's object, the way it goes into an
 * executable of the program.
 *
 * <p>What supplies it is not a Souther build and carries nothing a host is offered, so it adds
 * nothing to the header, and what a host calls is still only what the program's object carries.
 */
class ALibraryIsSuppliedWhatNoBuildDefinesTest {

    private static final String PRICING = """
            module pricing exposing ( twice )

            behavior lookUp : (a: Int) -> Int

            behavior twice : (a: Int) -> Int
                depends on lookUp
            let twice (a, lookUp) = lookUp(a) * 2
            """;

    /**
     * What answers `lookUp`, under the symbol the object names it by. Written with an {@code
     * __asm__} label, since the symbol carries a dot: supplying a behavior from C under a C name is
     * not something this compiler offers yet, and nothing a host calls is declared this way.
     */
    private static final String SUPPLYING = """
            #include <stdint.h>

            uint32_t lookUp(int64_t a, int64_t *out) __asm__("%ssouther%s.pricing.lookUp");
            uint32_t lookUp(int64_t a, int64_t *out) {
                *out = a + 20;
                return 0;
            }
            """.formatted(Running.PREFIX, Running.ABI);

    private static final String HOST = """
            #include <inttypes.h>
            #include <stdio.h>
            #include "souther.h"

            int main(void) {
                int64_t answer = -1;
                souther_status status = souther2_m_pricing_b_twice(1, &answer);
                printf("%u %" PRId64 "\\n", status, answer);
                return 0;
            }
            """;

    @Test
    void whatSuppliesAnInjectedBehaviorIsLinkedIntoTheLibrary(@TempDir Path into)
            throws Exception {
        Path supplied = into.resolve("supplying.o");
        Path source = into.resolve("supplying.c");
        Files.writeString(source, SUPPLYING, StandardCharsets.UTF_8);
        said(List.of("cc", "-c", "-o", supplied.toString(), source.toString()));

        NativeCompiler.Library library = NativeCompiler.library(
                CheckedProgram.of(List.of(PRICING)), List.of(), List.of(supplied),
                into.resolve("built"));

        assertThat(Files.readString(library.declarations(), StandardCharsets.UTF_8))
                .contains("souther2_m_pricing_b_twice(")
                .doesNotContain("lookUp");

        Path host = into.resolve("host.c");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);
        Path executable = into.resolve("host");
        said(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), host.toString(),
                "-I", library.header().getParent().toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));
        assertThat(said(List.of(executable.toString()))).isEqualTo("0 42\n");
    }

    /** With nothing supplying it, the library is not all there, and the link says so. */
    @Test
    void aLibraryNothingSuppliesIsRefused(@TempDir Path into) {
        assertThatThrownBy(() -> NativeCompiler.library(
                CheckedProgram.of(List.of(PRICING)), List.of(), List.of(), into))
                .hasMessageContaining("the linker did not make");
    }

    private static String said(List<String> command) throws Exception {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError(command.get(0) + " failed: " + said);
        }
        return said;
    }
}
