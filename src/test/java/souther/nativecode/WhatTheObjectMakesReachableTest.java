package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.Verdict;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedRow;
import souther.compiler.types.ValueName;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * What the object's symbol table carries, held against what the module says it publishes.
 *
 * <p>Two questions and not one. A module's {@code exposing} clause is its surface in the language;
 * what a symbol table carries is this object's answer, worked out from that together with what the
 * object is for. This object is one whole program, so a name the module keeps is reached inside it
 * and nowhere else, and a name it publishes may be reached by a host linking this in.
 *
 * <p>Asked of the object rather than of the driver's decision, because what a linker can reach is
 * the whole of what either answer means. A test that read the transport document instead would
 * agree with the writer about a word.
 */
class WhatTheObjectMakesReachableTest {

    private static final String SURFACE = """
            module surfaced exposing ( shown )

            behavior shown : (a: Int) -> Int
            let shown (a) = hidden(a) + 1

            behavior hidden : (a: Int) -> Int
            let hidden (a) = a * 2

            example hidden
                | "twice" : (21) -> 42
                | "nothing of it" : (0) -> 0
            """;

    @Test
    void aNameTheModulePublishesIsInTheSymbolTableAndOneItKeepsIsLocalToTheObject()
            throws Exception {
        Map<String, String> table = named(CheckedProgram.of(List.of(SURFACE)));

        assertThat(table)
                .as("what the object carries: %s", table)
                .containsEntry("souther4.surfaced.shown", "T")
                .containsEntry("souther4.surfaced.hidden", "t");
    }

    /**
     * A type is built through the object of the build that declares it, and what that object
     * makes reachable is what the module says of the type. One it publishes can be named, and so
     * built, by another build; one it keeps and builds here through its clauses is built here and
     * nowhere else; one it keeps and nothing here builds has no constructor at all, and its clause,
     * which this backend cannot lower, refuses nothing. One it keeps that states no clause has no
     * constructor either, though a body builds it: there is nothing for one to run, and the value
     * is laid out where it is built.
     */
    @Test
    void aTypeIsBuiltWhereTheModuleSaysAndNowhereItDoesNot() throws Exception {
        Map<String, String> table = named(CheckedProgram.of(List.of("""
                module shaped exposing ( Open, made )

                data Open = { n: Int }
                data Closed = { n: Int }
                    invariant n >= 0
                data Plain = { n: Int }
                data Secret = String
                    invariant String.matches("[a-z]+", value)

                behavior made : (n: Int) -> Open
                let made (n) = {
                    let closed = Closed { n = n }
                    let plain = Plain { n = closed.n }
                    Open { n = plain.n }
                }
                """)));

        assertThat(table)
                .as("what the object carries: %s", table)
                .containsEntry("souther4.shaped$construct$Open", "T")
                .containsEntry("souther4.shaped$construct$Closed", "t")
                .doesNotContainKey("souther4.shaped$construct$Plain")
                .doesNotContainKey("souther4.shaped$construct$Secret");
    }

    /**
     * A row of a kept name still runs, through the entry the object carries for it.
     *
     * <p>Which is what makes keeping a name a decision about the module's surface and not about
     * what the language says the behavior owes. Were the rows run by reaching the behavior, the
     * only way to check them would be exporting a name the module keeps.
     */
    @Test
    void everyRowOfAKeptBehaviorHoldsAllTheSame() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SURFACE));
        int asked = 0;
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior kept = behaviorOf(module, "hidden");
        List<CheckedRow> rows = kept.rows();
        for (int at = 0; at < rows.size(); at++) {
            CheckedRow.SelfContained states =
                    (CheckedRow.SelfContained) rows.get(at).statement();
            ObservedValue answered = running.rowAnswering(module, kept, at, List.of());

            assertThat(states.holds(answered))
                    .as("row %d of %s answered %s", at, kept.name(), answered)
                    .isInstanceOf(Verdict.Held.class);
            asked++;
        }
        assertThat(asked).isPositive();
    }

    /**
     * And the entry is what a row is run through rather than a way round the module's answer: the
     * behavior itself is not reachable, and asking for it says so.
     */
    @Test
    void reachingAKeptBehaviorItselfSaysThatIsWhatTheModuleDecided() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(SURFACE));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior kept = behaviorOf(module, "hidden");

        assertThatThrownBy(() -> running.answering(module, kept,
                List.of(new ObservedValue.Integer(21))))
                .isInstanceOf(AssertionError.class)
                .hasMessageContaining("keeps");
    }

    /** The row entries the object carries, which are reached whatever the module publishes. */
    @Test
    void theObjectCarriesAnEntryForEveryRowItWasHandedValuesFor() throws Exception {
        Map<String, String> table = named(CheckedProgram.of(List.of(SURFACE)));

        assertThat(table)
                .containsEntry("souther4.surfaced.hidden$example$0", "T")
                .containsEntry("souther4.surfaced.hidden$example$1", "T");
    }

    /**
     * What a host reaches for an answer as the language writes it follows the same line: a
     * published behavior and every row have a boundary, and a behavior the module keeps has none,
     * since nothing outside the object reaches it to be answered.
     */
    @Test
    void aBoundaryIsOfferedWhereTheEntryItRunsIsReachedFromOutside() throws Exception {
        Map<String, String> table = named(CheckedProgram.of(List.of(SURFACE)));

        assertThat(table)
                .as("what the object carries: %s", table)
                .containsEntry("souther4.surfaced.shown$boundary", "T")
                .containsEntry("souther4.surfaced.hidden$example$0$boundary", "T")
                .containsEntry("souther4.surfaced.hidden$example$1$boundary", "T")
                .doesNotContainKey("souther4.surfaced.hidden$boundary");
    }

    private static CheckedBehavior behaviorOf(CheckedModule module, String name) {
        return module.behavior(new ValueName.Behavior(module.name(), name));
    }

    /**
     * What the object says about each name it carries, by the letter {@code nm} writes for it.
     *
     * <p>Upper case for a name the table offers and lower case for one local to the object, which
     * is the same convention in both formats this builds for. The leading underscore Mach-O writes
     * is taken off again, so what is asked about here is the name the compiler gave.
     */
    private static Map<String, String> named(CheckedProgram program)
            throws IOException, InterruptedException {
        Path into = Files.createTempDirectory("souther-native-table");
        try {
            Path object = into.resolve("program.o");
            Files.write(object, NativeCompiler.compile(program));

            Process nm = new ProcessBuilder("nm", object.toString())
                    .redirectErrorStream(true)
                    .start();
            String said = new String(nm.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            assertThat(nm.waitFor()).as("nm said: %s", said).isZero();

            Map<String, String> table = new LinkedHashMap<>();
            for (String line : said.split("\n")) {
                String[] parts = line.strip().split("\\s+");
                if (parts.length < 2) {
                    continue;
                }
                String name = parts[parts.length - 1];
                String letter = parts[parts.length - 2];
                table.put(name.startsWith("_") ? name.substring(1) : name, letter);
            }
            return table;
        } finally {
            List<Path> held = new ArrayList<>();
            try (var walked = Files.walk(into)) {
                walked.forEach(held::add);
            }
            held.sort((a, b) -> b.getNameCount() - a.getNameCount());
            for (Path path : held) {
                Files.deleteIfExists(path);
            }
        }
    }
}
