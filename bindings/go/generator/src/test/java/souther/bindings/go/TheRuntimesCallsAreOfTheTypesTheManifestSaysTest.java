package souther.bindings.go;

import org.junit.jupiter.api.Test;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Word;
import souther.bindings.RuntimeFunctions;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The runtime module calls the library's own runtime functions through C it has no declarations
 * for, so each call is written there as the type its words make, with an address as a plain pointer.
 * A generated package asserts the declarations against the words ({@code abi.go}); this holds what the
 * runtime writes to the same words, so the two cannot come apart with the declarations between them
 * saying nothing to either.
 */
class TheRuntimesCallsAreOfTheTypesTheManifestSaysTest {

    /** The type of a function as the runtime writes a call through it: its address as a pointer. */
    private static String plain(Function function) {
        List<String> parameters = new ArrayList<>();
        for (Parameter it : function.takes()) {
            parameters.add(switch (it.mode()) {
                case GIVEN -> CTypes.scalar(it.word()) != null ? CTypes.scalar(it.word())
                        : it.word() == Word.BYTES ? "const uint8_t *" : "void *";
                case ROOM -> CTypes.scalar(it.word()) != null ? CTypes.scalar(it.word()) + " *"
                        : "void **";
                case SLICE -> throw new IllegalArgumentException("the runtime reads no slice");
            });
        }
        Word answers = function.answers();
        String returns = answers == null ? "void" : CTypes.scalar(answers) != null
                ? CTypes.scalar(answers) : answers == Word.BYTES ? "const uint8_t *" : "void *";
        return normal(returns + "|" + (parameters.isEmpty() ? "void" : String.join(", ", parameters)));
    }

    private static String normal(String type) {
        return type.replaceAll("\\s+", " ").replace(" ,", ",").trim();
    }

    /** The shim of the runtime module that calls each function of the library's runtime. */
    private static final Map<String, String> SHIM = Map.ofEntries(
            Map.entry("souther_mark", "mark"),
            Map.entry("souther_reset", "reset"),
            Map.entry("souther_string_of_utf8", "string_of_utf8"),
            Map.entry("souther_string_length", "string_length"),
            Map.entry("souther_string_bytes", "string_bytes"),
            Map.entry("souther_decimal_of_parts", "decimal_of_parts"),
            Map.entry("souther_decimal_unscaled", "decimal_unscaled"),
            Map.entry("souther_decimal_scale", "decimal_scale"),
            Map.entry("souther_date_of_parts", "of_three_parts"),
            Map.entry("souther_date_parts", "three_parts"),
            Map.entry("souther_time_of_parts", "of_three_parts"),
            Map.entry("souther_time_parts", "three_parts"),
            Map.entry("souther_datetime_of_parts", "of_six_parts"),
            Map.entry("souther_datetime_parts", "six_parts"),
            Map.entry("souther_instant_of_parts", "of_two_parts"),
            Map.entry("souther_instant_parts", "two_parts"),
            Map.entry("souther_decoded_outcome", "decoded_outcome"),
            Map.entry("souther_decoded_value", "decoded_value"),
            Map.entry("souther_decoded_malformed_at", "decoded_malformed_at"),
            Map.entry("souther_decoded_issue_count", "decoded_issue_count"),
            Map.entry("souther_decoded_issue", "decoded_issue"),
            Map.entry("souther_issue_code", "issue_text"),
            Map.entry("souther_issue_message_key", "issue_text"),
            Map.entry("souther_issue_path", "issue_text"),
            Map.entry("souther_issue_meta", "issue_text"));

    /** The type of the function each shim of the runtime module casts an address to, by shim. */
    private static Map<String, String> written() throws IOException {
        Map<String, String> types = new TreeMap<>();
        Pattern shim = Pattern.compile("static [^(]*?call_(\\w+)\\(void \\*fn[^)]*\\)\\s*\\{\\s*(?:return\\s*)?"
                + "\\(\\(([^()]*?)\\s*\\(\\*\\)\\(([^()]*)\\)\\)fn\\)", Pattern.DOTALL);
        for (String file : List.of("words_unix.go", "native_unix.go")) {
            Matcher it = shim.matcher(Files.readString(GoHost.RUNTIME.resolve(file),
                    StandardCharsets.UTF_8));
            while (it.find()) {
                assertThat(types.put(it.group(1), normal(it.group(2) + "|" + it.group(3))))
                        .as("the shim %s is written twice", it.group(1)).isNull();
            }
        }
        return types;
    }

    @Test
    void everyFunctionTheRuntimeCallsIsCalledAsItsWordsSayAndNoShimIsWrittenForNone() throws IOException {
        Map<String, String> written = written();

        assertThat(SHIM.keySet()).as("every function of the runtime the library exports has a shim")
                .isEqualTo(RuntimeFunctions.CALLED.keySet());
        assertThat(written.keySet()).as("every shim is one of a function's")
                .isEqualTo(new java.util.TreeSet<>(SHIM.values()));
        RuntimeFunctions.CALLED.forEach((symbol, function) ->
                assertThat(written.get(SHIM.get(symbol)))
                        .as("the shim of %s", symbol).isEqualTo(plain(function)));
    }
}
