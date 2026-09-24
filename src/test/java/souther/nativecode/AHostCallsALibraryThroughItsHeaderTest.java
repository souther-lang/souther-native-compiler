package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.program.CheckedProgram;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A host calls a program built for it the way a C program does: it includes the header the build
 * wrote, links the shared library the build wrote, and declares nothing itself.
 *
 * <p>No {@code __asm__} label, and no symbol spelt by the test: every name the harness calls is one
 * the header declares, so a harness that compiles is one the header was enough for. And what the
 * header declares, what the manifest describes and what the library exports are asked of each of
 * them as it is, and held to be one set.
 */
class AHostCallsALibraryThroughItsHeaderTest {

    private static final String SHOP = """
            module shop exposing ( Money, Line, Free, Paid, Owed, Settled, Outcome, settle, owing )

            data Money = Int
                invariant notNegative = value >= 0

            data Line = { price: Money, quantity: Int, note: String? }
                invariant some = quantity > 0

            data Free
            data Paid = { amount: Money }
            data Waived = { reason: String }
            data Owed = { amount: Money, overdue: Bool }
            data Settled = Free | Paid | Waived
            data Outcome = Settled | Owed

            behavior settle : (line: Line, paid: Int) -> Outcome
            let settle (line, paid) = {
                let due = line.price.value * line.quantity
                if due == 0 then Free
                else if paid >= due then Paid { amount = Money(due) }
                else Owed { amount = Money(due - paid), overdue = paid == 0 }
            }

            behavior owing : (outcome: Outcome) -> Int
            let owing (outcome) = match outcome with
                | Owed as o -> o.amount.value
                | Settled -> 0

            behavior twice : (n: Int) -> Int
            let twice (n) = n * 2

            example twice
                | "two" : (1) -> 2
            """;

    private static final String HARNESS = """
            #include <inttypes.h>
            #include <stdio.h>
            #include <string.h>
            #include "souther.h"

            static void text(souther_string said) {
                printf("%.*s", (int) souther_string_length(said),
                       (const char *) souther_string_bytes(said));
            }

            static void decoded(const char *label, const char *json) {
                souther_decoded reading = NULL;
                souther_status status = souther2_m_shop_t_Line_decode(
                        (const uint8_t *) json, (int64_t) strlen(json), &reading);
                printf("%s: status %u", label, status);
                switch (souther_decoded_outcome(reading)) {
                case SOUTHER_DECODED_VALUE:
                    printf(", quantity %" PRId64,
                           souther2_m_shop_t_Line_f_quantity(souther_decoded_value(reading)));
                    break;
                case SOUTHER_DECODED_ISSUES:
                    for (int64_t at = 0; at < souther_decoded_issue_count(reading); at++) {
                        souther_issue issue = souther_decoded_issue(reading, at);
                        printf(", [");
                        text(souther_issue_path(issue));
                        printf(" ");
                        text(souther_issue_code(issue));
                        printf("]");
                    }
                    break;
                case SOUTHER_DECODED_MALFORMED:
                    printf(", malformed at %" PRId64, souther_decoded_malformed_at(reading));
                    break;
                }
                printf("\\n");
            }

            int main(void) {
                int64_t mark = souther_mark();

                souther_value three = NULL;
                souther_status status = souther2_m_shop_t_Money_construct(3, &three);
                printf("money: status %u, value %" PRId64 "\\n", status,
                       souther2_m_shop_t_Money_f_value(three));
                souther_value below = NULL;
                status = souther2_m_shop_t_Money_construct(-1, &below);
                printf("below: %d, untouched %d\\n", status == SOUTHER_INVARIANT_NOT_HELD,
                       below == NULL);

                const char *wrap = "gift wrap";
                souther_string note = souther_string_of_utf8((const uint8_t *) wrap,
                                                             (int64_t) strlen(wrap));
                souther_value line = NULL;
                status = souther2_m_shop_t_Line_construct(three, 2, 1, note, &line);
                souther_string noted = NULL;
                uint8_t present = souther2_m_shop_t_Line_f_note(line, &noted);
                printf("line: status %u, note %u ", status, present);
                text(noted);
                printf("\\n");

                souther_value outcome = NULL;
                status = souther2_m_shop_b_settle(line, 2, &outcome);
                int64_t owed = -1;
                souther_status owing = souther2_m_shop_b_owing(outcome, &owed);
                printf("settled: status %u, case %u, amount %" PRId64 ", owing %u %" PRId64 "\\n",
                       status, souther2_m_shop_t_Outcome_case(outcome),
                       souther2_m_shop_t_Money_f_value(souther2_m_shop_t_Owed_f_amount(outcome)),
                       owing, owed);

                printf("written: ");
                text(souther2_m_shop_t_Line_encode(line));
                printf("\\n");
                decoded("read", "{\\"price\\": 4, \\"quantity\\": 5}");
                decoded("read wrong", "{\\"price\\": -1, \\"quantity\\": 5}");
                decoded("not json", "{\\"price\\"");

                souther_reset(mark);
                return 0;
            }
            """;

    private static final JsonMapper JSON = JsonMapper.builder().build();

    @Test
    void aCProgramIncludingOnlyTheHeaderCallsTheLibrary(@TempDir Path into) throws Exception {
        assertThat(HARNESS).doesNotContain("__asm__").doesNotContain("extern");
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(SHOP)), into);

        Path source = into.resolve("host.c");
        Files.writeString(source, HARNESS, StandardCharsets.UTF_8);
        Path executable = into.resolve("host");
        said(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), source.toString(),
                "-I", library.header().getParent().toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));

        assertThat(said(List.of(executable.toString()))).isEqualTo("""
                money: status 0, value 3
                below: 1, untouched 1
                line: status 0, note 1 gift wrap
                settled: status 0, case 3, amount 4, owing 0 4
                written: {"price":3,"quantity":2,"note":"gift wrap"}
                read: status 0, quantity 5
                read wrong: status 0, [/price invariant_violation]
                not json: status 0, malformed at 8
                """);
    }

    /**
     * The header, the manifest and the library name one set of functions, and the library exports
     * nothing a host does not call: not a row's entry, not a boundary, not what one object built by
     * this compiler reaches in another, and none of the runtime's own.
     */
    @Test
    void theHeaderTheManifestAndTheLibraryNameOneSet(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(SHOP)), into);

        Set<String> declared = declaredIn(Files.readString(library.header()));
        Set<String> described = describedIn(JSON.readTree(library.manifest().toFile()));
        Set<String> exported = exportedBy(library.library());

        assertThat(declared).isNotEmpty();
        assertThat(described).isEqualTo(declared);
        assertThat(exported).isEqualTo(declared);

        // So that what is not exported is something the object does hold.
        Set<String> inTheObject = definedIn(library.object());
        assertThat(inTheObject).anyMatch(it -> it.contains("$example$"));
        assertThat(inTheObject).anyMatch(it -> it.endsWith("$boundary"));
        assertThat(inTheObject).contains("souther" + Running.ABI + ".shop.settle");
        assertThat(exported).noneMatch(it -> it.contains("$") || it.contains("."));
        assertThat(exported).doesNotContain("souther_alloc", "souther_decode_begin",
                "souther_read_int", "souther_external_json");
    }

    private static final Pattern DECLARATION = Pattern.compile("([A-Za-z0-9_]+)\\(.*\\);$");

    private static Set<String> declaredIn(String header) {
        Set<String> declared = new TreeSet<>();
        for (String line : header.lines().toList()) {
            Matcher found = DECLARATION.matcher(line);
            if (found.find()) {
                declared.add(found.group(1));
            }
        }
        return declared;
    }

    private static Set<String> describedIn(JsonNode manifest) {
        Set<String> described = new TreeSet<>();
        List<JsonNode> functions = new ArrayList<>();
        manifest.get("runtime").forEach(functions::add);
        for (JsonNode module : manifest.get("modules")) {
            module.get("behaviors").forEach(it -> functions.add(it.get("call")));
            module.get("values").forEach(it -> functions.add(it.get("read")));
            for (JsonNode declaration : module.get("declarations")) {
                for (String operation : List.of("construct", "case", "decode", "encode")) {
                    functions.add(declaration.get(operation));
                }
                declaration.get("fields").forEach(it -> functions.add(it.get("read")));
            }
        }
        for (JsonNode function : functions) {
            if (function != null && !function.isNull()) {
                described.add(function.get("name").stringValue());
            }
        }
        return described;
    }

    private static Set<String> exportedBy(Path library) throws IOException, InterruptedException {
        return symbols(switch (Running.HOST) {
            case DARWIN -> List.of("nm", "-gU", library.toString());
            case LINUX -> List.of("nm", "-D", "--defined-only", library.toString());
        });
    }

    private static Set<String> definedIn(Path object) throws IOException, InterruptedException {
        return symbols(switch (Running.HOST) {
            case DARWIN -> List.of("nm", "-gU", object.toString());
            case LINUX -> List.of("nm", "-g", "--defined-only", object.toString());
        });
    }

    private static Set<String> symbols(List<String> command)
            throws IOException, InterruptedException {
        Set<String> named = new TreeSet<>();
        for (String line : said(command).lines().toList()) {
            String[] fields = line.trim().split("\\s+");
            if (fields.length == 3) {
                String name = fields[2];
                named.add(name.startsWith(Running.PREFIX)
                        ? name.substring(Running.PREFIX.length()) : name);
            }
        }
        return named;
    }

    private static String said(List<String> command) throws IOException, InterruptedException {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError(command.get(0) + " failed: " + said);
        }
        return said;
    }
}
