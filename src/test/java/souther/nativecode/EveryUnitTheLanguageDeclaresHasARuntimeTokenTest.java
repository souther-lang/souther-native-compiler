package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedData;
import souther.compiler.types.TypeSymbol;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every unit the language itself declares has a token in the runtime, and the runtime claims a
 * token for nothing else.
 *
 * <p>What the language declares is upstream's to say (ADR-0087: the declaration is the source of
 * truth), and a program hands every one of them over whether it names it or not
 * ({@code CheckedProgram.languageDeclarations()}). A value of one is tagged by a token the runtime
 * defines, since no module's object is its home, so a unit the language gains and the runtime
 * does not define is a value no object can say it is. The chain is held link by link: this holds
 * {@code native/crates/abi/language-units.txt} to what upstream declares, the ABI crate holds
 * {@code LANGUAGE_UNITS} to that file, and the runtime holds its tokens to {@code LANGUAGE_UNITS}.
 * A unit added upstream fails here first, with nothing in the native crates having to know it.
 */
class EveryUnitTheLanguageDeclaresHasARuntimeTokenTest {

    private static final Path UNITS = Path.of("native", "crates", "abi", "language-units.txt");

    @Test
    void theUnitsTheRuntimeDefinesAreTheOnesTheLanguageDeclares() throws Exception {
        List<CheckedData> declared = Checked.of(List.of("module m")).languageDeclarations();
        Set<String> units = new TreeSet<>();
        Set<String> leaves = new TreeSet<>();
        for (CheckedData data : declared) {
            switch (data) {
                case CheckedData.Unit it -> units.add(key(it.name()));
                // A sum is never tagged with a token of its own: a value of it is one of its
                // cases, each of which is a unit the language declares too.
                case CheckedData.Sum it -> it.cases().forEach(each -> leaves.add(switch (each) {
                    case TypeSymbol.AtModule at -> key(at);
                    default -> throw new AssertionError(it.name() + " has " + each + " as a case,"
                            + " which is no unit a runtime token could tag");
                }));
                // Nothing the language declares is built from fields, and a backend reading one
                // would have to be told where its constructor is at home.
                default -> throw new AssertionError(data + " is declared by the language and is"
                        + " neither a unit nor a sum of them");
            }
        }
        String written = String.join("\n", units) + "\n";
        String recorded = Files.readString(UNITS, StandardCharsets.UTF_8);
        if (!written.equals(recorded)) {
            Files.writeString(Path.of("target", "language-units.txt"), written,
                    StandardCharsets.UTF_8);
        }

        assertThat(units).as("the language declares units").isNotEmpty();
        assertThat(units).as("every case of a sum the language declares").containsAll(leaves);
        assertThat(recorded)
                .as("the units the language declares, which the runtime defines a token for each"
                        + " of; a unit the language added needs a token in runtime/src/decimal.rs"
                        + " (language_units!), an entry in LANGUAGE_UNITS, and this file")
                .isEqualTo(written);
    }

    private static String key(TypeSymbol.AtModule name) {
        return name.module() + "." + name.name();
    }
}
