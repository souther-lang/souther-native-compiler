package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.Checked;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value whose body folds to a constant is declared and published, and builds no value: it has no
 * place to run (ADR-0074), so the module holds an entry for it and no {@code CheckedValue}, and the
 * entry answers what the value folds to. What the driver runs of that entry is
 * `native/crates/compiler/tests/values.rs`, which reads the document written here.
 */
class AnExposedConstantIsPublishedByAnEntryTest {

    private static final String MODULE = """
            module k exposing ( limit, doubled )

            let limit = 5

            let doubled = limit * 2
            """;

    private static final Path FIXTURE =
            Path.of("native", "crates", "compiler", "tests", "constants.transport.json");

    @Test
    void aConstantHasAnEntryAndNoValue() {
        CheckedProgram program = Checked.of(List.of(MODULE));
        CheckedModule module = program.modules().getFirst();

        assertThat(module.values()).isEmpty();
        assertThat(module.valueEntries()).hasSize(2);
        assertThat(ProgramWriter.written(program)).contains("\"values\":[]");
    }

    /** What the driver is tested against is this, written to a file rather than described there
     *  again — the same split every other transport fixture keeps. */
    @Test
    void theFixtureTheDriverIsTestedAgainstIsWhatThisWrites() throws IOException {
        assertThat(Files.readString(FIXTURE, StandardCharsets.UTF_8).strip())
                .isEqualTo(ProgramWriter.written(Checked.of(List.of(MODULE))));
    }
}
