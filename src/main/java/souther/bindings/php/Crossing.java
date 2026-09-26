package souther.bindings.php;

import org.jspecify.annotations.Nullable;
import souther.bindings.CrossingShape;
import souther.bindings.Manifest;
import souther.bindings.Manifest.ListCrossing;
import souther.bindings.Manifest.Word;

import java.util.List;
import java.util.stream.Collectors;

/**
 * How a value of one model type crosses between PHP and the library: the shape the library's ABI
 * gives it ({@link CrossingShape}), which says the words it is handed over as, and on it what PHP
 * calls its type and, for each way it crosses, what PHP does with those words.
 *
 * <p>The two ways are apart ({@link Given}, {@link Received}), as {@link CrossingShape} asks them
 * apart, because a type may cross one way and not the other. PHP handing over a value of a union
 * no declaration names knows which class it holds and hands over the value; PHP handed one has to
 * be told which case it is before it can make an object of it, and only a behavior's answer says.
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

    /**
     * What PHP's FFI makes ({@code FFI::new}) to hold one word the library writes or reads: room a
     * function writes through, or a column of a list. The word's own C type, as the declarations
     * the build wrote spell it, which is what room for it points at, and not how a parameter taking
     * that room is declared. Only a word PHP holds room for has one here: another is a word no
     * function writes into PHP's room, and room made of it would be a mistake found at a call.
     */
    static String storage(Word word) {
        return switch (word) {
            case INT -> "int64_t";
            case BOOL -> "uint8_t";
            case VALUE -> "souther_value";
            case STRING -> "souther_string";
            case DECIMAL -> "souther_decimal";
            case LIST -> "souther_list";
            case DECODED -> "souther_decoded";
            case STATUS, CASE, OUTCOME, COUNT, MARK, BYTES, ISSUE, REQUIREMENTS, CAPABILITY,
                 USERDATA -> throw new IllegalArgumentException("PHP holds no room for a " + word);
        };
    }

    /** A value PHP hands the library. */
    sealed interface Given extends Crossing {

        @Override
        CrossingShape.Plain shape();

        /** The PHP expressions handing {@code value} over, one for each of {@link #words()}. */
        List<String> given(String value, String session);

        /** A PHP expression true where {@code value} is a value of this. */
        String holds(String value);
    }

    /** A value the library hands PHP. */
    sealed interface Received extends Crossing {

        @Override
        CrossingShape shape();

        /**
         * The PHP expression making a value of this out of what the library answered, one
         * expression for each of {@link #words()}: a word returned or handed to an implementation
         * as itself, or read out of room with {@link #fromRooms}.
         */
        String of(List<String> words, String session);

        /** The words held in room {@code rooms} names, one room for each of {@link #words()}. */
        List<String> fromRooms(List<String> rooms);
    }

    /**
     * A value PHP holds both ways, and the same way each: never a union no declaration names, which
     * PHP holds only where it hands one over whole ({@link OneOf}) or is told its case
     * ({@link Told}), so a {@link Whole} is never one.
     */
    sealed interface Both extends Given, Received {

        @Override
        CrossingShape.Plain shape();
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

        /** What PHP's FFI makes to hold this word ({@link Crossing#storage}). */
        default String storage() {
            return Crossing.storage(word());
        }

        @Override
        default List<String> fromRooms(List<String> rooms) {
            return List.of(fromRoom(rooms.getFirst()));
        }
    }

    /**
     * One word of the library's that is the value itself: a primitive, or a value of a declared
     * type, as {@code kind} says and as the shape is.
     */
    record Whole(CrossingShape.Whole shape, String phpType, Kind kind, @Nullable String declared)
            implements Single {

        enum Kind { INT, BOOL, STRING, DECIMAL, PRODUCT, SUM }

        public Whole {
            boolean is = switch (kind) {
                case INT -> shape.word() == Word.INT;
                case BOOL -> shape.word() == Word.BOOL;
                case STRING -> shape.word() == Word.STRING;
                case DECIMAL -> shape.word() == Word.DECIMAL;
                case PRODUCT, SUM -> shape.type() instanceof Manifest.Type.Declared;
            };
            if (!is) {
                throw new IllegalArgumentException("a " + kind + " is not held as " + shape);
            }
        }

        /** A value of a primitive, as PHP's own type for it, or null where PHP has none. */
        static @Nullable Whole primitive(CrossingShape.Whole shape) {
            return switch (shape.word()) {
                case INT -> new Whole(shape, "int", Kind.INT, null);
                case BOOL -> new Whole(shape, "bool", Kind.BOOL, null);
                case STRING -> new Whole(shape, "string", Kind.STRING, null);
                // Its integer and its scale, which is all a `Decimal` is and which no type of PHP's
                // own keeps: a scale below nought has no place in one.
                case DECIMAL -> new Whole(shape, "\\Souther\\Runtime\\Decimal", Kind.DECIMAL, null);
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
                case DECIMAL -> session + "->decimal(" + value + ")";
                case PRODUCT, SUM -> value + "->nativeHandle()->borrow(" + session + ")";
            });
        }

        @Override
        public String absent() {
            return switch (kind) {
                case INT, BOOL -> "0";
                case STRING, DECIMAL, PRODUCT, SUM -> "null";
            };
        }

        @Override
        public String holds(String value) {
            return switch (kind) {
                case INT -> "\\is_int(" + value + ")";
                case BOOL -> "\\is_bool(" + value + ")";
                case STRING -> "\\is_string(" + value + ")";
                case DECIMAL, PRODUCT, SUM -> value + " instanceof " + phpType;
            };
        }

        @Override
        public String of(List<String> words, String session) {
            String word = words.getFirst();
            return switch (kind) {
                case INT -> word;
                case BOOL -> "(" + word + " !== 0)";
                case STRING -> session + "->text(" + word + ")";
                case DECIMAL -> session + "->amount(" + word + ")";
                case PRODUCT -> "new " + declared + "(" + session + "->held(" + word + "))";
                case SUM -> declared + "::wrap(" + session + ", " + word + ")";
            };
        }

        @Override
        public String fromRoom(String room) {
            return switch (kind) {
                case INT, BOOL -> room + "->cdata";
                case STRING, DECIMAL, PRODUCT, SUM -> room;
            };
        }
    }

    /** An optional: whether it holds a value, then the value, or nothing where it holds none. */
    record Present(Single of) implements Both {

        @Override
        public CrossingShape.Present shape() {
            return new CrossingShape.Present(of.shape());
        }

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
    record Listed(Both element, ListCrossing crossing) implements Single {

        /** Refuses a list through another element's functions, which the shape it makes does. */
        public Listed {
            new CrossingShape.Listed(element.shape(), crossing);
        }

        @Override
        public CrossingShape.Listed shape() {
            return new CrossingShape.Listed(element.shape(), crossing);
        }

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
            return List.of(session + "->list('" + crossing.construct().name() + "', "
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
            return session + "->elements('" + crossing.length().name() + "', '"
                    + crossing.at().name() + "', " + columns() + ", "
                    + words.getFirst() + ", static fn ("
                    + rooms.stream().map(it -> "\\FFI\\CData " + it).collect(Collectors.joining(", "))
                    + "): " + element.phpType() + " => "
                    + element.of(element.fromRooms(rooms), session) + ")";
        }

        /** What C calls each word an element crosses as, as a PHP array. */
        private String columns() {
            return element.words().stream().map(it -> "'" + Crossing.storage(it) + "'")
                    .collect(Collectors.joining(", ", "[", "]"));
        }
    }

    /**
     * One member of a union no declaration names, as PHP holds a value of it: a declared type as
     * the class generated for it, which is the union's value as it is, or a primitive as PHP's own
     * type for it, carried into the union and read back out of it through the functions the
     * manifest names for the case ({@code carried}).
     */
    record Member(Whole whole, Manifest.@Nullable CaseCrossing carried) {

        public Member {
            boolean primitive = switch (whole.kind()) {
                case INT, BOOL, STRING, DECIMAL -> true;
                case PRODUCT, SUM -> false;
            };
            if (primitive != (carried != null)) {
                throw new IllegalArgumentException(whole.phpType() + " is " + (primitive
                        ? "a primitive, carried into a union" : "a declared type, a union's value as"
                        + " it is") + ", and " + (carried == null ? "nothing" : carried.make().name())
                        + " is said to make it");
            }
        }

        /** The PHP expression handing {@code value}, a value of this member, over as the union. */
        String given(String value, String session) {
            String word = whole.given(value, session).getFirst();
            return carried == null ? word
                    : session + "->ffi()->" + carried.make().name() + "(" + word + ")";
        }

        /** The PHP expression making a value of this member out of the union's value {@code word}. */
        String of(String word, String session) {
            return whole.of(List.of(carried == null ? word
                    : session + "->ffi()->" + java.util.Objects.requireNonNull(carried.read()).name()
                    + "(" + word + ")"), session);
        }
    }

    /**
     * A value of a union no declaration names, handed over by PHP: a value of one of its members,
     * which says which one it is by what PHP holds it as. A declared member is handed over as it
     * is, and a primitive carried into the union.
     */
    record OneOf(CrossingShape.Whole shape, List<Member> members) implements Given {

        public OneOf {
            members = List.copyOf(members);
            if (!(shape.type() instanceof Manifest.Type.Union union)
                    || union.cases().size() != members.size()) {
                throw new IllegalArgumentException(members + " are not the members of " + shape);
            }
        }

        @Override
        public String phpType() {
            return members.stream().map(it -> it.whole().phpType())
                    .collect(Collectors.joining("|"));
        }

        /**
         * Each primitive member tested for first, by PHP's own type, and a declared one handed over
         * as the object it is: whichever class it is of, its handle is the union's value. PHP's type
         * of the parameter has already refused anything that is none of them.
         */
        @Override
        public List<String> given(String value, String session) {
            String declared = members.stream().filter(it -> it.carried() == null).findFirst()
                    .map(it -> it.given(value, session))
                    .orElse("throw new \\LogicException('no member of " + phpType() + " holds it')");
            String handed = declared;
            for (Member member : members.reversed()) {
                if (member.carried() != null) {
                    handed = "(" + member.whole().holds(value) + " ? " + member.given(value, session)
                            + " : " + handed + ")";
                }
            }
            return List.of(handed);
        }

        @Override
        public String holds(String value) {
            return members.stream().map(it -> it.whole().holds(value))
                    .collect(Collectors.joining(" || ", "(", ")"));
        }
    }

    /**
     * A value of a union no declaration names, handed to PHP as a behavior's answer: made as the
     * member the case the shape's {@code which} says it is, each of {@code cases} at the place the
     * library counts it.
     *
     * @param phpType the union of what PHP calls each of its members
     * @param cases   how a value of each case is made, in the order {@code which} counts them
     * @param what    what the answer is of, for the exception a case past them throws
     */
    record Told(CrossingShape.Told shape, String phpType, List<Member> cases, String what)
            implements Received {

        public Told {
            cases = List.copyOf(cases);
            if (cases.size() != shape.cases().size()) {
                throw new IllegalArgumentException(cases + " are not the cases of " + shape);
            }
        }

        @Override
        public String of(List<String> words, String session) {
            String word = words.getFirst();
            StringBuilder match = new StringBuilder("match (" + session + "->ffi()->"
                    + shape.which().name() + "(" + word + ")) {\n");
            for (int at = 0; at < cases.size(); at++) {
                match.append("            ").append(at).append(" => ")
                        .append(cases.get(at).of(word, session)).append(",\n");
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
