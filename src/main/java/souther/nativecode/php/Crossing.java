package souther.nativecode.php;

import org.jspecify.annotations.Nullable;
import souther.nativecode.php.Manifest.Word;

import java.util.List;

/**
 * How a value of one model type crosses between PHP and the library: what PHP calls its type, the
 * words it is handed over as, and how PHP makes a value of the word it is handed back.
 *
 * <p>An optional crosses as whether it is there and then the value, which is how the library hands
 * one over both ways; everything else crosses as one word.
 */
sealed interface Crossing {

    /** What PHP calls a value of this, as a parameter or an answer is typed. */
    String phpType();

    /** The words it is handed over as, in order. */
    List<Word> words();

    /** The PHP expressions handing {@code value} over, one for each of {@link #words()}. */
    List<String> given(String value, String session);

    /** A PHP expression true where {@code value} is a value of this. */
    String holds(String value);

    /**
     * The PHP expression making a value of this out of what the library answered, one expression
     * for each of {@link #words()}: a word returned or handed to an implementation as itself, or
     * read out of room with {@link Whole#fromRoom}.
     */
    String of(List<String> words, String session);

    /** One word of the library's. */
    record Whole(Word word, String phpType, Kind kind, @Nullable String declared) implements Crossing {

        enum Kind { INT, BOOL, STRING, PRODUCT, SUM }

        static Whole integer() {
            return new Whole(Word.INT, "int", Kind.INT, null);
        }

        static Whole truth() {
            return new Whole(Word.BOOL, "bool", Kind.BOOL, null);
        }

        static Whole text() {
            return new Whole(Word.STRING, "string", Kind.STRING, null);
        }

        /** A value of a declared type that is not a sum, as the class generated for it. */
        static Whole product(String fqcn) {
            return new Whole(Word.VALUE, fqcn, Kind.PRODUCT, fqcn);
        }

        /**
         * A value of a sum, as the interface generated for it, and made through what decides which
         * of its classes a value is.
         */
        static Whole sum(String iface, String codec) {
            return new Whole(Word.VALUE, iface, Kind.SUM, codec);
        }

        @Override
        public List<Word> words() {
            return List.of(word);
        }

        @Override
        public List<String> given(String value, String session) {
            return List.of(switch (kind) {
                case INT -> value;
                case BOOL -> "(" + value + " ? 1 : 0)";
                case STRING -> session + "->string(" + value + ")";
                case PRODUCT, SUM -> value + "->nativeHandle()->borrow(" + session + ")";
            });
        }

        /** What handing over nothing is, where an optional holds no value. */
        String absent() {
            return switch (kind) {
                case INT, BOOL -> "0";
                case STRING, PRODUCT, SUM -> "null";
            };
        }

        @Override
        public String holds(String value) {
            return switch (kind) {
                case INT -> "\\is_int(" + value + ")";
                case BOOL -> "\\is_bool(" + value + ")";
                case STRING -> "\\is_string(" + value + ")";
                case PRODUCT, SUM -> value + " instanceof " + phpType;
            };
        }

        @Override
        public String of(List<String> words, String session) {
            String word = words.getFirst();
            return switch (kind) {
                case INT -> word;
                case BOOL -> "(" + word + " !== 0)";
                case STRING -> session + "->text(" + word + ")";
                case PRODUCT -> "new " + declared + "(" + session + "->handle(" + word + "))";
                case SUM -> declared + "::wrap(" + session + ", " + word + ")";
            };
        }

        /** What C calls room for this word. */
        String cType() {
            return cType(word);
        }

        /** The word held in room {@code room} names, as {@link #of} takes it. */
        String fromRoom(String room) {
            return switch (kind) {
                case INT, BOOL -> room + "->cdata";
                case STRING, PRODUCT, SUM -> room;
            };
        }

        static String cType(Word word) {
            return switch (word) {
                case STATUS -> "souther_status";
                case INT, COUNT, MARK -> "int64_t";
                case BOOL -> "uint8_t";
                case CASE -> "uint32_t";
                case OUTCOME -> "int32_t";
                case BYTES -> "uint8_t *";
                case VALUE -> "souther_value";
                case STRING -> "souther_string";
                case DECODED -> "souther_decoded";
                case ISSUE -> "souther_issue";
            };
        }
    }

    /** An optional: whether it holds a value, then the value, or nothing where it holds none. */
    record Present(Whole of) implements Crossing {

        @Override
        public String phpType() {
            return "?" + of.phpType();
        }

        @Override
        public List<Word> words() {
            return List.of(Word.BOOL, of.word());
        }

        @Override
        public List<String> given(String value, String session) {
            return List.of("(" + value + " !== null ? 1 : 0)",
                    "(" + value + " !== null ? " + of.given(value, session).getFirst() + " : "
                            + of.absent() + ")");
        }

        @Override
        public String holds(String value) {
            return "(" + value + " === null || " + of.holds(value) + ")";
        }

        @Override
        public String of(List<String> words, String session) {
            return "(" + words.getFirst() + " !== 0 ? " + of.of(List.of(words.get(1)), session)
                    + " : null)";
        }
    }
}
