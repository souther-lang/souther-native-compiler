package souther.bindings.php;

import org.jspecify.annotations.Nullable;
import souther.bindings.CrossingShape;
import souther.bindings.Manifest.Word;

import java.util.List;
import java.util.stream.Collectors;

/**
 * How a value of one model type crosses between PHP and the library: the shape the library's ABI
 * gives it ({@link CrossingShape}), which says the words it is handed over as, and on it what PHP
 * calls its type and, for each way it crosses, what PHP does with those words.
 *
 * <p>The two ways are apart ({@link Given}, {@link Received}) as the shapes are, because a type may
 * cross one way and not the other. PHP handing over a value of a union no declaration names knows
 * which class it holds and hands over the value; PHP handed one has to be told which case it is
 * before it can make an object of it, and only a behavior's answer says.
 *
 * <p>A list is a PHP list of what its element crosses as: PHP hands one over as an array the
 * library builds a list of, and is handed one as an array read out of it, through the functions the
 * library defines for a list whose element crosses that way.
 */
sealed interface Crossing {

    /** How the value crosses, as the library's ABI has it. */
    CrossingShape shape();

    /** What PHP calls a value of this, as a parameter or an answer is typed. */
    String phpType();

    /**
     * What a docblock calls a value of this, where it says more than {@link #phpType()}: an array
     * is a {@code list<T>} to PHPStan, which checks the element, and only an {@code array} to PHP.
     */
    default String phpDocType() {
        return phpType();
    }

    /** The words it is handed over as, in order. */
    default List<Word> words() {
        return shape().words();
    }

    /** A value PHP hands the library. */
    sealed interface Given extends Crossing {

        @Override
        CrossingShape.Both shape();

        /** The PHP expressions handing {@code value} over, one for each of {@link #words()}. */
        List<String> given(String value, String session);

        /** A PHP expression true where {@code value} is a value of this. */
        String holds(String value);
    }

    /** A value the library hands PHP. */
    sealed interface Received extends Crossing {

        @Override
        CrossingShape.Received shape();

        /**
         * The PHP expression making a value of this out of what the library answered, one
         * expression for each of {@link #words()}: a word returned or handed to an implementation
         * as itself, or read out of room with {@link #fromRooms}.
         */
        String of(List<String> words, String session);

        /** The words held in room {@code rooms} names, one room for each of {@link #words()}. */
        List<String> fromRooms(List<String> rooms);
    }

    /** A value that crosses both ways, and the same way each. */
    sealed interface Both extends Given, Received {

        @Override
        CrossingShape.Both shape();
    }

    /**
     * A value that crosses both ways as one word: what an optional is a presence beside, and what
     * a list holds either of.
     */
    sealed interface Single extends Both {

        @Override
        CrossingShape.Single shape();

        default Word word() {
            return shape().word();
        }

        /** What handing over nothing is, where an optional holds no value. */
        String absent();

        /** The word held in room {@code room} names, as {@link #of} takes it. */
        String fromRoom(String room);

        /** What C calls room for this word. */
        default String cType() {
            return word().cType();
        }

        @Override
        default List<String> fromRooms(List<String> rooms) {
            return List.of(fromRoom(rooms.getFirst()));
        }
    }

    /** One word of the library's that is the value itself. */
    record Whole(CrossingShape.Whole shape, String phpType, Kind kind, @Nullable String declared)
            implements Single {

        enum Kind { INT, BOOL, STRING, PRODUCT, SUM }

        /** A value of a primitive, as PHP's own type for it, or null where PHP has none. */
        static @Nullable Whole primitive(CrossingShape.Whole shape) {
            return switch (shape.word()) {
                case INT -> new Whole(shape, "int", Kind.INT, null);
                case BOOL -> new Whole(shape, "bool", Kind.BOOL, null);
                case STRING -> new Whole(shape, "string", Kind.STRING, null);
                default -> null;
            };
        }

        /** A value of a declared type that is not a sum, as the class generated for it. */
        static Whole product(CrossingShape.Whole shape, String fqcn) {
            return new Whole(shape, fqcn, Kind.PRODUCT, fqcn);
        }

        /**
         * A value of a sum, as the interface generated for it, and made through what decides which
         * of its classes a value is.
         */
        static Whole sum(CrossingShape.Whole shape, String iface, String codec) {
            return new Whole(shape, iface, Kind.SUM, codec);
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

        @Override
        public String absent() {
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
                case PRODUCT -> "new " + declared + "(" + session + "->held(" + word + "))";
                case SUM -> declared + "::wrap(" + session + ", " + word + ")";
            };
        }

        @Override
        public String fromRoom(String room) {
            return switch (kind) {
                case INT, BOOL -> room + "->cdata";
                case STRING, PRODUCT, SUM -> room;
            };
        }
    }

    /** An optional: whether it holds a value, then the value, or nothing where it holds none. */
    record Present(CrossingShape.Present shape, Single of) implements Both {

        @Override
        public String phpType() {
            return "?" + of.phpType();
        }

        @Override
        public String phpDocType() {
            return of.phpDocType().equals(of.phpType()) ? phpType() : of.phpDocType() + "|null";
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

        @Override
        public List<String> fromRooms(List<String> rooms) {
            return List.of(rooms.getFirst() + "->cdata", of.fromRoom(rooms.get(1)));
        }
    }

    /**
     * A list, as a PHP list of what its element crosses as, through the functions the shape names.
     *
     * <p>An element is handed over as PHP hands over a value of its type anywhere else, through the
     * session the list is built in, so a value of an outer run may be one and a value of a run that
     * ended may not. An element read out is held for the session the list was read in, as a field's
     * value is.
     */
    record Listed(CrossingShape.Listed shape, Both element) implements Single {

        @Override
        public String phpType() {
            return "array";
        }

        @Override
        public String phpDocType() {
            return "list<" + element.phpDocType() + ">";
        }

        @Override
        public String absent() {
            return "null";
        }

        @Override
        public String fromRoom(String room) {
            return room;
        }

        @Override
        public List<String> given(String value, String session) {
            // Typed as the element, so an element of another type is refused by PHP before any of
            // it reaches the library.
            return List.of(session + "->list('" + shape.crossing().construct().name() + "', "
                    + columns() + ", " + value
                    + ", static fn (" + element.phpType() + " $it): array => ["
                    + String.join(", ", element.given("$it", session)) + "])");
        }

        @Override
        public String holds(String value) {
            return "\\is_array(" + value + ")";
        }

        @Override
        public String of(List<String> words, String session) {
            List<String> rooms = new java.util.ArrayList<>();
            for (int at = 0; at < element.words().size(); at++) {
                rooms.add("$r" + at);
            }
            return session + "->elements('" + shape.crossing().length().name() + "', '"
                    + shape.crossing().at().name() + "', " + columns() + ", "
                    + words.getFirst() + ", static fn ("
                    + rooms.stream().map(it -> "\\FFI\\CData " + it).collect(Collectors.joining(", "))
                    + "): " + element.phpType() + " => "
                    + element.of(element.fromRooms(rooms), session) + ")";
        }

        /** What C calls each word an element crosses as, as a PHP array. */
        private String columns() {
            return element.words().stream().map(it -> "'" + it.cType() + "'")
                    .collect(Collectors.joining(", ", "[", "]"));
        }
    }

    /**
     * A value of a union no declaration names, handed over by PHP: an object of the class of one
     * of its members, which already is the case it is, so the value is handed over as it is.
     */
    record OneOf(CrossingShape.Whole shape, List<Whole> members) implements Given {

        @Override
        public String phpType() {
            return members.stream().map(Whole::phpType).collect(Collectors.joining("|"));
        }

        @Override
        public List<String> given(String value, String session) {
            return List.of(value + "->nativeHandle()->borrow(" + session + ")");
        }

        @Override
        public String holds(String value) {
            return members.stream().map(it -> it.holds(value))
                    .collect(Collectors.joining(" || ", "(", ")"));
        }
    }

    /**
     * A value of a union no declaration names, handed to PHP as a behavior's answer: made as the
     * class of the case the shape's {@code which} says it is, each of {@code cases} at the place the
     * library counts it.
     *
     * @param phpType the union of what PHP calls each of its members
     * @param cases   how a value of each case is made, in the order {@code which} counts them
     * @param what    what the answer is of, for the exception a case past them throws
     */
    record Told(CrossingShape.Told shape, String phpType, List<Whole> cases, String what)
            implements Received {

        @Override
        public String of(List<String> words, String session) {
            String word = words.getFirst();
            StringBuilder match = new StringBuilder("match (" + session + "->ffi()->"
                    + shape.which().name() + "(" + word + ")) {\n");
            for (int at = 0; at < cases.size(); at++) {
                match.append("            ").append(at).append(" => ")
                        .append(cases.get(at).of(List.of(word), session)).append(",\n");
            }
            return match.append("            default => throw new \\LogicException('the library")
                    .append(" answered a case ").append(what).append(" does not answer'),\n")
                    .append("        }").toString();
        }

        @Override
        public List<String> fromRooms(List<String> rooms) {
            return List.of(rooms.getFirst());
        }
    }
}
