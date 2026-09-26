package souther.bindings.php;

import org.jspecify.annotations.Nullable;
import souther.bindings.Manifest;
import souther.bindings.Manifest.FunctionCrossing;
import souther.bindings.Manifest.ListCrossing;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;
import java.util.stream.IntStream;

/**
 * How a value of one model type crosses between PHP and the library: the shape the manifest says
 * it crosses in ({@link Shape}), which says the words it is handed over as, and on it what PHP
 * calls its type and, for each way it crosses, what PHP does with those words.
 *
 * <p>The shape is the library's decision, read off the manifest; what is decided here is only how
 * PHP holds what crosses in it. A value of a declared type is an object of the class generated for
 * it, a tuple a PHP list of its members, a list a PHP list of its elements, and a function value a
 * {@code \Closure}. An optional is null where it holds nothing, and what it holds where it does,
 * except where what it holds may be null itself: there a value it holds is a
 * {@code \Souther\Runtime\Some}, so an optional of an optional holding nothing is not null.
 *
 * <p>The two ways are apart ({@link Given}, {@link Received}), and so is every kind made of others,
 * because a type may cross one way and not the other, and what it is made of crosses the way it
 * does. PHP handing over a value of a union no declaration names knows which class it holds and
 * hands over the value; PHP handed one has to be told which case it is before it can make an object
 * of it, and only a behavior's answer says. So a list of one is built by PHP and never read, and a
 * function taking one is called by PHP and never made, and a list or a function value is reached
 * through what the manifest offers for the way it crosses and nothing else. Only a leaf is held the
 * same way both ways ({@link Whole}).
 */
sealed interface Crossing {

    /** How the value crosses, as the manifest says. */
    Shape shape();

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

    /** Whether a value of this may be null to PHP: only an optional's may. */
    default boolean nullable() {
        return false;
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
            case FUNCTION -> "souther_function";
            case DECODED -> "souther_decoded";
            // This binding holds none of the four temporals (`Whole#primitive` answers none for a
            // pair carrying one), so no function writes one into PHP's room.
            case STATUS, CASE, OUTCOME, COUNT, MARK, BYTES, ISSUE, REQUIREMENTS, CAPABILITY,
                 USERDATA, DATE, TIME, DATETIME, INSTANT ->
                    throw new IllegalArgumentException("PHP holds no room for a " + word);
        };
    }

    /**
     * The word held in {@code room}, room of {@link #storage} of {@code word}, as a word is handed
     * to {@link Received#of}: a number as PHP's own, and an address as the room holding it.
     */
    static String fromRoom(Word word, String room) {
        return switch (word) {
            case INT, BOOL -> room + "->cdata";
            default -> room;
        };
    }

    /** What handing over no {@code word} is, where an optional holds nothing. */
    static String absent(Word word) {
        return switch (word) {
            case INT, BOOL -> "0";
            default -> "null";
        };
    }

    /** A value PHP hands the library. */
    sealed interface Given extends Crossing {

        /** The PHP expressions handing {@code value} over, one for each of {@link #words()}. */
        List<String> given(String value, String session);

        /** A PHP expression true where {@code value} is a value of this. */
        String holds(String value);
    }

    /** A value the library hands PHP. */
    sealed interface Received extends Crossing {

        /**
         * The PHP expression making a value of this out of what the library answered, one
         * expression for each of {@link #words()}: a word returned or handed to an implementation
         * as itself, or read out of room with {@link #fromRooms}.
         */
        String of(List<String> words, String session);

        /** The words held in room {@code rooms} names, one room for each of {@link #words()}. */
        default List<String> fromRooms(List<String> rooms) {
            List<Word> words = words();
            return IntStream.range(0, words.size())
                    .mapToObj(at -> Crossing.fromRoom(words.get(at), rooms.get(at))).toList();
        }
    }

    /**
     * One word of the library's that is the value itself: a primitive, or a value of a declared
     * type, as {@code kind} says and as the shape is. Held the same way both ways.
     */
    record Whole(Shape.Leaf shape, String phpType, Kind kind, @Nullable String declared)
            implements Given, Received {

        enum Kind { INT, BOOL, STRING, DECIMAL, PRODUCT, SUM }

        public Whole {
            Word is = switch (kind) {
                case INT -> Word.INT;
                case BOOL -> Word.BOOL;
                case STRING -> Word.STRING;
                case DECIMAL -> Word.DECIMAL;
                case PRODUCT, SUM -> Word.VALUE;
            };
            if (shape.word() != is) {
                throw new IllegalArgumentException("a " + kind + " is not held as " + shape);
            }
        }

        /**
         * A value of the primitive {@code name} crossing as {@code word}, as PHP's own type for it,
         * or null where this binding has no way to hold that pair.
         *
         * <p>The one place this binding says which primitives it holds and how: an {@code Int} as
         * an {@code int} where it crosses as an {@code INT}, a {@code Bool} as a {@code bool} where
         * it crosses as a {@code BOOL}, a {@code String} as a {@code string} where it crosses as a
         * {@code STRING}, and a {@code Decimal} as a {@code \\Souther\\Runtime\\Decimal}, its
         * integer and its scale, where it crosses as a {@code DECIMAL}, since no type of PHP's own
         * keeps a scale below nought. Both are asked, the name and the word: what a primitive crosses as is
         * the manifest's to say, and a {@code Decimal} said to cross as an {@code INT} is a pair
         * this binding does not hold, and not an {@code int}. A value of a union carrying the
         * primitive is asked the same.
         */
        static @Nullable Whole primitive(String name, Word word) {
            return switch (name) {
                case "Int" -> word == Word.INT
                        ? new Whole(new Shape.Leaf(word), "int", Kind.INT, null) : null;
                case "Bool" -> word == Word.BOOL
                        ? new Whole(new Shape.Leaf(word), "bool", Kind.BOOL, null) : null;
                case "String" -> word == Word.STRING
                        ? new Whole(new Shape.Leaf(word), "string", Kind.STRING, null) : null;
                case "Decimal" -> word == Word.DECIMAL
                        ? new Whole(new Shape.Leaf(word), "\\Souther\\Runtime\\Decimal",
                                Kind.DECIMAL, null) : null;
                default -> null;
            };
        }

        /** A value of a declared type that is not a sum, as the class generated for it. */
        static Whole product(String fqcn) {
            return new Whole(new Shape.Leaf(Word.VALUE), fqcn, Kind.PRODUCT, fqcn);
        }

        /**
         * A value of a sum, as the interface generated for it, and made through what decides which
         * of its classes a value is.
         */
        static Whole sum(String iface, String codec) {
            return new Whole(new Shape.Leaf(Word.VALUE), iface, Kind.SUM, codec);
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
    }

    /**
     * What an optional holding a value of {@code of} holds it in: {@code \\Souther\\Runtime\\Some}
     * where what it holds may be null itself, so each depth of absence is its own, and the value as
     * it is otherwise.
     */
    private static boolean wraps(Crossing of) {
        return of.nullable();
    }

    /** What PHP calls an optional of {@code of}. */
    private static String optionalType(Crossing of) {
        return "?" + (wraps(of) ? SOME : of.phpType());
    }

    /** What a docblock calls an optional of {@code of}. */
    private static String optionalDocType(Crossing of) {
        if (wraps(of)) {
            return SOME + "<" + of.phpDocType() + ">|null";
        }
        return of.phpDocType().equals(of.phpType()) ? optionalType(of)
                : of.phpDocType() + "|null";
    }

    /** What PHP holds a value an optional holds in, where it wraps it. */
    String SOME = "\\Souther\\Runtime\\Some";

    /**
     * An optional PHP hands over: null where it holds nothing, and what it holds, or a {@code Some}
     * of it where that may be null itself.
     */
    record GivenOptional(Given of) implements Given {

        @Override
        public Shape shape() {
            return new Shape.Option(of.shape());
        }

        @Override
        public boolean nullable() {
            return true;
        }

        @Override
        public String phpType() {
            return optionalType(of);
        }

        @Override
        public String phpDocType() {
            return optionalDocType(of);
        }

        @Override
        public List<String> given(String value, String session) {
            String held = wraps(of) ? value + "->value" : value;
            List<String> words = new ArrayList<>(List.of("(" + value + " !== null ? 1 : 0)"));
            List<String> inner = of.given(held, session);
            List<Word> innerWords = of.words();
            for (int at = 0; at < inner.size(); at++) {
                words.add("(" + value + " !== null ? " + inner.get(at) + " : "
                        + Crossing.absent(innerWords.get(at)) + ")");
            }
            return words;
        }

        @Override
        public String holds(String value) {
            String held = wraps(of) ? "(" + value + " instanceof " + SOME + " && "
                    + of.holds(value + "->value") + ")" : of.holds(value);
            return "(" + value + " === null || " + held + ")";
        }
    }

    /** An optional the library hands PHP, held as {@link GivenOptional} says. */
    record ReceivedOptional(Received of) implements Received {

        @Override
        public Shape shape() {
            return new Shape.Option(of.shape());
        }

        @Override
        public boolean nullable() {
            return true;
        }

        @Override
        public String phpType() {
            return optionalType(of);
        }

        @Override
        public String phpDocType() {
            return optionalDocType(of);
        }

        @Override
        public String of(List<String> words, String session) {
            String made = of.of(words.subList(1, words.size()), session);
            return "(" + words.getFirst() + " !== 0 ? "
                    + (wraps(of) ? "new " + SOME + "(" + made + ")" : made) + " : null)";
        }
    }

    /** What a docblock calls a tuple of {@code members}: an array of each at its place. */
    private static String tupleDocType(List<? extends Crossing> members) {
        return IntStream.range(0, members.size())
                .mapToObj(at -> at + ": " + members.get(at).phpDocType())
                .collect(Collectors.joining(", ", "array{", "}"));
    }

    /** A tuple PHP hands over, as a PHP list of its members. */
    record GivenTuple(List<Given> members) implements Given {

        public GivenTuple {
            members = List.copyOf(members);
        }

        @Override
        public Shape shape() {
            return new Shape.Product(members.stream().map(Crossing::shape).toList());
        }

        @Override
        public String phpType() {
            return "array";
        }

        @Override
        public String phpDocType() {
            return tupleDocType(members);
        }

        @Override
        public List<String> given(String value, String session) {
            List<String> words = new ArrayList<>();
            for (int at = 0; at < members.size(); at++) {
                words.addAll(members.get(at).given(value + "[" + at + "]", session));
            }
            return words;
        }

        @Override
        public String holds(String value) {
            List<String> held = new ArrayList<>(List.of("\\is_array(" + value + ")",
                    "\\array_is_list(" + value + ")", "\\count(" + value + ") === " + members.size()));
            for (int at = 0; at < members.size(); at++) {
                held.add(members.get(at).holds(value + "[" + at + "]"));
            }
            return "(" + String.join(" && ", held) + ")";
        }
    }

    /** A tuple the library hands PHP, as a PHP list of its members. */
    record ReceivedTuple(List<Received> members) implements Received {

        public ReceivedTuple {
            members = List.copyOf(members);
        }

        @Override
        public Shape shape() {
            return new Shape.Product(members.stream().map(Crossing::shape).toList());
        }

        @Override
        public String phpType() {
            return "array";
        }

        @Override
        public String phpDocType() {
            return tupleDocType(members);
        }

        @Override
        public String of(List<String> words, String session) {
            List<String> made = new ArrayList<>();
            int at = 0;
            for (Received member : members) {
                int wide = member.words().size();
                made.add(member.of(words.subList(at, at + wide), session));
                at += wide;
            }
            return "[" + String.join(", ", made) + "]";
        }
    }

    /** What C calls each word an element of {@code element} crosses as, as a PHP array. */
    private static String columns(Crossing element) {
        return element.words().stream().map(it -> "'" + Crossing.storage(it) + "'")
                .collect(Collectors.joining(", ", "[", "]"));
    }

    /** Refuses a list through another element's functions. */
    private static void listing(Crossing element, ListCrossing crossing) {
        if (!crossing.element().equals(element.shape())) {
            throw new IllegalArgumentException("a list of " + element.shape() + " is not reached"
                    + " through the functions of a list of " + crossing.element());
        }
    }

    /**
     * A list PHP hands over, as a PHP list of what its element crosses as, built through what the
     * manifest says builds a list of its element's shape. An element is handed over as PHP hands
     * over a value of its type anywhere else, through the session the list is built in, so a value
     * of an outer run may be one and a value of a run that ended may not.
     */
    record GivenList(Given element, ListCrossing crossing) implements Given {

        public GivenList {
            listing(element, crossing);
            if (crossing.construct() == null) {
                throw new IllegalArgumentException("a list of " + crossing.element() + " is built"
                        + " by nothing");
            }
        }

        @Override
        public Shape shape() {
            return new Shape.ListOf(element.shape());
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
        public List<String> given(String value, String session) {
            // Typed as the element, so an element of another type is refused by PHP before any of
            // it reaches the library.
            return List.of(session + "->list('"
                    + java.util.Objects.requireNonNull(crossing.construct()).name() + "', "
                    + columns(element) + ", " + value + ", static fn (" + element.phpType()
                    + " $it): array => [" + String.join(", ", element.given("$it", session)) + "])");
        }

        @Override
        public String holds(String value) {
            return "\\is_array(" + value + ")";
        }
    }

    /**
     * A list the library hands PHP, as a PHP list of what its element crosses as, read through
     * what the manifest says reads a list of its element's shape. An element read out is held for
     * the session the list was read in, as a field's value is.
     */
    record ReceivedList(Received element, ListCrossing crossing) implements Received {

        public ReceivedList {
            listing(element, crossing);
            if (crossing.read() == null) {
                throw new IllegalArgumentException("a list of " + crossing.element() + " is read"
                        + " by nothing");
            }
        }

        @Override
        public Shape shape() {
            return new Shape.ListOf(element.shape());
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
        public String of(List<String> words, String session) {
            Manifest.ListRead read = java.util.Objects.requireNonNull(crossing.read());
            List<String> rooms = new ArrayList<>();
            for (int at = 0; at < element.words().size(); at++) {
                rooms.add("$r" + at);
            }
            return session + "->elements('" + read.length().name() + "', '" + read.at().name()
                    + "', " + columns(element) + ", " + words.getFirst() + ", static fn ("
                    + rooms.stream().map(it -> "\\FFI\\CData " + it).collect(Collectors.joining(", "))
                    + "): " + element.phpType() + " => "
                    + element.of(element.fromRooms(rooms), session) + ")";
        }
    }

    /** What a docblock calls a function value taking {@code takes} and answering {@code answers}. */
    private static String functionDocType(List<? extends Crossing> takes, Crossing answers) {
        return "\\Closure(" + takes.stream().map(Crossing::phpDocType)
                .collect(Collectors.joining(", ")) + "): " + answers.phpDocType();
    }

    /** Refuses a function value called or made through another shape's functions. */
    private static void signing(List<? extends Crossing> takes, Crossing answers,
                                FunctionCrossing crossing) {
        Manifest.Signature signature = new Manifest.Signature(
                takes.stream().map(Crossing::shape).toList(), answers.shape());
        if (!crossing.signature().equals(signature)) {
            throw new IllegalArgumentException("a function of " + signature + " is not reached"
                    + " through the functions of one of " + crossing.signature());
        }
    }

    /**
     * A function value PHP hands over, as a {@code \\Closure} of its own: made into one through
     * the slot the binding keeps for its type ({@code \\Souther\\Runtime\\FunctionSlot}), which
     * the library calls it through with what it takes, as PHP is handed each, for as long as the
     * run it was handed over in.
     *
     * @param binding the generated binding's class, as PHP names it
     * @param slot    what the binding keeps the slot a closure of this type is called through
     *                under: one for each function type as PHP holds it, and not for each shape,
     *                since two types crossing in one shape are made into two sets of classes
     */
    record GivenFunction(List<Received> takes, Given answers, FunctionCrossing crossing,
                         String binding, String slot) implements Given {

        public GivenFunction {
            takes = List.copyOf(takes);
            signing(takes, answers, crossing);
            if (crossing.make() == null) {
                throw new IllegalArgumentException("a function of " + crossing.signature()
                        + " is made by nothing");
            }
        }

        @Override
        public Shape shape() {
            return new Shape.FunctionOf(crossing.signature());
        }

        @Override
        public String phpType() {
            return "\\Closure";
        }

        @Override
        public String phpDocType() {
            return functionDocType(takes, answers);
        }

        @Override
        public List<String> given(String value, String session) {
            return List.of(binding + "::in(" + session + "->library())->hosting('" + slot
                    + "')->implement(" + session + ", " + value + ")");
        }

        @Override
        public String holds(String value) {
            return value + " instanceof \\Closure";
        }
    }

    /**
     * A function value the library hands PHP, as a {@code \\Closure} calling it through the
     * manifest's {@code call} for its shape: called with what the function takes, as PHP hands each
     * over, in the innermost run going when it is called, and answering what the function answered,
     * or throwing where it ended without an answer.
     *
     * @param binding the generated binding's class, as PHP names it
     */
    record ReceivedFunction(List<Given> takes, Received answers, FunctionCrossing crossing,
                            String binding) implements Received {

        public ReceivedFunction {
            takes = List.copyOf(takes);
            signing(takes, answers, crossing);
            if (crossing.call() == null) {
                throw new IllegalArgumentException("a function of " + crossing.signature()
                        + " is called by nothing");
            }
        }

        @Override
        public Shape shape() {
            return new Shape.FunctionOf(crossing.signature());
        }

        @Override
        public String phpType() {
            return "\\Closure";
        }

        @Override
        public String phpDocType() {
            return functionDocType(takes, answers);
        }

        @Override
        public String of(List<String> words, String session) {
            List<String> parameters = new ArrayList<>(
                    List.of("\\Souther\\Runtime\\Session $session", "\\FFI\\CData $function"));
            List<String> given = new ArrayList<>(List.of("$function"));
            for (int at = 0; at < takes.size(); at++) {
                parameters.add(takes.get(at).phpType() + " $a" + at);
                given.addAll(takes.get(at).given("$a" + at, "$session"));
            }
            StringBuilder rooms = new StringBuilder();
            List<String> held = new ArrayList<>();
            List<Word> answered = answers.words();
            for (int at = 0; at < answered.size(); at++) {
                rooms.append("$r").append(at).append(" = $ffi->new('")
                        .append(Crossing.storage(answered.get(at))).append("'); ");
                given.add("\\FFI::addr($r" + at + ")");
                held.add("$r" + at);
            }
            return session + "->callable(" + binding + "::class, " + words.getFirst()
                    + ", static function (" + String.join(", ", parameters) + "): "
                    + answers.phpType() + " { $ffi = $session->call(); " + rooms
                    + "$session->answered($ffi->"
                    + java.util.Objects.requireNonNull(crossing.call()).name() + "("
                    + String.join(", ", given) + ")); return "
                    + answers.of(answers.fromRooms(held), "$session") + "; })";
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
    record OneOf(Manifest.Type.Union union, List<Member> members) implements Given {

        public OneOf {
            members = List.copyOf(members);
            if (union.cases().size() != members.size()) {
                throw new IllegalArgumentException(members + " are not the members of " + union);
            }
        }

        @Override
        public Shape shape() {
            return new Shape.Leaf(Word.VALUE);
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
     * member the case {@code which} says it is, each of {@code cases} at the place the library
     * counts it.
     *
     * @param phpType the union of what PHP calls each of its members
     * @param cases   how a value of each case is made, in the order {@code which} counts them
     * @param what    what the answer is of, for the exception a case past them throws
     */
    record Told(Manifest.Function which, String phpType, List<Member> cases, String what)
            implements Received {

        public Told {
            cases = List.copyOf(cases);
        }

        @Override
        public Shape shape() {
            return new Shape.Leaf(Word.VALUE);
        }

        @Override
        public String of(List<String> words, String session) {
            String word = words.getFirst();
            StringBuilder match = new StringBuilder("match (" + session + "->ffi()->"
                    + which.name() + "(" + word + ")) {\n");
            for (int at = 0; at < cases.size(); at++) {
                match.append("            ").append(at).append(" => ")
                        .append(cases.get(at).of(word, session)).append(",\n");
            }
            return match.append("            default => throw new \\LogicException('the library")
                    .append(" answered a case ").append(what).append(" does not answer'),\n")
                    .append("        }").toString();
        }
    }
}
