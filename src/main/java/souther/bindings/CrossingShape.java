package souther.bindings;

import org.jspecify.annotations.Nullable;
import souther.bindings.Manifest.Answer;
import souther.bindings.Manifest.Case;
import souther.bindings.Manifest.Element;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Implementation;
import souther.bindings.Manifest.ListCrossing;
import souther.bindings.Manifest.Module;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.UnionAnswer;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.List;

/**
 * How a value of a model type crosses between a host and the library, as the library's ABI has it:
 * the words it is handed over as, the functions a list of it is built and read through, and what
 * says which case a union a behavior answers is. The counterpart of {@code Host} in the driver's
 * {@code host.rs}, read back out of a manifest.
 *
 * <p>Nothing here is of any host's language. Where this answers null, the ABI hands a value of the
 * type no way, and no host is handed one. Where it answers a shape, a host's generator still
 * decides whether its language has a way to hold what crosses: a union no declaration names crosses
 * as one word both ways, and a language whose every value of it has to be an object of a class the
 * binding generated has none for a union whose member it generated no class for.
 *
 * <p>The two ways are asked apart ({@link #given}, {@link #received}), because a union no
 * declaration names crosses one way and not the other. A host handing one over hands over the
 * value, which already is the case it is. A host handed one has to be told which case it is before
 * it can hold it as that, and only a behavior's answer says ({@link Told}); a union anywhere else,
 * at any depth, is one a host is handed no way.
 */
public sealed interface CrossingShape {

    /** The words a value of it is handed over as, in order. */
    List<Word> words();

    /** How a value crosses from the library to a host. */
    sealed interface Received extends CrossingShape {
    }

    /**
     * How a value crosses both ways, and the same way each: one word, or a presence beside one for
     * an optional.
     */
    sealed interface Both extends Received {
    }

    /** One word: what an optional is a presence beside, and what a list holds either of. */
    sealed interface Single extends Both {

        Word word();

        @Override
        default List<Word> words() {
            return List.of(word());
        }
    }

    /**
     * The value itself: an {@code Int}, a {@code Bool} or a {@code String}, or the address of a
     * value of a declared type or of a union every member of which is one, which a host holds and
     * never reads behind.
     *
     * @param type what the model says it is: a {@link Type.Primitive}, a {@link Type.Declared} or
     *             a {@link Type.Union}
     */
    record Whole(Word word, Type type) implements Single {
    }

    /**
     * A list, as one word: built by {@code crossing}'s {@code construct} out of a column for each
     * word {@code element} crosses as, and read as {@code length} elements, each written by
     * {@code at} into room for those words.
     */
    record Listed(Both element, ListCrossing crossing) implements Single {

        @Override
        public Word word() {
            return Word.LIST;
        }
    }

    /**
     * An optional: whether it holds a value, as a {@code BOOL}, then the value, or nothing where it
     * holds none.
     */
    record Present(Single of) implements Both {

        @Override
        public List<Word> words() {
            return List.of(Word.BOOL, of.word());
        }
    }

    /**
     * A union no declaration names, answered by a behavior: one word, and {@code which} answers
     * which of {@code cases} a value is, counting them in the order they are listed. A member that
     * is a sum stands among them as its own cases.
     */
    record Told(Type.Union union, List<Case> cases, Function which) implements Received {

        @Override
        public List<Word> words() {
            return List.of(Word.VALUE);
        }
    }

    /**
     * How a host hands the library a value of {@code type} in a function of {@code module}'s, or
     * null where the ABI has no way.
     *
     * @throws IllegalStateException where a list crosses and {@code module} says nothing to build
     *                               one of its element through
     */
    static @Nullable Both given(Module module, Type type) {
        if (type instanceof Type.Option option) {
            Single of = single(module, option.of());
            return of == null ? null : new Present(of);
        }
        return single(module, type);
    }

    /**
     * How the library hands a host a value of {@code type} in a function of {@code module}'s, or
     * null where it has no way: as it is handed the other way, except that a union no declaration
     * names, wherever it is in {@code type}, is handed no way, since nothing says which case it is.
     *
     * @throws IllegalStateException where a list crosses and {@code module} says nothing to build
     *                               one of its element through
     */
    static @Nullable Both received(Module module, Type type) {
        Both shape = given(module, type);
        return shape == null || holdsAUnion(shape) ? null : shape;
    }

    /**
     * How the library hands a host what a behavior of {@code module}'s answers, or null where it
     * has no way: a union no declaration names as the case {@code which} says a value is, and
     * anything else as a value of its type is handed.
     *
     * @throws IllegalStateException where the manifest says {@code which} is other than what tells
     *                               a union's cases apart
     */
    static @Nullable Received received(Module module, Answer answer) {
        if (!(answer.type() instanceof Type.Union union)) {
            return received(module, answer.type());
        }
        UnionAnswer cases = answer.union();
        if (given(module, union) == null || cases == null || cases.which() == null) {
            return null;
        }
        agreesAsWhich(cases.which());
        return new Told(union, cases.cases(), cases.which());
    }

    /** How a value of the declared type {@code module.name} crosses: one word, wherever it is. */
    static Whole declared(String module, String name) {
        return new Whole(Word.VALUE, new Type.Declared(module, name));
    }

    // ---------------------------------------------------------------------------------------------
    // What each function a host calls or implements takes and answers.
    //
    // The manifest says each function's words, and the shapes above are a second reading of the
    // same thing. Where the two disagree a binding would call the function as something it is not,
    // so each is held to the other before anything is written to call it.

    /**
     * Holds a behavior's or a published value's {@code function} to taking what it is called with
     * first where it {@code requires} what it was constructed with, then {@code takes}, and to
     * writing {@code answers} through room and answering a status.
     */
    static void agreesAsCall(Function function, boolean requires,
                             List<? extends CrossingShape> takes, CrossingShape answers) {
        List<Word> given = new ArrayList<>();
        if (requires) {
            given.add(Word.REQUIREMENTS);
        }
        given.addAll(words(takes));
        agrees(function, given, answers.words(), Word.STATUS);
    }

    /** Holds a declared type's {@code construct} to taking {@code fields} and writing the value. */
    static void agreesAsConstruct(Function construct, List<? extends CrossingShape> fields) {
        agrees(construct, words(fields), List.of(Word.VALUE), Word.STATUS);
    }

    /**
     * Holds a field's {@code read} to taking the value and answering the field: one word as its
     * answer, and an optional as whether it is there, the value written through room.
     */
    static void agreesAsRead(Function read, Both field) {
        switch (field) {
            case Single single -> agrees(read, List.of(Word.VALUE), List.of(), single.word());
            case Present present -> agrees(read, List.of(Word.VALUE), List.of(present.of().word()),
                    Word.BOOL);
        }
    }

    /** Holds a sum's or a behavior's {@code which} to taking a value and answering its case. */
    static void agreesAsWhich(Function which) {
        agrees(which, List.of(Word.VALUE), List.of(), Word.CASE);
    }

    /** Holds a declared type's {@code encode} to taking a value and answering its text. */
    static void agreesAsEncode(Function encode) {
        agrees(encode, List.of(Word.VALUE), List.of(), Word.STRING);
    }

    /**
     * Holds a declared type's {@code decode}, or its {@code decodehost}, to taking bytes and their
     * count and writing what the reading came to.
     */
    static void agreesAsDecode(Function decode) {
        agrees(decode, List.of(Word.BYTES, Word.COUNT), List.of(Word.DECODED), Word.STATUS);
    }

    /**
     * Holds the {@code implementation} of {@code behavior} a host implements to being handed what
     * it was handed where its capability was made, then {@code takes}, and to writing
     * {@code answers} through room and answering a status.
     */
    static void agreesAsImplementation(Implementation implementation, String behavior,
                                       List<? extends CrossingShape> takes,
                                       CrossingShape answers) {
        List<Parameter> expected = new ArrayList<>();
        // What the implementation was handed where its capability was made comes first, and the
        // runtime takes it off before a host is handed the rest.
        expected.add(Parameter.given(Word.USERDATA));
        words(takes).forEach(word -> expected.add(Parameter.given(word)));
        answers.words().forEach(word -> expected.add(Parameter.room(word)));
        if (!expected.equals(implementation.takes()) || implementation.answers() != Word.STATUS) {
            throw new IllegalStateException("the manifest says an implementation of " + behavior
                    + " takes " + implementation.takes() + ", and this generator would hand it "
                    + expected);
        }
    }

    private static void agrees(Function function, List<Word> given, List<Word> rooms,
                               @Nullable Word answers) {
        List<Parameter> expected = new ArrayList<>();
        given.forEach(word -> expected.add(Parameter.given(word)));
        rooms.forEach(word -> expected.add(Parameter.room(word)));
        if (!expected.equals(function.takes()) || answers != function.answers()) {
            throw new IllegalStateException("the manifest says " + function.name() + " takes "
                    + function.takes() + " and answers " + function.answers()
                    + ", and this generator would call it with " + expected + " for " + answers);
        }
    }

    private static List<Word> words(List<? extends CrossingShape> shapes) {
        return shapes.stream().flatMap(it -> it.words().stream()).toList();
    }

    // ---------------------------------------------------------------------------------------------
    // Working a shape out.

    /**
     * How a value of {@code type} crosses as one word, or null where it does not. Every primitive
     * the ABI hands over named, as {@code host.rs} names them: one the manifest spells otherwise is
     * one no host is handed.
     */
    private static @Nullable Single single(Module module, Type type) {
        return switch (type) {
            case Type.Primitive it -> switch (it.name()) {
                case "Int" -> new Whole(Word.INT, it);
                case "Bool" -> new Whole(Word.BOOL, it);
                case "String" -> new Whole(Word.STRING, it);
                default -> null;
            };
            case Type.Declared it -> new Whole(Word.VALUE, it);
            // What holds a union holds one of its members, each of which says which it is, where
            // every member is a declared type.
            case Type.Union it -> it.cases().stream().allMatch(Case.Declared.class::isInstance)
                    ? new Whole(Word.VALUE, it) : null;
            case Type.ListOf it -> listed(module, it);
            // An optional inside an optional would need a presence for each.
            case Type.Option it -> null;
            case Type.Unrepresented it -> null;
        };
    }

    /**
     * How a list crosses, where its element does: through the functions {@code module} says for a
     * list of such elements. An element that crosses with nothing in the module to build a list of
     * it through is the manifest disagreeing with itself, since the library defines one for every
     * list its module's functions hand across, and is refused rather than taken for a list no host
     * can reach.
     */
    private static @Nullable Listed listed(Module module, Type.ListOf list) {
        Both element = given(module, list.of());
        if (element == null) {
            return null;
        }
        Element shape = switch (element) {
            case Present present -> new Element(true, present.of().word());
            case Single single -> new Element(false, single.word());
        };
        ListCrossing crossing = module.lists().stream()
                .filter(it -> it.element().equals(shape))
                .findFirst()
                .orElseThrow(() -> new IllegalStateException("the manifest gives module `"
                        + module.name() + "` nothing to build a list of " + shape + " through, and"
                        + " a function of it hands one across"));
        return new Listed(element, crossing);
    }

    private static boolean holdsAUnion(Both shape) {
        return switch (shape) {
            case Whole whole -> whole.type() instanceof Type.Union;
            case Listed listed -> holdsAUnion(listed.element());
            case Present present -> holdsAUnion(present.of());
        };
    }
}
