package souther.bindings.go;

import org.jspecify.annotations.Nullable;
import souther.bindings.Manifest;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.stream.Collectors;

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

        enum Kind { INT, BOOL, STRING, DECIMAL, DATE, TIME, DATETIME, INSTANT, HANDLE }

        /**
         * A value of the primitive {@code name} crossing as {@code word}, or null where this binding
         * has no way to hold that pair. Both are asked, the name and the word.
         */
        static @Nullable Whole primitive(String name, Word word) {
            Kind kind = switch (name) {
                case "Int" -> word == Word.INT ? Kind.INT : null;
                case "Bool" -> word == Word.BOOL ? Kind.BOOL : null;
                case "String" -> word == Word.STRING ? Kind.STRING : null;
                case "Decimal" -> word == Word.DECIMAL ? Kind.DECIMAL : null;
                case "Date" -> word == Word.DATE ? Kind.DATE : null;
                case "Time" -> word == Word.TIME ? Kind.TIME : null;
                case "DateTime" -> word == Word.DATETIME ? Kind.DATETIME : null;
                case "Instant" -> word == Word.INSTANT ? Kind.INSTANT : null;
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
                case DECIMAL -> imports.raoh() + ".Decimal";
                case DATE -> imports.souther() + ".Date";
                case TIME -> imports.souther() + ".Time";
                case DATETIME -> imports.souther() + ".DateTime";
                case INSTANT -> imports.souther() + ".Instant";
                case HANDLE -> imports.module(declared.importPath()) + declared.name();
            };
        }

        @Override
        public String zero(Body.Imports imports) {
            return switch (kind) {
                case INT -> "0";
                case BOOL -> "false";
                case STRING -> "\"\"";
                case DECIMAL, DATE, TIME, DATETIME, INSTANT, HANDLE -> type(imports) + "{}";
            };
        }

        /** The function of the runtime handing a value of this over as the word the library reads. */
        private String hand() {
            return switch (kind) {
                case DATE -> "DateWord";
                case TIME -> "TimeWord";
                case DATETIME -> "DateTimeWord";
                case INSTANT -> "InstantWord";
                default -> throw new IllegalStateException(kind + " is handed over by no function of the runtime");
            };
        }

        /** The function of the runtime reading what the library answered as a value of this. */
        private String read() {
            return switch (kind) {
                case DECIMAL -> "Amount";
                case DATE -> "DateOf";
                case TIME -> "TimeOf";
                case DATETIME -> "DateTimeOf";
                case INSTANT -> "InstantOfWord";
                default -> throw new IllegalStateException(kind + " is read by no function of the runtime");
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
                case DECIMAL -> {
                    body.making();
                    body.line(word + " = " + body.imports.souther() + ".Decimal(" + body.run + ", "
                            + value + ")");
                }
                case DATE, TIME, DATETIME, INSTANT -> {
                    String at = body.temp("at");
                    body.line(at + ", err := " + body.imports.souther() + "." + hand() + "("
                            + body.run + ", " + value + ")").checked().line(word + " = " + at);
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
                case DECIMAL, DATE, TIME, DATETIME, INSTANT -> body.imports.souther() + "." + read()
                        + "(" + body.run + ", " + word + ")";
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
                case DECIMAL -> "Decimal";
                case DATE -> "Date";
                case TIME -> "Time";
                case DATETIME -> "DateTime";
                case INSTANT -> "Instant";
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

    /** A tuple: {@code souther.Tuple2} and its like, of what its members are. */
    record Tuple(List<Crossing> members) implements Crossing {

        /** The most members a tuple has that this binding holds: {@code souther.Tuple8}. */
        static final int MOST = 8;

        public Tuple {
            members = List.copyOf(members);
            if (members.size() < 2 || members.size() > MOST) {
                throw new IllegalArgumentException("a tuple of " + members.size() + " members has"
                        + " no type");
            }
        }

        @Override
        public Shape shape() {
            return new Shape.Product(members.stream().map(Crossing::shape).toList());
        }

        @Override
        public String type(Body.Imports imports) {
            return imports.souther() + ".Tuple" + members.size() + "["
                    + members.stream().map(it -> it.type(imports)).collect(Collectors.joining(", "))
                    + "]";
        }

        @Override
        public String zero(Body.Imports imports) {
            return type(imports) + "{}";
        }

        @Override
        public void give(Body body, String value, List<String> into) {
            int at = 0;
            for (int member = 0; member < members.size(); member++) {
                int wide = members.get(member).words().size();
                members.get(member).give(body, value + ".V" + member, into.subList(at, at + wide));
                at += wide;
            }
        }

        @Override
        public String of(Body body, List<String> words) {
            List<String> made = new ArrayList<>();
            int at = 0;
            for (int member = 0; member < members.size(); member++) {
                int wide = members.get(member).words().size();
                made.add("V" + member + ": " + members.get(member).of(body, words.subList(at, at + wide)));
                at += wide;
            }
            return type(body.imports) + "{" + String.join(", ", made) + "}";
        }

        @Override
        public String label() {
            return "Tuple" + members.stream().map(Crossing::label).collect(Collectors.joining("And"));
        }
    }

    /**
     * A list, and a set and a map, which cross as the list of what they hold: a slice of what its
     * element is, built and read through the functions the manifest names for a list of its
     * element's shape.
     *
     * @param construct what builds one, or null where nothing does
     * @param read      what reads one, or null where nothing does
     */
    record Listed(Crossing element, @Nullable Function construct, Manifest.@Nullable ListRead read)
            implements Crossing {

        @Override
        public Shape shape() {
            return new Shape.ListOf(element.shape());
        }

        @Override
        public String type(Body.Imports imports) {
            return "[]" + element.type(imports);
        }

        @Override
        public String zero(Body.Imports imports) {
            return "nil";
        }

        /**
         * The list, built of a column of each word its element crosses as, every element's at its
         * index. The columns are read for the length of the call and not kept.
         */
        @Override
        public void give(Body body, String value, List<String> into) {
            Objects.requireNonNull(construct, "a list handed over is built by something");
            List<Word> words = element.words();
            String count = body.temp("count");
            body.line(count + " := len(" + value + ")");
            List<String> columns = new ArrayList<>();
            for (Word word : words) {
                String column = body.temp("column");
                body.line(column + " := make([]" + local(word, body.imports) + ", 0, " + count + ")");
                columns.add(column);
            }
            String each = body.temp("element");
            body.open("for _, " + each + " := range " + value);
            List<String> inner = declare(body, "e", words);
            element.give(body, each, inner);
            for (int at = 0; at < words.size(); at++) {
                body.line(columns.get(at) + " = append(" + columns.get(at) + ", " + inner.get(at) + ")");
            }
            body.close();
            List<String> handed = new ArrayList<>(List.of("C.int64_t(" + count + ")"));
            columns.forEach(column -> handed.add(body.imports.souther() + ".Addr(" + column + ")"));
            body.line(into.getFirst() + " = " + body.call(construct, handed));
        }

        /** Each element read into room of its own, in order. */
        @Override
        public String of(Body body, List<String> words) {
            Objects.requireNonNull(read, "a list handed over is read by something");
            String count = body.temp("count");
            body.line(count + " := int64(" + body.call(read.length(), List.of(words.getFirst())) + ")");
            String elements = body.temp("elements");
            body.line(elements + " := make([]" + element.type(body.imports) + ", 0, " + count + ")");
            String index = body.temp("index");
            body.open("for " + index + " := int64(0); " + index + " < " + count + "; " + index + "++");
            List<String> rooms = declare(body, "e", element.words());
            List<String> handed = new ArrayList<>(List.of(words.getFirst(), "C." + "int64_t(" + index + ")"));
            rooms.forEach(room -> handed.add("&" + room));
            String inside = body.temp("inside");
            body.line(inside + " := " + body.call(read.at(), handed));
            body.open("if " + inside + " == 0")
                    .line("panic(\"the library answers every element of a list below its length\")")
                    .close();
            body.line(elements + " = append(" + elements + ", " + element.of(body, rooms) + ")");
            body.close();
            return elements;
        }

        @Override
        public String label() {
            return "List" + element.label();
        }
    }

    /**
     * A value of a union no declaration names, as the interface generated for it: a type for each
     * of its members, a declared one holding its handle and a primitive Go's own value. Handed over
     * as one of the member types; handed to Go only where the library says which case it is, which
     * a behavior's answer does ({@code told}).
     *
     * @param importPath the package the interface is written in
     * @param name    what the interface is called there
     * @param members each member, in the order the union names them
     * @param told    how each case the library counts is made, in its order, and what counts it;
     *                null where the library says nothing of which case a value is
     */
    record OneOf(String importPath, String name, List<Member> members, @Nullable Told told)
            implements Crossing {

        /**
         * One member: the type it is, how Go holds its value, and where it is a primitive, what
         * carries a value of it into the union and reads it back out.
         */
        record Member(String variant, Whole whole, @Nullable Function make, @Nullable Function read) {
        }

        /** What the library says a value is: {@code which} counts the cases the union descends to. */
        record Told(Function which, List<Arm> arms) {
        }

        /** One case the library counts: the member it is made as. */
        record Arm(Member member) {
        }

        public OneOf {
            members = List.copyOf(members);
        }

        @Override
        public Shape shape() {
            return new Shape.Leaf(Word.VALUE);
        }

        @Override
        public String type(Body.Imports imports) {
            return imports.module(importPath) + name;
        }

        @Override
        public String zero(Body.Imports imports) {
            return "nil";
        }

        @Override
        public void give(Body body, String value, List<String> into) {
            String it = body.temp("member");
            body.open("switch " + it + " := " + value + ".(type)");
            for (Member member : members) {
                body.label("case " + type(body.imports) + member.variant() + ":");
                if (member.make() == null) {
                    member.whole().give(body, it + ".Value", into);
                } else {
                    List<String> word = declare(body, "w", member.whole().words());
                    member.whole().give(body, it + ".Value", word);
                    body.line(into.getFirst() + " = " + body.call(member.make(), word));
                }
            }
            body.label("default:").line(body.imports.souther() + ".NoValue()");
            body.close();
        }

        @Override
        public String of(Body body, List<String> words) {
            Objects.requireNonNull(told, "a union is handed to Go only where it is told its case");
            String union = body.temp("union");
            body.line("var " + union + " " + type(body.imports));
            body.open("switch " + body.call(told.which(), List.of(words.getFirst())));
            for (int place = 0; place < told.arms().size(); place++) {
                Member member = told.arms().get(place).member();
                body.label("case " + place + ":");
                String held = words.getFirst();
                if (member.read() != null) {
                    held = body.temp("held");
                    body.line(held + " := " + body.call(member.read(), List.of(words.getFirst())));
                }
                body.line(union + " = " + type(body.imports) + member.variant() + "{Value: "
                        + member.whole().of(body, List.of(held)) + "}");
            }
            body.label("default:")
                    .line("panic(\"the library answered a case the union does not have\")");
            body.close();
            return union;
        }

        @Override
        public String label() {
            return name;
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
