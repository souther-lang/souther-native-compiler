package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.Verdict;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedData;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedRow;
import souther.compiler.types.TypeSymbol;

import java.io.IOException;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.StringJoiner;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a host's own implementation answers a row of a behavior it implements is held to the row in
 * the library, and the library says what the JVM says of the same answer.
 *
 * <p>The JVM is the oracle and not this file: each answer is handed to the library through an
 * implementation a C host makes a capability of, and the same answer, as the value it is, to the row's
 * own {@link CheckedRow.SelfContained#holds}. What is compared is the two verdicts. The answers are
 * chosen where a comparison made by anything but the language's equality would part from it: a
 * decimal at another scale, a set and a map in another order, a case stated with nothing under it
 * and answered with something, a primitive carried into a union no declaration names, and answers
 * that do not keep the row beside each that does.
 *
 * <p>The implementation reads what it answers out of the language's external form through the
 * answer type's own decoder, so the value the library holds is one the library built and not one
 * this file laid out.
 */
class AHostsImplementationIsHeldToARowAsTheJvmHoldsItTest {

    private static final String HELD = """
            module held exposing ( Price, Tags, Found, Missing, Outcome, priced, tagged, looked, numbered,
                                   counted )

            data Price = { amount: Decimal }
            data Tags = { tags: Set<Int>, by: Map<String, Int> }
            data Found = { id: Int }
            data Missing
            data Outcome = Found | Missing

            behavior priced : (n: Int) -> Price
            behavior tagged : (n: Int) -> Tags
            behavior looked : (n: Int) -> Outcome
            behavior numbered : (n: Int) -> Int | Missing
            behavior counted : (n: Int) -> Int

            example priced
                | "a price at the scale it was written" : (1) -> Price { amount = 1.50m }

            example tagged
                | "tags as a set and a map" : (1) -> Tags { tags = Set.fromList([1, 2]), by = Map.fromList([("a", 1), ("b", 2)]) }

            example looked
                | "found, whatever it holds" : (1) -> Found
                | "found with what it holds" : (2) -> Found { id = 2 }
                | "missing" : (3) -> Missing

            example numbered
                | "a number where a union is answered" : (1) -> 1
                | "missing where a number could be" : (2) -> Missing

            example counted
                | "three" : (1) -> 3
                | (2) -> <?>
            """;

    /** One answer an implementation gives a row, as its external form and as the value it is. */
    private record Answer(String behavior, int row, String written, ObservedValue value) {

        String label() {
            return behavior + "." + row + " " + written;
        }
    }

    @Test
    void theLibraryHoldsAnAnswerToARowAsTheJvmDoes(@TempDir Path into) throws Exception {
        CheckedProgram program = Checked.of(List.of(HELD));
        CheckedModule module = program.modules().getFirst();
        TypeSymbol price = declared(module, "Price");
        TypeSymbol tags = declared(module, "Tags");
        TypeSymbol found = declared(module, "Found");
        TypeSymbol missing = declared(module, "Missing");
        List<Answer> answers = List.of(
                new Answer("priced", 0, "{\"amount\":1.5}", new ObservedValue.Constructed(price,
                        Map.of("amount", new ObservedValue.Decimal(new BigDecimal("1.5"))))),
                new Answer("priced", 0, "{\"amount\":1.500}", new ObservedValue.Constructed(price,
                        Map.of("amount", new ObservedValue.Decimal(new BigDecimal("1.500"))))),
                new Answer("priced", 0, "{\"amount\":1.51}", new ObservedValue.Constructed(price,
                        Map.of("amount", new ObservedValue.Decimal(new BigDecimal("1.51"))))),
                new Answer("tagged", 0, "{\"tags\":[2,1],\"by\":{\"b\":2,\"a\":1}}",
                        new ObservedValue.Constructed(tags, ordered(
                                "tags", sequence(2, 1),
                                "by", mapping("b", 2, "a", 1)))),
                new Answer("tagged", 0, "{\"tags\":[2],\"by\":{\"b\":2,\"a\":1}}",
                        new ObservedValue.Constructed(tags, ordered(
                                "tags", sequence(2),
                                "by", mapping("b", 2, "a", 1)))),
                new Answer("tagged", 0, "{\"tags\":[1,2],\"by\":{\"a\":2,\"b\":1}}",
                        new ObservedValue.Constructed(tags, ordered(
                                "tags", sequence(1, 2),
                                "by", mapping("a", 2, "b", 1)))),
                new Answer("looked", 0, "{\"type\":\"Found\",\"id\":7}",
                        foundWith(found, 7)),
                new Answer("looked", 0, "{\"type\":\"Missing\"}", new ObservedValue.Unit(missing)),
                new Answer("looked", 1, "{\"type\":\"Found\",\"id\":2}",
                        foundWith(found, 2)),
                new Answer("looked", 1, "{\"type\":\"Found\",\"id\":7}",
                        foundWith(found, 7)),
                new Answer("looked", 2, "{\"type\":\"Missing\"}", new ObservedValue.Unit(missing)),
                new Answer("looked", 2, "{\"type\":\"Found\",\"id\":2}",
                        foundWith(found, 2)),
                new Answer("numbered", 0, "1", new ObservedValue.Integer(1)),
                new Answer("numbered", 0, "2", new ObservedValue.Integer(2)),
                new Answer("numbered", 0, "Missing", new ObservedValue.Unit(missing)),
                new Answer("numbered", 1, "Missing", new ObservedValue.Unit(missing)),
                new Answer("numbered", 1, "2", new ObservedValue.Integer(2)),
                new Answer("counted", 0, "3", new ObservedValue.Integer(3)),
                new Answer("counted", 0, "4", new ObservedValue.Integer(4)));

        StringBuilder expected = new StringBuilder();
        for (Answer answer : answers) {
            CheckedRow row = behaviorNamed(module, answer.behavior()).rows().get(answer.row());
            Verdict verdict = ((CheckedRow.SelfContained) row.statement()).holds(answer.value());
            expected.append(answer.label()).append(": ")
                    .append(verdict instanceof Verdict.Held ? "kept" : "not kept").append('\n');
        }
        expected.append("counted.1 owed: answered 5\n");
        expected.append("priced.0 handed nothing: unbound\n");

        assertThat(said(program, answers, into)).isEqualTo(expected.toString());
    }

    /** What the library says of each answer, run through a C host that includes only the header. */
    private static String said(CheckedProgram program, List<Answer> answers, Path into)
            throws IOException, InterruptedException {
        NativeCompiler.Library library = NativeCompiler.library(program, into);
        StringJoiner runs = new StringJoiner("\n");
        for (Answer answer : answers) {
            String userdata = answer.behavior().equals("counted")
                    ? "(void *) (intptr_t) " + answer.written()
                    : "(void *) \"" + answer.written().replace("\"", "\\\"") + "\"";
            runs.add("    asserted(\"%s\", souther@_m_held_b_%s_implement, answer_%s, %s, souther@_m_held_b_%s_example_%d);"
                    .formatted(answer.label().replace("\"", "\\\""), answer.behavior(),
                            answer.behavior(), userdata, answer.behavior(), answer.row()));
        }
        String harness = Running.spelt(HARNESS.formatted(runs));
        Path source = into.resolve("held.c");
        Files.writeString(source, harness, StandardCharsets.UTF_8);
        Path executable = into.resolve("held");
        ran(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), source.toString(),
                "-I", library.header().getParent().toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));
        return ran(List.of(executable.toString()));
    }

    /**
     * Each run makes a capability of an implementation answering what it is handed first, hands the
     * capability to the row's function, and says whether the answer kept the row. A status that is
     * not an answer is said as its number, and nothing else is read.
     */
    private static final String HARNESS = """
            #include <inttypes.h>
            #include <stdint.h>
            #include <stdio.h>
            #include <stdlib.h>
            #include <string.h>
            #include "souther.h"

            #define DECODED(type) \\
                static souther_status answer_##type(void *userdata, int64_t n, souther_value *out) { \\
                    (void) n; \\
                    const char *written = userdata; \\
                    souther_decoded reading = NULL; \\
                    souther_status status = souther@_m_held_t_##type##_decode( \\
                            (const uint8_t *) written, (int64_t) strlen(written), &reading); \\
                    if (status != 0) { \\
                        return status; \\
                    } \\
                    if (souther_decoded_outcome(reading) != SOUTHER_DECODED_VALUE) { \\
                        return 1; \\
                    } \\
                    *out = souther_decoded_value(reading); \\
                    return 0; \\
                }

            DECODED(Price)
            DECODED(Tags)
            DECODED(Outcome)
            #define answer_priced answer_Price
            #define answer_tagged answer_Tags
            #define answer_looked answer_Outcome

            /* A number carried into the union, or the one value of Missing. */
            static souther_status answer_numbered(void *userdata, int64_t n, souther_value *out) {
                (void) n;
                const char *written = userdata;
                if (strcmp(written, "Missing") != 0) {
                    *out = souther_case_int_make(strtoll(written, NULL, 10));
                    return 0;
                }
                souther_decoded made = NULL;
                souther_status status = souther@_m_held_t_Missing_construct(&made);
                if (status != 0) {
                    return status;
                }
                *out = souther_decoded_value(made);
                return 0;
            }

            static souther_status answer_counted(void *userdata, int64_t n, int64_t *out) {
                (void) n;
                *out = (int64_t) (intptr_t) userdata;
                return 0;
            }

            static void written(souther_string text) {
                fwrite(souther_string_bytes(text), 1, (size_t) souther_string_length(text), stdout);
            }

            #define asserted(label, implement, implementation, userdata, entry) do { \\
                int64_t scope = souther_scope_open(); \\
                souther_capability capability; \\
                souther_hosted hosted; \\
                implement(&capability, &hosted, implementation, userdata); \\
                const souther_capability *handed[] = {&capability}; \\
                uint8_t kept = 2; \\
                souther_string answer = NULL; \\
                souther_status status = entry(handed, &kept, &answer); \\
                printf("%%s: ", label); \\
                if (status != 0) { \\
                    printf("status %%u\\n", status); \\
                } else { \\
                    printf("%%s\\n", kept == 1 ? "kept" : kept == 0 ? "not kept" : "unwritten"); \\
                } \\
                souther_scope_close(scope); \\
            } while (0)

            int main(void) {
            %s

                int64_t scope = souther_scope_open();
                souther_capability capability;
                souther_hosted hosted;
                souther@_m_held_b_counted_implement(&capability, &hosted, answer_counted,
                        (void *) (intptr_t) 5);
                const souther_capability *handed[] = {&capability};
                souther_string answer = NULL;
                souther_status status = souther@_m_held_b_counted_owed_1(handed, &answer);
                printf("counted.1 owed: ");
                if (status != 0) {
                    printf("status %%u\\n", status);
                } else {
                    printf("answered ");
                    written(answer);
                    printf("\\n");
                }

                uint8_t kept = 2;
                status = souther@_m_held_b_priced_example_0(NULL, &kept, &answer);
                printf("priced.0 handed nothing: %%s\\n",
                        status == SOUTHER_INJECTION_UNBOUND ? "unbound" : "bound");
                souther_scope_close(scope);
                return 0;
            }
            """;

    private static TypeSymbol declared(CheckedModule module, String name) {
        return module.data().stream()
                .map(CheckedData::name)
                .filter(it -> it.toString().endsWith(name))
                .findFirst().orElseThrow();
    }

    private static CheckedBehavior behaviorNamed(CheckedModule module, String name) {
        return module.behaviors().stream()
                .filter(it -> it.name().name().equals(name))
                .findFirst().orElseThrow();
    }

    private static ObservedValue foundWith(TypeSymbol found, long id) {
        return new ObservedValue.Constructed(found, Map.of("id", new ObservedValue.Integer(id)));
    }

    private static ObservedValue sequence(long... elements) {
        return new ObservedValue.Sequence(java.util.Arrays.stream(elements)
                .<ObservedValue>mapToObj(ObservedValue.Integer::new).toList());
    }

    /** A map's entries in the order the external form writes them, which is not part of it. */
    private static ObservedValue mapping(String firstKey, long first, String secondKey, long second) {
        return new ObservedValue.Mapping(List.of(
                new ObservedValue.Entry(new ObservedValue.Text(firstKey),
                        new ObservedValue.Integer(first)),
                new ObservedValue.Entry(new ObservedValue.Text(secondKey),
                        new ObservedValue.Integer(second))));
    }

    private static Map<String, ObservedValue> ordered(String first, ObservedValue one,
                                                      String second, ObservedValue two) {
        Map<String, ObservedValue> fields = new LinkedHashMap<>();
        fields.put(first, one);
        fields.put(second, two);
        return fields;
    }

    private static String ran(List<String> command) throws IOException, InterruptedException {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError(command.getFirst() + " failed: " + said);
        }
        return said;
    }
}
