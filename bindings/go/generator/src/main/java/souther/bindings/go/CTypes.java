package souther.bindings.go;

import org.jspecify.annotations.Nullable;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.List;

/**
 * What C calls the type of each word a function of the library takes and answers, as the manifest
 * says the words are.
 *
 * <p>This is the one place Go's binding says what the declarations' types are, and it is held to
 * them where they meet: every function a generated package calls is asserted, by the C compiler,
 * to have the type made here of its words ({@link #prototype}), so a word that is a number of
 * another width, or an address of another type, than the declarations say is a build that fails
 * and not a call that converts one into the other without a word.
 */
final class CTypes {

    private CTypes() {
    }

    /** What C calls the type of a word that is a number. */
    static @Nullable String scalar(Word word) {
        // The type its representation is, as the ABI records it.
        return switch (word.representation()) {
            case U8 -> "uint8_t";
            case U32 -> "uint32_t";
            case I32 -> "int32_t";
            case I64 -> "int64_t";
            case ADDRESS -> null;
        };
    }

    /** What the declarations call the type of a word that is an address of the library's. */
    static @Nullable String opaque(Word word) {
        return switch (word) {
            case VALUE -> "souther_value";
            case STRING -> "souther_string";
            case DECIMAL -> "souther_decimal";
            case DATE -> "souther_date";
            case TIME -> "souther_time";
            case DATETIME -> "souther_datetime";
            case INSTANT -> "souther_instant";
            case DECODED -> "souther_decoded";
            case ISSUE -> "souther_issue";
            case LIST -> "souther_list";
            case FUNCTION -> "souther_function";
            default -> null;
        };
    }

    /** The type of a word a function is handed. */
    static String given(Word word) {
        String number = scalar(word);
        String address = opaque(word);
        if (number != null) {
            return number;
        }
        if (address != null) {
            return address;
        }
        return switch (word) {
            case BYTES -> "const uint8_t *";
            case REQUIREMENTS -> "const souther_capability *const *";
            case USERDATA -> "void *";
            default -> throw new IllegalArgumentException("a function is not handed a " + word);
        };
    }

    /** The type of the room a function writes a word through. */
    static String room(Word word) {
        if (word == Word.CAPABILITY) {
            return "souther_capability *";
        }
        String number = scalar(word);
        String address = opaque(word);
        if (number != null) {
            return number + " *";
        }
        if (address != null) {
            return address + " *";
        }
        throw new IllegalArgumentException("a function writes no " + word + " through room");
    }

    /** The type of the elements a function reads as many of as another parameter counts. */
    static String slice(Word word) {
        String number = scalar(word);
        String address = opaque(word);
        if (number != null) {
            return "const " + number + " *";
        }
        if (address != null) {
            return "const " + address + " *";
        }
        throw new IllegalArgumentException("a function reads no slice of " + word);
    }

    /** The type of what a function answers. */
    static String answered(@Nullable Word word) {
        if (word == null) {
            return "void";
        }
        return word == Word.BYTES ? "const uint8_t *" : given(word);
    }

    /** The type of one parameter, as its mode says it is taken. */
    static String parameter(Parameter parameter) {
        return switch (parameter.mode()) {
            case GIVEN -> given(parameter.word());
            case ROOM -> room(parameter.word());
            case SLICE -> slice(parameter.word());
        };
    }

    /**
     * The type of a pointer to {@code function}, named {@code name}: {@code typedef} it and it is
     * the type of what the declarations declare, or what the manifest says is not what they say.
     */
    static String prototype(Function function, String name) {
        List<String> parameters = new ArrayList<>();
        for (Parameter it : function.takes()) {
            parameters.add(parameter(it));
        }
        return answered(function.answers()) + " (*" + name + ")("
                + (parameters.isEmpty() ? "void" : String.join(", ", parameters)) + ")";
    }

    /**
     * The C that has the compiler assert that the function the declarations declare as {@code
     * symbol} is of the type the manifest's words make of {@code function}, spelled as a typedef
     * named {@code expected_<symbol>}.
     */
    static String asserted(Function function, String symbol) {
        String expected = "expected_" + symbol;
        return "typedef " + prototype(function, expected) + ";\n"
                + "_Static_assert(__builtin_types_compatible_p(__typeof__(&" + symbol + "), "
                + expected + "),\n\t\"" + symbol + " is not the function the manifest says it is\");\n";
    }
}
