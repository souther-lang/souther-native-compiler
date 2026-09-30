package souther.bindings.rust;

import org.jspecify.annotations.Nullable;
import souther.bindings.Manifest;
import souther.bindings.Manifest.ListCrossing;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.stream.Collectors;
import java.util.stream.IntStream;

/**
 * How a value of one model type crosses between Rust and the library: the shape the manifest says
 * it crosses in ({@link Shape}), which says the words it is handed over as, and on it the Rust types
 * a value of it is and the Rust that turns one into the other.
 *
 * <p>The shape is the library's decision, read off the manifest; what is decided here is only how
 * Rust holds what crosses in it. Every value has two Rust types: the one it is {@link #owned} as,
 * which is what the library hands Rust, and the one it is taken as where Rust hands one over, its
 * {@link #view}, which borrows what it can ({@code &str} for a {@code String}, a slice for a list).
 * Every view is {@code Copy}, so what hands one over can read it as often as it has words to write.
 *
 * <p>What is written here is held to three rules, so that what the crate is made of is Rust a lint
 * has nothing to say about:
 *
 * <ul>
 *   <li>{@link #given} is handed a place expression of the view and writes one expression for each
 *       word, which may name {@code run} (the run the value is handed over in), {@code library}
 *       (what it is a run of) and {@code ?}, in a function answering what a {@code Failure}
 *       becomes. A value of a declared type and a function value are handed over only through
 *       what checks the runtime that made them is {@code run}'s ({@code __word}).
 *   <li>{@link #of} is handed an identifier for each word and writes one expression standing on
 *       its own: {@code unsafe} around exactly what of it reads what the library answered, and
 *       never around what another crossing wrote, so no block is inside another.
 *   <li>{@link #viewOf} is handed an identifier of a reference to the owned value.
 * </ul>
 */
sealed interface Crossing {

    /** How the value crosses, as the manifest says. */
    Shape shape();

    /** The type Rust is handed a value of this as, and keeps. */
    String owned();

    /** The type Rust hands a value of this over as. */
    String view();

    /** The words it is handed over as, in order. */
    default List<Word> words() {
        return shape().words();
    }

    /** The expressions handing {@code value}, a place of {@link #view}, over, one for each word. */
    List<String> given(String value);

    /** The expression of this crossing's {@link #owned} value made of {@code words}, identifiers. */
    String of(List<String> words);

    /** The expression of {@link #view} of {@code owned}, an identifier of {@code &owned()}. */
    String viewOf(String owned);

    /**
     * What a type named after this one calls it: a function type's enum is named after what it
     * takes and answers.
     */
    String label();

    /** What Rust calls one word, as it stands in a function's parameters and in room. */
    static String word(Word word) {
        return switch (word) {
            case STATUS -> "u32";
            case INT, COUNT, SCOPE -> "i64";
            case BOOL -> "u8";
            case CASE -> "u32";
            case OUTCOME -> "i32";
            case BYTES -> "*const u8";
            case VALUE, STRING, DECIMAL, DATE, TIME, DATETIME, INSTANT, DECODED, ISSUE, LIST,
                 FUNCTION -> "rt::Word";
            case REQUIREMENTS -> "*const *const rt::Capability";
            case CAPABILITY -> "rt::Capability";
            case USERDATA -> "*mut std::ffi::c_void";
        };
    }

    /**
     * What room for {@code word} starts as, before the library writes it: a word it writes only
     * where there is something to write, so what an optional holding nothing leaves is this.
     */
    static String nothing(Word word) {
        return switch (word) {
            case INT, COUNT, SCOPE, STATUS, CASE, OUTCOME, BOOL -> "0";
            case BYTES, VALUE, STRING, DECIMAL, DATE, TIME, DATETIME, INSTANT, DECODED, ISSUE,
                 LIST, FUNCTION, REQUIREMENTS -> "std::ptr::null()";
            case USERDATA -> "std::ptr::null_mut()";
            case CAPABILITY -> throw new IllegalArgumentException("a capability is no room of a value");
        };
    }

    /**
     * {@code let name = expression;}, or nothing where the expression is the name itself: what is
     * written binds only what it changes, since a binding that changes nothing is one a lint
     * refuses and a reader reads twice.
     */
    static String let(String name, String expression) {
        return expression.equals(name) ? "" : "let " + name + " = " + expression + ";";
    }

    /** {@code path} without the lifetime a type is written with. */
    private static String bare(String path) {
        int at = path.indexOf('<');
        return at < 0 ? path : path.substring(0, at);
    }

    /** The last segment of {@code path}, without its lifetime. */
    private static String last(String path) {
        String bare = bare(path);
        return bare.substring(bare.lastIndexOf(':') + 1);
    }

    /**
     * One word of the library's that is the value itself: a primitive, or a value of a declared
     * type as the handle generated for it. Held the same way both ways.
     *
     * @param type Rust's type of it, and for a handle its path as {@code crate::m::Name}
     */
    record Whole(Shape.Leaf shape, Kind kind, String type) implements Crossing {

        enum Kind { INT, BOOL, STRING, DECIMAL, DATE, TIME, DATETIME, INSTANT, HANDLE }

        public Whole {
            Word is = switch (kind) {
                case INT -> Word.INT;
                case BOOL -> Word.BOOL;
                case STRING -> Word.STRING;
                case DECIMAL -> Word.DECIMAL;
                case DATE -> Word.DATE;
                case TIME -> Word.TIME;
                case DATETIME -> Word.DATETIME;
                case INSTANT -> Word.INSTANT;
                case HANDLE -> Word.VALUE;
            };
            if (shape.word() != is) {
                throw new IllegalArgumentException("a " + kind + " is not held as " + shape);
            }
        }

        /**
         * A value of the primitive {@code name} crossing as {@code word}, as Rust's own type for it
         * or the runtime's, or null where this binding has no way to hold that pair. Both are
         * asked, the name and the word, as the PHP binding asks them.
         */
        static @Nullable Whole primitive(Manifest.Primitive primitive, Word word) {
            Kind kind = switch (primitive) {
                case INT -> word == Word.INT ? Kind.INT : null;
                case BOOL -> word == Word.BOOL ? Kind.BOOL : null;
                case STRING -> word == Word.STRING ? Kind.STRING : null;
                case DECIMAL -> word == Word.DECIMAL ? Kind.DECIMAL : null;
                // Each of the four as the runtime's type for it, held as the numbers it means and
                // handed over as those numbers.
                case DATE -> word == Word.DATE ? Kind.DATE : null;
                case TIME -> word == Word.TIME ? Kind.TIME : null;
                case DATETIME -> word == Word.DATETIME ? Kind.DATETIME : null;
                case INSTANT -> word == Word.INSTANT ? Kind.INSTANT : null;
                // Held by objects and never handed to a host.
                case RATIONAL -> null;
            };
            if (kind == null) {
                return null;
            }
            String type = switch (kind) {
                case INT -> "i64";
                case BOOL -> "bool";
                case STRING -> "String";
                case DECIMAL -> "rt::Decimal";
                case DATE -> "rt::Date";
                case TIME -> "rt::Time";
                case DATETIME -> "rt::DateTime";
                case INSTANT -> "rt::Instant";
                case HANDLE -> throw new IllegalStateException("a primitive is no handle");
            };
            return new Whole(new Shape.Leaf(word), kind, type);
        }

        /** A value of a declared type, as the handle generated for it. */
        static Whole handle(String type) {
            return new Whole(new Shape.Leaf(Word.VALUE), Kind.HANDLE, type);
        }

        @Override
        public String owned() {
            return kind == Kind.HANDLE ? type + "<'run>" : type;
        }

        @Override
        public String view() {
            return switch (kind) {
                case INT, BOOL, DATE, TIME, DATETIME, INSTANT -> type;
                case STRING -> "&str";
                case DECIMAL -> "&rt::Decimal";
                case HANDLE -> owned();
            };
        }

        @Override
        public List<String> given(String value) {
            return List.of(switch (kind) {
                case INT -> value;
                case BOOL -> "u8::from(" + value + ")";
                case STRING -> "library.words.string(run, " + value + ")?";
                // The unscaled digits never go through String admission (souther-native-compiler#109):
                // an integer's text is never the value's written form, so it is not fallible on a
                // String's own capacity.
                case DECIMAL -> "library.words.decimal(run, " + value + ")?";
                case DATE -> "library.words.date(run, " + value + ")?";
                case TIME -> "library.words.time(run, " + value + ")?";
                case DATETIME -> "library.words.date_time(run, " + value + ")?";
                case INSTANT -> "library.words.instant(run, " + value + ")?";
                case HANDLE -> type + "::__word(" + value + ", run)?";
            });
        }

        @Override
        public String of(List<String> words) {
            String word = words.getFirst();
            return switch (kind) {
                case INT -> word;
                case BOOL -> word + " != 0";
                case STRING -> "unsafe { library.words.text(" + word + ") }";
                case DECIMAL -> "unsafe { library.words.amount(" + word + ") }";
                case DATE -> "unsafe { library.words.date_of(" + word + ") }";
                case TIME -> "unsafe { library.words.time_of(" + word + ") }";
                case DATETIME -> "unsafe { library.words.date_time_of(" + word + ") }";
                case INSTANT -> "unsafe { library.words.instant_of(" + word + ") }";
                case HANDLE -> "unsafe { " + type + "::__held(library, " + word + ") }";
            };
        }

        @Override
        public String viewOf(String owned) {
            return switch (kind) {
                case INT, BOOL, DATE, TIME, DATETIME, INSTANT, HANDLE -> "*" + owned;
                case STRING -> owned + ".as_str()";
                case DECIMAL -> owned;
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
                case HANDLE -> last(type);
            };
        }
    }

    /** An optional: {@code Option} of what it holds, both ways. */
    record Optional(Crossing of) implements Crossing {

        @Override
        public Shape shape() {
            return new Shape.Option(of.shape());
        }

        @Override
        public String owned() {
            return "Option<" + of.owned() + ">";
        }

        @Override
        public String view() {
            return "Option<" + of.view() + ">";
        }

        @Override
        public List<String> given(String value) {
            List<String> words = new ArrayList<>(List.of("u8::from(" + value + ".is_some())"));
            List<String> inner = of.given("held");
            List<Word> innerWords = of.words();
            for (int at = 0; at < inner.size(); at++) {
                // What an optional holding nothing leaves is the word's default where the word is
                // what it holds as it is.
                words.add(inner.get(at).equals("held") && Crossing.nothing(innerWords.get(at))
                        .equals("0") ? value + ".unwrap_or_default()"
                        : "match " + value + " { Some(held) => " + inner.get(at) + ", None => "
                        + Crossing.nothing(innerWords.get(at)) + " }");
            }
            return words;
        }

        @Override
        public String of(List<String> words) {
            return "if " + words.getFirst() + " != 0 { Some(" + of.of(words.subList(1, words.size()))
                    + ") } else { None }";
        }

        /**
         * The view of what it holds, written the way Rust writes each: an optional of a {@code Copy}
         * view is one itself, one of a reference is {@code as_ref}, one of text {@code as_deref}.
         */
        @Override
        public String viewOf(String owned) {
            String inner = of.viewOf("held");
            return switch (inner) {
                case "*held" -> "*" + owned;
                case "held" -> owned + ".as_ref()";
                case "held.as_str()" -> owned + ".as_deref()";
                default -> owned + ".as_ref().map(|held| " + inner + ")";
            };
        }

        @Override
        public String label() {
            return "Option" + of.label();
        }
    }

    /** A tuple, as a Rust tuple of its members. */
    record Tuple(List<Crossing> members) implements Crossing {

        public Tuple {
            members = List.copyOf(members);
        }

        @Override
        public Shape shape() {
            return new Shape.Product(members.stream().map(Crossing::shape).toList());
        }

        @Override
        public String owned() {
            return tuple(members.stream().map(Crossing::owned).toList());
        }

        @Override
        public String view() {
            return tuple(members.stream().map(Crossing::view).toList());
        }

        @Override
        public List<String> given(String value) {
            List<String> words = new ArrayList<>();
            for (int at = 0; at < members.size(); at++) {
                words.addAll(members.get(at).given(value + "." + at));
            }
            return words;
        }

        @Override
        public String of(List<String> words) {
            List<String> made = new ArrayList<>();
            int at = 0;
            for (Crossing member : members) {
                int wide = member.words().size();
                made.add(member.of(words.subList(at, at + wide)));
                at += wide;
            }
            return tuple(made);
        }

        /** Each member opened out of the reference by a pattern, as a reference of its own. */
        @Override
        public String viewOf(String owned) {
            List<String> names = IntStream.range(0, members.size()).mapToObj(at -> "member" + at)
                    .toList();
            return "{ let " + tuple(names) + " = " + owned + "; " + tuple(IntStream
                    .range(0, members.size()).mapToObj(at -> members.get(at).viewOf(names.get(at)))
                    .toList()) + " }";
        }

        @Override
        public String label() {
            return "Tuple" + members.stream().map(Crossing::label).collect(Collectors.joining("And"));
        }

        /** A Rust tuple of {@code of}: a one-member tuple keeps its comma. */
        private static String tuple(List<String> of) {
            return of.size() == 1 ? "(" + of.getFirst() + ",)" : "(" + String.join(", ", of) + ")";
        }
    }

    /**
     * A list: handed over as a slice of what its element is owned as, and handed to Rust as a
     * {@code Vec} of it, built and read through the functions the manifest names for a list of its
     * element's shape.
     *
     * @param construct the field of the symbol table building one, or null where nothing does
     * @param length    the field reading how many elements one holds, or null where nothing does
     * @param at        the field reading one element, or null where nothing does
     */
    record Listed(Crossing element, ListCrossing crossing, @Nullable String construct,
                  @Nullable String length, @Nullable String at) implements Crossing {

        public Listed {
            if (!crossing.element().equals(element.shape())) {
                throw new IllegalArgumentException("a list of " + element.shape() + " is not reached"
                        + " through the functions of a list of " + crossing.element());
            }
        }

        @Override
        public Shape shape() {
            return new Shape.ListOf(element.shape());
        }

        @Override
        public String owned() {
            return "Vec<" + element.owned() + ">";
        }

        @Override
        public String view() {
            return "&[" + element.owned() + "]";
        }

        /**
         * The list, built of a column of each word its element crosses as, every element's at its
         * index. The columns are read for the length of the call and not kept.
         */
        @Override
        public List<String> given(String value) {
            Objects.requireNonNull(construct, "a list handed over is built by something");
            List<Word> words = element.words();
            StringBuilder block = new StringBuilder("{ let elements = ").append(value).append(";");
            for (int column = 0; column < words.size(); column++) {
                block.append(" let mut column").append(column).append(": Vec<")
                        .append(Crossing.word(words.get(column)))
                        .append("> = Vec::with_capacity(elements.len());");
            }
            block.append(" for element in elements { ")
                    .append(Crossing.let("element", element.viewOf("element")));
            List<String> handed = element.given("element");
            for (int column = 0; column < handed.size(); column++) {
                block.append(" column").append(column).append(".push(").append(handed.get(column))
                        .append(");");
            }
            block.append(" } let count = i64::try_from(elements.len()).expect(\"a list's length is a"
                    + " 64-bit count\"); let mut list = std::ptr::null(); let made = unsafe {"
                    + " (library.symbols.").append(construct).append(")(count");
            for (int column = 0; column < words.size(); column++) {
                block.append(", column").append(column).append(".as_ptr()");
            }
            block.append(", &mut list) }; rt::made(made, list)? }");
            return List.of(block.toString());
        }

        /** Each element read into room of its own, in order. */
        @Override
        public String of(List<String> words) {
            Objects.requireNonNull(at, "a list handed over is read by something");
            List<Word> rooms = element.words();
            StringBuilder block = new StringBuilder("{ let list = ").append(words.getFirst())
                    .append("; let count = unsafe { (library.symbols.").append(length)
                    .append(")(list) }; let mut elements = Vec::with_capacity(usize::try_from(count)"
                            + ".expect(\"a list's length is never below nought\")); for index in"
                            + " 0..count {");
            List<String> held = new ArrayList<>();
            for (int room = 0; room < rooms.size(); room++) {
                block.append(" let mut room").append(room).append(": ")
                        .append(Crossing.word(rooms.get(room))).append(" = ")
                        .append(Crossing.nothing(rooms.get(room))).append(";");
                held.add("room" + room);
            }
            block.append(" let inside = unsafe { (library.symbols.").append(at).append(")(list, index");
            for (String room : held) {
                block.append(", &mut ").append(room);
            }
            block.append(") }; assert!(inside != 0, \"the library answers every element of a list"
                    + " below its length\"); elements.push(").append(element.of(held))
                    .append("); } elements }");
            return block.toString();
        }

        @Override
        public String viewOf(String owned) {
            return owned + ".as_slice()";
        }

        @Override
        public String label() {
            return "List" + element.label();
        }
    }

    /**
     * A value of a union no declaration names, as the enum generated for it: a variant for each of
     * its members, a declared one holding its handle and a primitive Rust's own value. Handed over as
     * a reference to one, since a member may be one Rust owns; handed to Rust only where the library
     * says which case it is, which a behavior's answer does ({@code told}).
     *
     * @param type    the enum, as {@code crate::m::Name}, with its lifetime where a member has one
     * @param members each member, in the order the union names them
     * @param told    how each case the library counts is made, in its order, and what counts it;
     *                null where the library says nothing of which case a value is
     */
    record OneOf(String type, List<Member> members, @Nullable Told told) implements Crossing {

        /**
         * One member: the variant it is, how Rust holds its value, and where it is a primitive, the
         * fields of the symbol table carrying a value of it into the union and reading it back out.
         */
        record Member(String variant, Whole whole, @Nullable String make, @Nullable String read) {
        }

        /**
         * What the library says a value is: {@code which} counts the cases the union descends to,
         * and each is made as the member it is, or the member sum it is a case of.
         */
        record Told(String which, List<Arm> arms) {
        }

        /** One case the library counts: the member it is made as, and how, of {@code value}. */
        record Arm(Member member, String made) {
        }

        public OneOf {
            members = List.copyOf(members);
        }

        @Override
        public Shape shape() {
            return new Shape.Leaf(Word.VALUE);
        }

        @Override
        public String owned() {
            return type;
        }

        @Override
        public String view() {
            return "&" + type;
        }

        @Override
        public List<String> given(String value) {
            StringBuilder arms = new StringBuilder("match " + value + " {");
            for (Member member : members) {
                String word = member.whole().given(member.whole().viewOf("held")).getFirst();
                arms.append(" ").append(bare(type)).append("::").append(member.variant())
                        .append("(held) => ").append(member.make() == null ? word
                                : "{ let word = " + word + "; unsafe { (library.symbols."
                                + member.make() + ")(word) } }").append(",");
            }
            return List.of(arms.append(" }").toString());
        }

        @Override
        public String of(List<String> words) {
            Objects.requireNonNull(told, "a union is handed to Rust only where it is told its case");
            StringBuilder arms = new StringBuilder("{ let value = " + words.getFirst() + "; match"
                    + " unsafe { (library.symbols." + told.which() + ")(value) } {");
            for (int place = 0; place < told.arms().size(); place++) {
                Arm arm = told.arms().get(place);
                arms.append(" ").append(place).append(" => ").append(bare(type)).append("::")
                        .append(arm.member().variant()).append("(").append(arm.made()).append("),");
            }
            return arms.append(" _ => unreachable!(\"the library answered a case the union does not"
                    + " have\"), } }").toString();
        }

        @Override
        public String viewOf(String owned) {
            return owned;
        }

        @Override
        public String label() {
            return last(type);
        }
    }

    /**
     * A function value, as the enum generated for its type: one the library made, which the enum
     * calls through the library, or a function of the host's own, which the library calls through
     * the run it is handed over in. Handed over as a reference to one, and handed to Rust as one
     * the library made.
     *
     * @param type the enum, as {@code crate::m::Name<'run>}
     */
    record Function(String type, Shape.FunctionOf shape) implements Crossing {

        @Override
        public String owned() {
            return type;
        }

        @Override
        public String view() {
            return "&" + type;
        }

        @Override
        public List<String> given(String value) {
            return List.of(bare(type) + "::__word(" + value + ", run)?");
        }

        @Override
        public String of(List<String> words) {
            return "unsafe { " + bare(type) + "::__held(library, " + words.getFirst() + ") }";
        }

        @Override
        public String viewOf(String owned) {
            return owned;
        }

        @Override
        public String label() {
            return last(type);
        }
    }

    /** What the view of each of {@code crossings} is, joined as the parameters of a signature say. */
    static String views(List<? extends Crossing> crossings) {
        return crossings.stream().map(Crossing::view).collect(Collectors.joining(", "));
    }
}
