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
            module shop exposing ( Money, Line, Free, Paid, Owed, Settled, Outcome, settle, owing, stillOwing : Int,
                                   charge )

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

            behavior stillOwing = settle >-> owing

            behavior charge : (paid: Int) -> Owed | Settled
            let charge (paid) = if paid > 0 then Free else Owed { amount = Money(1), overdue = true }

            behavior twice : (n: Int) -> Int
            let twice (n) = n * 2

            behavior discountFor : (line: Line) -> Int

            behavior discounted : (line: Line) -> Int
                depends on discountFor
            let discounted (line, discountFor) = line.price.value * line.quantity - discountFor(line)

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
                souther_status status = souther3_m_shop_t_Line_decode(
                        (const uint8_t *) json, (int64_t) strlen(json), &reading);
                printf("%s: status %u", label, status);
                switch (souther_decoded_outcome(reading)) {
                case SOUTHER_DECODED_VALUE:
                    printf(", quantity %" PRId64,
                           souther3_m_shop_t_Line_f_quantity(souther_decoded_value(reading)));
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
                souther_status status = souther3_m_shop_t_Money_construct(3, &three);
                printf("money: status %u, value %" PRId64 "\\n", status,
                       souther3_m_shop_t_Money_f_value(three));
                souther_value below = NULL;
                status = souther3_m_shop_t_Money_construct(-1, &below);
                printf("below: %d\\n", status == SOUTHER_INVARIANT_NOT_HELD && below == NULL);

                const char *wrap = "gift wrap";
                souther_string note = souther_string_of_utf8((const uint8_t *) wrap,
                                                             (int64_t) strlen(wrap));
                souther_value line = NULL;
                status = souther3_m_shop_t_Line_construct(three, 2, 1, note, &line);
                souther_string noted = NULL;
                uint8_t present = souther3_m_shop_t_Line_f_note(line, &noted);
                printf("line: status %u, note %u ", status, present);
                text(noted);
                printf("\\n");

                souther_value outcome = NULL;
                status = souther3_m_shop_b_settle(line, 2, &outcome);
                int64_t owed = -1;
                souther_status owing = souther3_m_shop_b_owing(outcome, &owed);
                printf("settled: status %u, case %u, amount %" PRId64 ", owing %u %" PRId64 "\\n",
                       status, souther3_m_shop_t_Outcome_case(outcome),
                       souther3_m_shop_t_Money_f_value(souther3_m_shop_t_Owed_f_amount(outcome)),
                       owing, owed);

                souther_value owes = NULL;
                status = souther3_m_shop_b_charge(0, &owes);
                souther_value free = NULL;
                souther_status freed = souther3_m_shop_b_charge(1, &free);
                printf("charged: status %u, case %u, status %u, case %u\\n", status,
                       souther3_m_shop_b_charge_answer_case(owes), freed,
                       souther3_m_shop_b_charge_answer_case(free));

                printf("written: ");
                text(souther3_m_shop_t_Line_encode(line));
                printf("\\n");
                decoded("read", "{\\"price\\": 4, \\"quantity\\": 5}");
                decoded("read wrong", "{\\"price\\": -1, \\"quantity\\": 5}");
                decoded("not json", "{\\"price\\"");

                souther_reset(mark);
                return 0;
            }
            """;

    /**
     * The same, from PHP: the declarations handed to {@code FFI::cdef} as they were written, which
     * is the reader with no preprocessor the declarations are for.
     */
    private static final String PHP = """
            <?php
            $ffi = FFI::cdef(file_get_contents($argv[1]), $argv[2]);

            function bytes($ffi, string $text) {
                $held = $ffi->new("uint8_t[" . max(1, strlen($text)) . "]");
                FFI::memcpy($held, $text, strlen($text));
                return $held;
            }

            function text($ffi, $string): string {
                return FFI::string($ffi->souther_string_bytes($string),
                        $ffi->souther_string_length($string));
            }

            function decoded($ffi, string $label, string $json): void {
                $reading = $ffi->new("souther_decoded");
                $status = $ffi->souther3_m_shop_t_Line_decode(bytes($ffi, $json), strlen($json),
                        FFI::addr($reading));
                echo "$label: status $status";
                $outcome = $ffi->souther_decoded_outcome($reading);
                if ($outcome === $ffi->SOUTHER_DECODED_VALUE) {
                    echo ", quantity ", $ffi->souther3_m_shop_t_Line_f_quantity(
                            $ffi->souther_decoded_value($reading));
                } elseif ($outcome === $ffi->SOUTHER_DECODED_ISSUES) {
                    for ($at = 0; $at < $ffi->souther_decoded_issue_count($reading); $at++) {
                        $issue = $ffi->souther_decoded_issue($reading, $at);
                        echo ", [", text($ffi, $ffi->souther_issue_path($issue)), " ",
                                text($ffi, $ffi->souther_issue_code($issue)), "]";
                    }
                } else {
                    echo ", malformed at ", $ffi->souther_decoded_malformed_at($reading);
                }
                echo "\n";
            }

            $mark = $ffi->souther_mark();

            $three = $ffi->new("souther_value");
            $status = $ffi->souther3_m_shop_t_Money_construct(3, FFI::addr($three));
            echo "money: status $status, value ", $ffi->souther3_m_shop_t_Money_f_value($three), "\n";
            $below = $ffi->new("souther_value");
            $status = $ffi->souther3_m_shop_t_Money_construct(-1, FFI::addr($below));
            echo "below: ", (int) ($status === $ffi->SOUTHER_INVARIANT_NOT_HELD && FFI::isNull($below)),
                    "\n";

            $wrap = "gift wrap";
            $note = $ffi->souther_string_of_utf8(bytes($ffi, $wrap), strlen($wrap));
            $line = $ffi->new("souther_value");
            $status = $ffi->souther3_m_shop_t_Line_construct($three, 2, 1, $note, FFI::addr($line));
            $noted = $ffi->new("souther_string");
            $present = $ffi->souther3_m_shop_t_Line_f_note($line, FFI::addr($noted));
            echo "line: status $status, note $present ", text($ffi, $noted), "\n";

            $outcome = $ffi->new("souther_value");
            $status = $ffi->souther3_m_shop_b_settle($line, 2, FFI::addr($outcome));
            $owed = $ffi->new("int64_t");
            $owing = $ffi->souther3_m_shop_b_owing($outcome, FFI::addr($owed));
            echo "settled: status $status, case ", $ffi->souther3_m_shop_t_Outcome_case($outcome),
                    ", amount ", $ffi->souther3_m_shop_t_Money_f_value(
                            $ffi->souther3_m_shop_t_Owed_f_amount($outcome)),
                    ", owing $owing ", $owed->cdata, "\n";

            $owes = $ffi->new("souther_value");
            $status = $ffi->souther3_m_shop_b_charge(0, FFI::addr($owes));
            $free = $ffi->new("souther_value");
            $freed = $ffi->souther3_m_shop_b_charge(1, FFI::addr($free));
            echo "charged: status $status, case ", $ffi->souther3_m_shop_b_charge_answer_case($owes),
                    ", status $freed, case ", $ffi->souther3_m_shop_b_charge_answer_case($free), "\n";

            echo "written: ", text($ffi, $ffi->souther3_m_shop_t_Line_encode($line)), "\n";
            decoded($ffi, "read", '{"price": 4, "quantity": 5}');
            decoded($ffi, "read wrong", '{"price": -1, "quantity": 5}');
            decoded($ffi, "not json", '{"price"');

            $ffi->souther_reset($mark);
            """;

    /** What both hosts are answered, which is the one program asked the same things. */
    private static final String ANSWERED = """
            money: status 0, value 3
            below: 1
            line: status 0, note 1 gift wrap
            settled: status 0, case 3, amount 4, owing 0 4
            charged: status 0, case 0, status 0, case 1
            written: {"price":3,"quantity":2,"note":"gift wrap"}
            read: status 0, quantity 5
            read wrong: status 0, [/price invariant_violation]
            not json: status 0, malformed at 8
            """;

    /** What version 4 of the manifest is, for the program above. */
    private static final Path INTERFACE_V4 =
            Path.of("native", "crates", "compiler", "tests", "interface-v4.json");

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

        assertThat(said(List.of(executable.toString()))).isEqualTo(ANSWERED);
    }

    @Test
    void phpDeclaresTheFunctionsFromTheDeclarationsAndCallsTheLibrary(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(SHOP)), into);
        Path script = into.resolve("host.php");
        Files.writeString(script, PHP, StandardCharsets.UTF_8);

        assertThat(Php.ran(List.of("-d", "ffi.enable=1", script.toString(),
                library.declarations().toString(), library.library().toString())))
                .isEqualTo(ANSWERED);
    }

    /**
     * The manifest a binding is written against, as version 4 says it for this program. A change
     * to what the manifest says is a change here, and whether it moves the version is decided
     * looking at it.
     */
    @Test
    void theManifestIsWhatVersionFourSays(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(SHOP)), into);

        String written = Files.readString(library.manifest(), StandardCharsets.UTF_8);
        String fixed = Files.exists(INTERFACE_V4)
                ? Files.readString(INTERFACE_V4, StandardCharsets.UTF_8) : "";
        if (!written.equals(fixed)) {
            // Kept where it can be compared with the fixture, and copied over it once it is read.
            Files.writeString(Path.of("target", "interface-v4.written.json"), written,
                    StandardCharsets.UTF_8);
        }
        assertThat(written).isEqualTo(fixed);
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

        String declarations = Files.readString(library.declarations());
        assertThat(declarations.lines()).noneMatch(line -> line.strip().startsWith("#"));
        assertThat(Files.readString(library.header()))
                .contains("#include \"" + library.declarations().getFileName() + "\"");
        Set<String> declared = declaredIn(declarations);
        Set<String> described = describedIn(JSON.readTree(library.manifest().toFile()));
        Set<String> exported = exportedBy(library.library());

        assertThat(declared).isNotEmpty();
        assertThat(described).isEqualTo(declared);
        assertThat(exported).isEqualTo(declared);

        // So that what is not exported is something the object does hold.
        Set<String> inTheObject = definedIn(library.object());
        assertThat(inTheObject).anyMatch(it -> it.contains("$example$"));
        assertThat(inTheObject).anyMatch(it -> it.endsWith("$boundary"));
        assertThat(inTheObject).contains("souther" + Running.ABI + ".shop.settle",
                "souther" + Running.ABI + ".shop.discountFor");
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

    /**
     * Every function the manifest names, wherever it names one: every member shaped as a function
     * is (a name, what it takes and what it answers, and nothing else), and what a host registers
     * an implementation through, and not what it registers, whose name is a type's. Found by
     * walking the whole manifest rather than by a list of where functions are kept, so a function
     * a later version puts somewhere new is held to the header and the library without this
     * having to be told.
     */
    private static Set<String> describedIn(JsonNode manifest) {
        Set<String> described = new TreeSet<>();
        walk(manifest, described);
        return described;
    }

    private static void walk(JsonNode node, Set<String> described) {
        if (node.isObject()) {
            if (new TreeSet<>(node.propertyNames()).equals(Set.of("answers", "name", "takes"))) {
                described.add(node.get("name").stringValue());
            }
            if (node.has("register")) {
                described.add(node.get("register").stringValue());
            }
        }
        node.forEach(it -> walk(it, described));
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
