package souther.bindings;

import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * The runtime functions every library exports, and the words each is called with, as a binding's
 * runtime calls them.
 *
 * <p>The Rust and the Go runtime call each of these by name through the library they load, with
 * these words. They are the same for both, so they are said once: a manifest that says another is
 * one no runtime could call as what it is.
 */
public final class RuntimeFunctions {

    private RuntimeFunctions() {
    }

    /**
     * The runtime's functions a binding's runtime calls, as the manifest has to say them: it looks
     * each up by this name and calls it as these words say, so a manifest saying another is one
     * it would call as something it is not.
     */
    public static final Map<String, Function> CALLED = runtime();

    private static Map<String, Function> runtime() {
        Map<String, Function> functions = new LinkedHashMap<>();
        java.util.function.BiConsumer<String, List<Object>> add = (name, words) -> {
            List<Parameter> takes = new ArrayList<>();
            for (Object word : words.subList(0, words.size() - 1)) {
                takes.add(Parameter.given((Word) word));
            }
            functions.put(name, new Function(name, takes, (Word) words.getLast()));
        };
        functions.put("souther_mark", new Function("souther_mark", List.of(), Word.MARK));
        functions.put("souther_reset",
                new Function("souther_reset", List.of(Parameter.given(Word.MARK)), null));
        // A String has no place for text past what the language bounds it to
        // (souther-native-compiler#109), so this answers whether it wrote one, as a generated
        // string operation already does, in place of always answering a String.
        functions.put("souther_string_of_utf8", new Function("souther_string_of_utf8",
                List.of(Parameter.given(Word.BYTES), Parameter.given(Word.COUNT),
                        Parameter.room(Word.STRING)),
                Word.BOOL));
        add.accept("souther_string_length", List.of(Word.STRING, Word.COUNT));
        add.accept("souther_string_bytes", List.of(Word.STRING, Word.BYTES));
        // The unscaled digits as bytes and a count, not a String: they are the integer's text and
        // never the value's written form, so they are never fallible on what a String holds
        // (souther-native-compiler#109).
        add.accept("souther_decimal_of_parts", List.of(Word.BYTES, Word.COUNT, Word.INT, Word.DECIMAL));
        add.accept("souther_decimal_unscaled", List.of(Word.DECIMAL, Word.STRING));
        add.accept("souther_decimal_scale", List.of(Word.DECIMAL, Word.INT));
        add.accept("souther_date_of_iso", List.of(Word.STRING, Word.DATE));
        add.accept("souther_date_iso", List.of(Word.DATE, Word.STRING));
        add.accept("souther_time_of_iso", List.of(Word.STRING, Word.TIME));
        add.accept("souther_time_iso", List.of(Word.TIME, Word.STRING));
        add.accept("souther_datetime_of_iso", List.of(Word.STRING, Word.DATETIME));
        add.accept("souther_datetime_iso", List.of(Word.DATETIME, Word.STRING));
        add.accept("souther_instant_of_iso", List.of(Word.STRING, Word.INSTANT));
        add.accept("souther_instant_iso", List.of(Word.INSTANT, Word.STRING));
        add.accept("souther_decoded_outcome", List.of(Word.DECODED, Word.OUTCOME));
        add.accept("souther_decoded_value", List.of(Word.DECODED, Word.VALUE));
        add.accept("souther_decoded_malformed_at", List.of(Word.DECODED, Word.COUNT));
        add.accept("souther_decoded_issue_count", List.of(Word.DECODED, Word.COUNT));
        add.accept("souther_decoded_issue", List.of(Word.DECODED, Word.COUNT, Word.ISSUE));
        add.accept("souther_issue_code", List.of(Word.ISSUE, Word.STRING));
        add.accept("souther_issue_message_key", List.of(Word.ISSUE, Word.STRING));
        add.accept("souther_issue_path", List.of(Word.ISSUE, Word.STRING));
        add.accept("souther_issue_meta", List.of(Word.ISSUE, Word.STRING));
        return Map.copyOf(functions);
    }

    /**
     * Refuses a manifest whose runtime functions {@code runtime} would call as something they are
     * not: each it calls is there, taking and answering what it calls it with.
     *
     * @param runtime what the binding's runtime is called, for the message
     */
    public static void check(Manifest manifest, String runtime) {
        Map<String, Function> said = new LinkedHashMap<>();
        manifest.runtime().forEach(it -> said.put(it.name(), it));
        for (Function expected : CALLED.values()) {
            Function it = said.get(expected.name());
            if (it == null || !it.takes().equals(expected.takes())
                    || it.answers() != expected.answers()) {
                throw new IllegalArgumentException("the manifest says the runtime's "
                        + expected.name() + " is " + it + ", and the " + runtime + " runtime calls it as "
                        + expected);
            }
        }
    }
}
