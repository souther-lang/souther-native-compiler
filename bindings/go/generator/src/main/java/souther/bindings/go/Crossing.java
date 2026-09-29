package souther.bindings.go;

import org.jspecify.annotations.Nullable;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.List;

/**
 * How a value of one model type crosses between Go and the library: the shape the manifest says it
 * crosses in ({@link Shape}), which says the words it is handed over as, and on it the Go type a
 * value of it is and the Go that turns one into the other.
 *
 * <p>The shape is the library's decision, read off the manifest; what is decided here is only how
 * Go holds what crosses in it. A word is held in a local of its own, of the type {@link #local}
 * says, which {@link #give} sets from a Go value and {@link #of} reads a Go value out of.
 *
 * <p>A value of a declared type is held as the handle generated for it, and is handed over only
 * through {@code Ref.In}, which refuses one another runtime made. Reading one takes the run it was
 * made in, so what a reader answers belongs to the run of the value it read.
 */
sealed interface Crossing {

    /** How the value crosses, as the manifest says. */
    Shape shape();

    /** The Go type a value of this is, as the file being written names it. */
    String type(Body.Imports imports);

    /** The zero value of {@link #type}, which a function answers alongside a failure. */
    String zero(Body.Imports imports);

    /** The words it is handed over as, in order. */
    default List<Word> words() {
        return shape().words();
    }

    /**
     * Sets each of {@code into}, a declared local for each word, from {@code value}, a Go
     * expression of {@link #type} that may be read as often as there are words. Writes what it
     * needs to before, and ends the function with {@code err} where handing it over fails.
     */
    void give(Body body, String value, List<String> into);

    /** The Go expression of a value made of {@code words}, locals the library wrote. */
    String of(Body body, List<String> words);

    /** What a type named after this one calls it. */
    String label();

    /** The type of the local a word is held in. */
    static String local(Word word, Body.Imports imports) {
        return switch (word) {
            case STATUS, CASE -> "C.uint32_t";
            case INT, COUNT, MARK -> "C.int64_t";
            case BOOL -> "C.uint8_t";
            case OUTCOME -> "C.int32_t";
            case BYTES, VALUE, STRING, DECIMAL, DATE, TIME, DATETIME, INSTANT, DECODED, ISSUE,
                 LIST, FUNCTION, REQUIREMENTS, USERDATA -> imports.unsafe() + ".Pointer";
            case CAPABILITY -> throw new IllegalArgumentException("a capability is no local of a value");
        };
    }

    /**
     * One word of the library's that is the value itself: a primitive, or a value of a declared
     * type as the handle generated for it.
     */
    record Whole(Shape.Leaf shape, Kind kind, @Nullable Declared declared) implements Crossing {

        enum Kind { INT, BOOL, STRING, HANDLE }

        /**
         * A value of the primitive {@code name} crossing as {@code word}, or null where this binding
         * has no way to hold that pair. Both are asked, the name and the word.
         */
        static @Nullable Whole primitive(String name, Word word) {
            Kind kind = switch (name) {
                case "Int" -> word == Word.INT ? Kind.INT : null;
                case "Bool" -> word == Word.BOOL ? Kind.BOOL : null;
                case "String" -> word == Word.STRING ? Kind.STRING : null;
                default -> null;
            };
            return kind == null ? null : new Whole(new Shape.Leaf(word), kind, null);
        }

        /** A value of a declared type, as the handle generated for it. */
        static Whole handle(Declared declared) {
            return new Whole(new Shape.Leaf(Word.VALUE), Kind.HANDLE, declared);
        }

        @Override
        public String type(Body.Imports imports) {
            return switch (kind) {
                case INT -> "int64";
                case BOOL -> "bool";
                case STRING -> "string";
                case HANDLE -> imports.module(declared.importPath()) + declared.name();
            };
        }

        @Override
        public String zero(Body.Imports imports) {
            return switch (kind) {
                case INT -> "0";
                case BOOL -> "false";
                case STRING -> "\"\"";
                case HANDLE -> type(imports) + "{}";
            };
        }

        @Override
        public void give(Body body, String value, List<String> into) {
            String word = into.getFirst();
            switch (kind) {
                case INT -> body.line(word + " = C.int64_t(" + value + ")");
                case BOOL -> body.open("if " + value).line(word + " = 1").close();
                case STRING -> {
                    String text = body.temp("text");
                    body.line(text + ", err := " + body.imports.souther() + ".String(" + body.run
                            + ", " + value + ")").checked().line(word + " = " + text);
                }
                case HANDLE -> {
                    String at = body.temp("at");
                    body.line(at + ", err := " + value + ".Ref__.In(" + body.run + ")").checked()
                            .line(word + " = " + at);
                }
            }
        }

        @Override
        public String of(Body body, List<String> words) {
            String word = words.getFirst();
            return switch (kind) {
                case INT -> "int64(" + word + ")";
                case BOOL -> word + " != 0";
                case STRING -> body.imports.souther() + ".Text(" + body.run + ", " + word + ")";
                case HANDLE -> type(body.imports) + "{Ref__: " + body.imports.souther() + ".NewRef("
                        + body.run + ", " + word + ")}";
            };
        }

        @Override
        public String label() {
            return switch (kind) {
                case INT -> "Int";
                case BOOL -> "Bool";
                case STRING -> "String";
                case HANDLE -> declared.name();
            };
        }
    }

    /** An optional: {@code souther.Option} of what it holds. */
    record Optional(Crossing of) implements Crossing {

        @Override
        public Shape shape() {
            return new Shape.Option(of.shape());
        }

        @Override
        public String type(Body.Imports imports) {
            return imports.souther() + ".Option[" + of.type(imports) + "]";
        }

        @Override
        public String zero(Body.Imports imports) {
            return type(imports) + "{}";
        }

        @Override
        public void give(Body body, String value, List<String> into) {
            String held = body.temp("held");
            body.open("if " + held + ", ok := " + value + ".Get(); ok").line(into.getFirst() + " = 1");
            of.give(body, held, into.subList(1, into.size()));
            body.close();
        }

        @Override
        public String of(Body body, List<String> words) {
            String option = body.temp("option");
            body.line(option + " := " + body.imports.souther() + ".None[" + of.type(body.imports)
                    + "]()");
            body.open("if " + words.getFirst() + " != 0");
            String inner = of.of(body, words.subList(1, words.size()));
            body.line(option + " = " + body.imports.souther() + ".Some(" + inner + ")").close();
            return option;
        }

        @Override
        public String label() {
            return "Option" + of.label();
        }
    }

    /** {@code words}, each declared before a call. */
    static List<String> declare(Body body, String base, List<Word> words) {
        List<String> names = new ArrayList<>();
        for (Word word : words) {
            String name = body.temp(base);
            body.line("var " + name + " " + local(word, body.imports));
            names.add(name);
        }
        return names;
    }
}
