package souther.bindings;

import org.jspecify.annotations.Nullable;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * What a library offers a host and asks of one, as every host's generator is handed it: the model
 * of what the driver wrote beside the library, and not how it was written.
 *
 * <p>The command reads what the driver wrote, and is what knows how it is written and which
 * version of it this is; a generator is handed this and never the file, so what the file looks
 * like can change without a generator knowing.
 *
 * <p>Made only by {@link #of}, which holds it to what no part can hold alone; what it is made of
 * is plain records a generator takes apart as it needs. What the driver promises of a
 * manifest is held by the part that holds it, wherever that part is made: a function by the words
 * of the shapes said beside it ({@link Call}, {@link Construct}, {@link Read}), a module by saying
 * the functions for every list and every function value it hands across, each the way it crosses
 * ({@link Module}), and what constructing a behavior requires by being closed over the whole.
 *
 * <p>How a value crosses is what the driver decided and the manifest says ({@link Shape}); nothing
 * here works it out again from the model, and nothing holds a shape to the type it is said beside:
 * which shape a type crosses in is the driver's to choose, and a shape it chooses later for a type
 * is read the day it is written. What a generator adds is whether its own language has a way to
 * hold what crosses, and a pair of a type and a shape it has none for is one it does not bind.
 */
public final class Manifest {

    private final Map<String, Integer> statuses;
    private final Map<String, Integer> outcomes;
    private final List<CaseCrossing> cases;
    private final List<Module> modules;

    /**
     * What constructing a behavior requires is closed, as the driver holds a library's surface to
     * be: every behavior a construction requires is one a module here constructs or asks a host to
     * implement. A construction naming what nothing constructs would be found out by a host, at a
     * call, as something no binding can build.
     */
    private Manifest(Map<String, Integer> statuses, Map<String, Integer> outcomes,
                     List<CaseCrossing> cases, List<Module> modules) {
        this.statuses = Collections.unmodifiableMap(new LinkedHashMap<>(statuses));
        this.outcomes = Collections.unmodifiableMap(new LinkedHashMap<>(outcomes));
        this.cases = List.copyOf(cases);
        this.modules = List.copyOf(modules);
        // One way to make and read each case, and one for every case no declaration names of a
        // union that crosses, wherever the manifest says one does: a behavior's answer, what an
        // injection answers, a parameter, a field, a published value, a sum. Asked of every union
        // that crosses and not of where a generator happens to need one, so a manifest leaving one
        // out is refused here as incomplete rather than read by a generator as a union its
        // language has no way to hold.
        Set<Case> crossed = new HashSet<>();
        for (CaseCrossing crossing : this.cases) {
            if (!crossed.add(crossing.of())) {
                throw new IllegalArgumentException("it says twice how " + crossing.of()
                        + " is made and read");
            }
        }
        for (Module module : this.modules) {
            for (Case of : carriedIn(module)) {
                if (!crossed.contains(of)) {
                    throw new IllegalArgumentException("it says module `" + module.name()
                            + "` hands across " + of + ", and nothing of how a value of it is made"
                            + " or read");
                }
            }
        }
        Set<String> constructible = new HashSet<>();
        for (Module module : this.modules) {
            module.constructions().forEach(it -> constructible.add(module.name() + "." + it.name()));
            module.injections().forEach(it -> constructible.add(module.name() + "." + it.name()));
        }
        for (Module module : this.modules) {
            for (Construction construction : module.constructions()) {
                for (Required required : construction.requires()) {
                    if (!constructible.contains(required.key())) {
                        throw new IllegalArgumentException("it says " + module.name() + "."
                                + construction.name() + " requires " + required.key()
                                + ", which nothing in it constructs or asks a host to implement");
                    }
                }
            }
        }
    }

    /**
     * A library's model, of what the driver says of it.
     *
     * @param statuses every status a function answering one answers, by name
     * @param outcomes what a reading comes to, by name
     * @param cases    how a host makes and reads each case no declaration names
     * @param modules  what each module offers and asks
     * @throws IllegalArgumentException where what it says is not whole: a case a module hands
     *         across with nothing to make and read it through, or a construction requiring what
     *         nothing constructs
     */
    public static Manifest of(Map<String, Integer> statuses, Map<String, Integer> outcomes,
                              List<CaseCrossing> cases, List<Module> modules) {
        return new Manifest(statuses, outcomes, cases, modules);
    }

    /**
     * Every status a function answering one answers, by name: the one saying it answered, each
     * reason a computation ends without a value, and what a host's implementation brings about.
     */
    public Map<String, Integer> statuses() {
        return statuses;
    }

    /** What a reading comes to, by name. */
    public Map<String, Integer> outcomes() {
        return outcomes;
    }

    /**
     * How a host makes and reads a value of each case no declaration names, as a union holds one:
     * a property of the case, whatever union it stands in.
     */
    public List<CaseCrossing> cases() {
        return cases;
    }

    /**
     * How a value of {@code of} is made and read: said for every case of a union that crosses,
     * which a manifest is refused where it leaves out.
     *
     * @throws IllegalArgumentException where {@code of} is not a case a host makes or reads
     */
    public CaseCrossing crossing(Case of) {
        return cases.stream().filter(it -> it.of().equals(of)).findFirst()
                .orElseThrow(() -> new IllegalArgumentException(of + " is not a case a host makes"
                        + " or reads through the runtime"));
    }

    /**
     * Every case no declaration names of each union that crosses in something {@code module}
     * reaches: each one a host hands over or is handed only through the runtime.
     */
    private static List<Case> carriedIn(Module module) {
        List<Crossed> crossed = new ArrayList<>();
        for (Behavior behavior : module.behaviors()) {
            if (behavior.call() instanceof Reach.Available<Call>(Call call)) {
                crossed.addAll(Crossed.of(behavior.parameters().types(), call.signature().takes()));
                crossed.add(new Crossed(behavior.answers().type(), call.signature().answers()));
            }
        }
        for (Injection injection : module.injections()) {
            crossed.addAll(Crossed.of(injection.parameters().stream().map(NamedParameter::type)
                    .toList(), injection.signature().takes()));
            crossed.add(new Crossed(injection.answers(), injection.signature().answers()));
        }
        for (PublishedValue value : module.values()) {
            if (value.read() instanceof Reach.Available<Call>(Call call)) {
                crossed.add(new Crossed(value.type(), call.signature().answers()));
            }
        }
        for (Declaration declaration : module.declarations()) {
            for (Field field : declaration.fields()) {
                if (field.read() instanceof Reach.Available<Read>(Read read)) {
                    crossed.add(new Crossed(field.type(), read.answers()));
                }
            }
            if (Declaration.built(declaration) instanceof Construct construct) {
                crossed.addAll(Crossed.of(declaration.fields().stream().map(Field::type).toList(),
                        construct.takes()));
            }
        }
        List<Case> carried = new ArrayList<>();
        for (Crossed it : crossed) {
            it.carried(carried);
        }
        for (Declaration declaration : module.declarations()) {
            if (declaration instanceof Declaration.Sum sum && sum.which() != null) {
                sum.cases().stream().filter(it -> !(it instanceof Case.Declared))
                        .forEach(carried::add);
            }
        }
        return carried;
    }

    /** A type, and the shape a value of it crosses in. */
    private record Crossed(Type type, Shape shape) {

        private static List<Crossed> of(List<Type> types, List<Shape> shapes) {
            List<Crossed> crossed = new ArrayList<>();
            for (int at = 0; at < types.size(); at++) {
                crossed.add(new Crossed(types.get(at), shapes.get(at)));
            }
            return crossed;
        }

        /**
         * Every case no declaration names of a union that crosses here as one value, into {@code
         * carried}: where the manifest says a union crosses as a {@code VALUE}, a value of such a
         * case is made and read through the runtime ({@link Manifest#cases}). Followed only where
         * the type and the shape are made alike; how else a type crosses is the driver's to say, and
         * nothing here holds it to one way.
         */
        private void carried(List<Case> carried) {
            switch (shape) {
                case Shape.Leaf leaf -> {
                    if (type instanceof Type.Union union && leaf.word() == Word.VALUE) {
                        union.cases().stream().filter(it -> !(it instanceof Case.Declared))
                                .forEach(carried::add);
                    }
                }
                case Shape.Option option -> {
                    if (type instanceof Type.Option it) {
                        new Crossed(it.of(), option.of()).carried(carried);
                    }
                }
                case Shape.Product product -> {
                    if (type instanceof Type.Tuple it && it.of().size() == product.of().size()) {
                        of(it.of(), product.of()).forEach(member -> member.carried(carried));
                    }
                }
                case Shape.ListOf list -> {
                    if (Type.listed(type) instanceof Type element) {
                        new Crossed(element, list.element()).carried(carried);
                    }
                }
                case Shape.FunctionOf function -> {
                    if (type instanceof Type.Function it
                            && it.takes().size() == function.signature().takes().size()) {
                        of(it.takes(), function.signature().takes())
                                .forEach(taken -> taken.carried(carried));
                        new Crossed(it.answers(), function.signature().answers()).carried(carried);
                    }
                }
            }
        }
    }

    /** What each module of the library offers a host. */
    public List<Module> modules() {
        return modules;
    }

    /** One word a host hands over or is handed. */
    public enum Word {
        STATUS, INT, BOOL, CASE, OUTCOME, COUNT, SCOPE, BYTES, VALUE, STRING, DECIMAL, DATE, TIME,
        DATETIME, INSTANT, DECODED, ISSUE, LIST, REQUIREMENTS, CAPABILITY, USERDATA, FUNCTION
    }

    /**
     * One parameter of a function: a word handed over, room the function writes one through, or as
     * many of a word as another parameter counts, which the function reads.
     */
    public record Parameter(Mode mode, Word word) {

        public enum Mode { GIVEN, ROOM, SLICE }

        public static Parameter given(Word word) {
            return new Parameter(Mode.GIVEN, word);
        }

        public static Parameter room(Word word) {
            return new Parameter(Mode.ROOM, word);
        }

        public static Parameter slice(Word word) {
            return new Parameter(Mode.SLICE, word);
        }
    }

    /** A function the library defines, by its symbol. */
    public record Function(String name, List<Parameter> takes, @Nullable Word answers) {

        public Function {
            takes = List.copyOf(takes);
        }

        /**
         * Refuses this unless it takes {@code first}, then each of {@code given} handed over, then
         * room for each of {@code rooms}, and answers {@code answers}: what the shapes said beside
         * it make of it.
         */
        private void takes(List<Parameter> first, List<Shape> given, List<Shape> rooms,
                   @Nullable Word answers) {
            List<Parameter> expected = new ArrayList<>(first);
            Shape.words(given).forEach(word -> expected.add(Parameter.given(word)));
            Shape.words(rooms).forEach(word -> expected.add(Parameter.room(word)));
            if (!expected.equals(takes) || answers != this.answers) {
                throw new IllegalArgumentException(name + " takes " + takes + " and answers "
                        + this.answers + ", where what is said beside it crosses as " + expected
                        + " answering " + answers);
            }
        }
    }

    /**
     * The shape a value crosses between a host and the library in, as the driver decided it: the
     * words it is handed over as, and what they are made of. Not a type of the model: a value of a
     * declared type and one of a union are both one {@code VALUE}, and a tuple is a
     * {@link Product} of its members. What the value is, is the {@link Type} said beside it.
     */
    public sealed interface Shape {

        /** The words a value crossing in this is handed over as, in order. */
        List<Word> words();

        /** The words of each of {@code shapes}, one after another. */
        static List<Word> words(List<Shape> shapes) {
            return shapes.stream().flatMap(it -> it.words().stream()).toList();
        }

        /**
         * One word that is the value itself: an {@code INT}, a {@code BOOL}, a {@code STRING}, a
         * {@code DECIMAL}, a {@code DATE}, a {@code TIME}, a {@code DATETIME}, an {@code INSTANT}
         * or a {@code VALUE}.
         */
        record Leaf(Word word) implements Shape {

            /** Every word a value is handed over whole as, which a manifest's leaf is read against. */
            public static final List<Word> WORDS =
                    List.of(Word.INT, Word.BOOL, Word.STRING, Word.DECIMAL, Word.DATE, Word.TIME,
                            Word.DATETIME, Word.INSTANT, Word.VALUE);

            public Leaf {
                if (!WORDS.contains(word)) {
                    throw new IllegalArgumentException(word + " is not a value handed over whole");
                }
            }

            @Override
            public List<Word> words() {
                return List.of(word);
            }
        }

        /**
         * An optional: a {@code BOOL} saying whether it holds a value, then the words of what it
         * holds, read and written only where it holds one.
         */
        record Option(Shape of) implements Shape {

            @Override
            public List<Word> words() {
                List<Word> words = new ArrayList<>(List.of(Word.BOOL));
                words.addAll(of.words());
                return List.copyOf(words);
            }
        }

        /** Each member's words, one member after another. */
        record Product(List<Shape> of) implements Shape {

            public Product {
                of = List.copyOf(of);
            }

            @Override
            public List<Word> words() {
                return Shape.words(of);
            }
        }

        /** One {@code LIST}, built and read through the {@link ListCrossing} for {@code element}. */
        record ListOf(Shape element) implements Shape {

            @Override
            public List<Word> words() {
                return List.of(Word.LIST);
            }
        }

        /** One {@code FUNCTION}, called and made through the {@link FunctionCrossing} for it. */
        record FunctionOf(Signature signature) implements Shape {

            @Override
            public List<Word> words() {
                return List.of(Word.FUNCTION);
            }
        }
    }

    /** What something takes and answers, as the shape each value crosses in. */
    public record Signature(List<Shape> takes, Shape answers) {

        public Signature {
            takes = List.copyOf(takes);
        }
    }

    /** What reaches a value, or why nothing does. */
    public sealed interface Reach<T> {

        /** What reaches it. */
        record Available<T>(T it) implements Reach<T> {
        }

        /** Why nothing does. */
        record Unavailable<T>(Refusal refusal) implements Reach<T> {
        }

        /** What reaches it, or null where nothing does. */
        default @Nullable T available() {
            return this instanceof Available<T>(T it) ? it : null;
        }
    }

    /**
     * Why a host has no way to a value: what stands in the way, and where in what the function
     * would hand over or be handed it stands, from the outside in. A binding says it in its own
     * language's words.
     */
    public record Refusal(Reason reason, List<Step> path) {

        public Refusal {
            path = List.copyOf(path);
        }
    }

    /** What stands in the way of a value crossing to a host. */
    public enum Reason {
        /**
         * A type with no representation for a host: a {@code Rational}, which has no external
         * form, and a set or a map in what a function value takes or answers.
         */
        NO_REPRESENTATION,
        /** A type with no value to hand over. */
        NO_VALUE,
        /** A union a host would be handed with nothing to say which case it is. */
        NO_DISCRIMINATOR
    }

    /** One step into what a function hands over or is handed. */
    public sealed interface Step {

        /** What is taken at this place, counted from nought. */
        record Takes(int at) implements Step {
        }

        /** What is answered. */
        record Answers() implements Step {
        }

        /** A field of a declared type, by its name. */
        record Field(String name) implements Step {
        }

        /** What an optional holds. */
        record Option() implements Step {
        }

        /** A tuple's member at this place. */
        record Member(int at) implements Step {
        }

        /** A list's element. */
        record Element() implements Step {
        }
    }

    /** A behavior's or a published value's call, and the shape each value it takes and answers crosses in. */
    public record Call(Function function, Signature signature) {

        /**
         * Refuses this unless its function takes what it was constructed with first where
         * {@code constructed}, then what the signature takes, then room for what it answers, and
         * answers a status.
         */
        private void calls(boolean constructed) {
            function.takes(constructed ? List.of(Parameter.given(Word.REQUIREMENTS)) : List.of(),
                    signature.takes(), List.of(signature.answers()), Word.STATUS);
        }
    }

    /** A declared type's constructor, and the shape each field is handed over in. */
    public record Construct(Function function, List<Shape> takes) {

        public Construct {
            takes = List.copyOf(takes);
            function.takes(List.of(), takes, List.of(new Shape.Leaf(Word.VALUE)), Word.STATUS);
        }
    }

    /** A field's reader, and the shape the field is handed over in. */
    public record Read(Function function, Shape answers) {

        public Read {
            function.takes(List.of(Parameter.given(Word.VALUE)), List.of(), List.of(answers), null);
        }
    }

    /**
     * What one module offers a host. It says one list for each shape an element crosses in, and
     * one function crossing for each shape a function value crosses in, since a list or a function
     * reached through two sets of functions is two things said of one; and it says one for every
     * list and function value anything here hands across, since one handed across with nothing to
     * build or call it through is one no host can reach.
     */
    public record Module(String name, List<Behavior> behaviors, List<Construction> constructions,
                         List<Injection> injections,
                         List<PublishedValue> values, List<Declaration> declarations,
                         List<ListCrossing> lists, List<FunctionCrossing> functions) {

        public Module {
            behaviors = List.copyOf(behaviors);
            constructions = List.copyOf(constructions);
            injections = List.copyOf(injections);
            values = List.copyOf(values);
            declarations = List.copyOf(declarations);
            lists = List.copyOf(lists);
            functions = List.copyOf(functions);
            Map<Shape, ListCrossing> listed = new LinkedHashMap<>();
            for (ListCrossing list : lists) {
                if (listed.put(list.element(), list) != null) {
                    throw new IllegalArgumentException("module `" + name + "` says two lists of "
                            + list.element());
                }
            }
            Map<Signature, FunctionCrossing> called = new LinkedHashMap<>();
            for (FunctionCrossing function : functions) {
                if (called.put(function.signature(), function) != null) {
                    throw new IllegalArgumentException("module `" + name + "` says two function"
                            + " crossings of " + function.signature());
                }
            }
            for (Crossing crossing : crossingsIn(behaviors, injections, values, declarations, lists,
                    functions)) {
                offered(name, crossing.shape(), crossing.way(), listed, called);
            }
        }

        /**
         * What this module says a list whose elements cross in {@code element} is built and read
         * through.
         *
         * @throws IllegalArgumentException where it says nothing of one
         */
        public ListCrossing listOf(Shape element) {
            return lists.stream().filter(it -> it.element().equals(element)).findFirst()
                    .orElseThrow(() -> new IllegalArgumentException("module `" + name
                            + "` says nothing of a list of " + element));
        }

        /**
         * What this module says a function value of {@code signature} is called and made through.
         *
         * @throws IllegalArgumentException where it says nothing of one
         */
        public FunctionCrossing functionOf(Signature signature) {
            return functions.stream().filter(it -> it.signature().equals(signature)).findFirst()
                    .orElseThrow(() -> new IllegalArgumentException("module `" + name
                            + "` says nothing of a function value of " + signature));
        }

        /**
         * Refuses {@code shape}, crossing the way {@code way} says, where anything in it is a list
         * or a function value this module says nothing to reach that way through: a list a host
         * hands over with nothing to build it, one it is handed with nothing to read it, a function
         * value it is handed with nothing to call it, and one it hands over with nothing to make
         * it. What a function value takes crosses the other way from the value.
         */
        private static void offered(String module, Shape shape, Way way,
                                    Map<Shape, ListCrossing> lists,
                                    Map<Signature, FunctionCrossing> functions) {
            switch (shape) {
                case Shape.Leaf leaf -> { }
                case Shape.Option option -> offered(module, option.of(), way, lists, functions);
                case Shape.Product product -> product.of()
                        .forEach(member -> offered(module, member, way, lists, functions));
                case Shape.ListOf list -> {
                    ListCrossing crossing = lists.get(list.element());
                    boolean there = crossing != null && (way == Way.GIVEN
                            ? crossing.construct() != null : crossing.read() != null);
                    if (!there) {
                        throw new IllegalArgumentException("module `" + module + "` hands a list"
                                + " of " + list.element() + " across and says nothing to "
                                + (way == Way.GIVEN ? "build" : "read") + " one through");
                    }
                    offered(module, list.element(), way, lists, functions);
                }
                case Shape.FunctionOf function -> {
                    FunctionCrossing crossing = functions.get(function.signature());
                    boolean there = crossing != null && (way == Way.HANDED
                            ? crossing.call() != null : crossing.make() != null);
                    if (!there) {
                        throw new IllegalArgumentException("module `" + module + "` hands a"
                                + " function of " + function.signature() + " across and says"
                                + " nothing to " + (way == Way.HANDED ? "call" : "make")
                                + " one through");
                    }
                    function.signature().takes().forEach(
                            taken -> offered(module, taken, way.turned(), lists, functions));
                    offered(module, function.signature().answers(), way, lists, functions);
                }
            }
        }

        /** A shape said somewhere in a module, and the way a value crossing in it crosses there. */
        private record Crossing(Shape shape, Way way) {
        }

        /**
         * Every shape said anywhere in a module made of these, with the way it crosses there, which
         * is the way its words go: what a function takes is handed over by a host, and what it
         * writes through room is handed to one. A function a host writes is called the other way
         * round.
         */
        private static List<Crossing> crossingsIn(List<Behavior> behaviors,
                                                  List<Injection> injections,
                                                  List<PublishedValue> values,
                                                  List<Declaration> declarations,
                                                  List<ListCrossing> lists,
                                                  List<FunctionCrossing> functions) {
            List<Crossing> crossings = new ArrayList<>();
            java.util.function.BiConsumer<Signature, Way> signed = (signature, way) -> {
                signature.takes().forEach(it -> crossings.add(new Crossing(it, way)));
                crossings.add(new Crossing(signature.answers(), way.turned()));
            };
            behaviors.forEach(it -> {
                if (it.call().available() instanceof Call call) {
                    signed.accept(call.signature(), Way.GIVEN);
                }
            });
            injections.forEach(it -> signed.accept(it.signature(), Way.HANDED));
            values.forEach(it -> {
                if (it.read().available() instanceof Call call) {
                    signed.accept(call.signature(), Way.GIVEN);
                }
            });
            for (Declaration declaration : declarations) {
                if (Declaration.built(declaration) instanceof Construct construct) {
                    construct.takes().forEach(it -> crossings.add(new Crossing(it, Way.GIVEN)));
                }
                for (Field field : declaration.fields()) {
                    if (field.read().available() instanceof Read read) {
                        crossings.add(new Crossing(read.answers(), Way.HANDED));
                    }
                }
            }
            for (ListCrossing list : lists) {
                if (list.construct() != null) {
                    crossings.add(new Crossing(list.element(), Way.GIVEN));
                }
                if (list.read() != null) {
                    crossings.add(new Crossing(list.element(), Way.HANDED));
                }
            }
            for (FunctionCrossing function : functions) {
                if (function.call() != null) {
                    signed.accept(function.signature(), Way.GIVEN);
                }
                if (function.make() != null) {
                    signed.accept(function.signature(), Way.HANDED);
                }
            }
            return crossings;
        }
    }

    /** Which way a value crosses: handed over by a host, or handed to one. */
    public enum Way {
        GIVEN, HANDED;

        /** The other way, which is the way what a function value takes crosses. */
        public Way turned() {
            return this == GIVEN ? HANDED : GIVEN;
        }
    }

    /**
     * What a list whose elements cross in the shape {@code element} is built through, where a host
     * hands one over, and read through, where it is handed one: a list of one declared type
     * through the same functions as a list of any other. Each function is of the shape a list of
     * that element is: {@code (count, a slice for each word an element crosses as) -> list}, and
     * {@code (list) -> count} and {@code (list, index, room for each word) -> bool}. At least one
     * of the two is there.
     */
    public record ListCrossing(Shape element, @Nullable Function construct,
                               @Nullable ListRead read) {

        public ListCrossing {
            if (construct == null && read == null) {
                throw new IllegalArgumentException("a list of " + element + " is neither built nor"
                        + " read");
            }
            if (construct != null) {
                List<Parameter> built = new ArrayList<>(List.of(Parameter.given(Word.COUNT)));
                element.words().forEach(word -> built.add(Parameter.slice(word)));
                if (!built.equals(construct.takes()) || construct.answers() != Word.LIST) {
                    throw new IllegalArgumentException("a list of " + element + " is built through "
                            + construct.name() + ", which takes " + construct.takes()
                            + " and answers " + construct.answers());
                }
            }
            if (read != null) {
                read.length().takes(List.of(Parameter.given(Word.LIST)), List.of(), List.of(),
                        Word.COUNT);
                read.at().takes(List.of(Parameter.given(Word.LIST), Parameter.given(Word.COUNT)),
                        List.of(), List.of(element), Word.BOOL);
            }
        }
    }

    /** What a host reads a list through: its length, and its element at an index. */
    public record ListRead(Function length, Function at) {
    }

    /**
     * What a function value crossing as {@code signature} is called through, where a host is
     * handed one, {@code (function, what it takes, room for its answer) -> status}; and what a host
     * makes one of its own through, where one is taken from a host. At least one of the two is
     * there.
     */
    public record FunctionCrossing(Signature signature, @Nullable Function call,
                                   @Nullable FunctionMaking make) {

        public FunctionCrossing {
            if (call == null && make == null) {
                throw new IllegalArgumentException("a function of " + signature + " is neither"
                        + " called nor made");
            }
            if (call != null) {
                call.takes(List.of(Parameter.given(Word.FUNCTION)), signature.takes(),
                        List.of(signature.answers()), Word.STATUS);
            }
            if (make != null) {
                make.implementation().answering(signature);
            }
        }
    }

    /**
     * What a host makes a function value of its own through: the {@code implementation} it writes,
     * handed what it was handed first where the value was made, then what the value was called
     * with, and room for its answer; and {@code implement}, which makes a value of an
     * implementation out of room a host laid out.
     */
    public record FunctionMaking(Implementation implementation, String implement) {
    }

    /** A published behavior, and what a host calls it through, or why nothing does. */
    public record Behavior(String name, Parameters parameters, Answer answers, Reach<Call> call) {

        /**
         * What a host calls it through takes what it was constructed with first, and each
         * parameter and the answer in a shape that fits its type. What tells an answer's cases
         * apart is there exactly where the behavior can be called: a host is handed the answer
         * only by the call, and one told nothing of which case it was handed could name the union
         * and not hold it.
         */
        public Behavior {
            if (call.available() instanceof Call it) {
                it.calls(true);
            }
            UnionAnswer union = answers.union();
            if (union != null && (union.which() != null) != (call.available() != null)) {
                throw new IllegalArgumentException("it says " + name + " is called "
                        + (call.available() == null ? "no way" : "by "
                        + call.available().function().name()) + " and its answer's case is told "
                        + (union.which() == null ? "no way" : "by " + union.which().name())
                        + ", where the one is there exactly where the other is");
            }
        }
    }

    /**
     * A behavior a host constructs the capabilities of what a call is made with out of, whether or
     * not it may call it by name: what constructing it requires injected, and what a host makes a
     * capability of it through where something may require it.
     */
    public record Construction(String name, List<Required> requires, @Nullable Function bind) {

        public Construction {
            requires = List.copyOf(requires);
        }
    }

    /** A behavior another requires injected, by its module and its name. */
    public record Required(String module, String name) {

        /** The module and the name joined the one way, which no two behaviors share. */
        public String key() {
            return module + "." + name;
        }
    }

    /**
     * What a behavior answers, and where that is a union no declaration names, what a host tells
     * its cases apart by.
     */
    public record Answer(Type type, @Nullable UnionAnswer union) {

        /** Said of every union no declaration names that a behavior answers, and of nothing else. */
        public Answer {
            if ((type instanceof Type.Union) != (union != null)) {
                throw new IllegalArgumentException("an answer of " + type + " says "
                        + (union == null ? "nothing of its cases" : "cases of a union it is not"));
            }
        }
    }

    /**
     * The cases a union a behavior answers descends to, a member that is a sum as its own cases,
     * and what says which of them a value is, in the order they are listed.
     */
    public record UnionAnswer(List<Case> cases, @Nullable Function which) {

        public UnionAnswer {
            cases = oneOrMore(cases, "a union answer");
            if (which != null) {
                which.takes(List.of(Parameter.given(Word.VALUE)), List.of(), List.of(), Word.CASE);
            }
        }
    }

    /**
     * {@code cases}, owned, where there is one: every set of alternatives a manifest names has a
     * case in it, and a binding telling a value apart by which of none it is would have nothing to
     * make of it.
     */
    private static List<Case> oneOrMore(List<Case> cases, String of) {
        if (cases.isEmpty()) {
            throw new IllegalArgumentException(of + " with no case in it");
        }
        return List.copyOf(cases);
    }

    /** What a behavior takes: named as its declaration names them, or in order for a composition. */
    public sealed interface Parameters {

        /** Each type a behavior takes, in order, with or without the name it is declared under. */
        List<Type> types();

        record Named(List<NamedParameter> parameters) implements Parameters {

            public Named {
                parameters = List.copyOf(parameters);
            }

            @Override
            public List<Type> types() {
                return parameters.stream().map(NamedParameter::type).toList();
            }
        }

        record Positional(List<Type> types) implements Parameters {

            public Positional {
                types = List.copyOf(types);
            }
        }
    }

    public record NamedParameter(String name, Type type) {
    }

    /**
     * A behavior a host implements, the shape each value it takes and answers crosses in, and what
     * a host makes a capability of an implementation of its own through: {@code (room for a
     * capability, room for a souther_hosted, the implementation, what it is handed first)}.
     */
    public record Injection(String name, List<NamedParameter> parameters, Type answers,
                            Signature signature, Implementation implementation, String implement) {

        public Injection {
            parameters = List.copyOf(parameters);
            implementation.answering(signature);
        }
    }

    /** The C type of the function a host implements a behavior or a function value as. */
    public record Implementation(String type, List<Parameter> takes, Word answers) {

        public Implementation {
            takes = List.copyOf(takes);
        }

        /**
         * Refuses this unless it takes what it was handed first where it was laid out, then what
         * {@code signature} takes, then room for what it answers, and answers a status.
         */
        private void answering(Signature signature) {
            new Function(type, takes, answers).takes(List.of(Parameter.given(Word.USERDATA)),
                    signature.takes(), List.of(signature.answers()), Word.STATUS);
        }
    }

    /** A published value, and what a host reads it through, or why nothing does. */
    public record PublishedValue(String name, Type type, Reach<Call> read) {

        public PublishedValue {
            if (read.available() instanceof Call call) {
                call.calls(false);
                if (!call.signature().takes().isEmpty()) {
                    throw new IllegalArgumentException("the value " + name + " is read by a"
                            + " function taking " + call.signature().takes());
                }
            }
        }
    }

    /** A published type, with what a host reaches it through. */
    public sealed interface Declaration {

        String name();

        /** Its fields, in order. A unit and a sum have none. */
        List<Field> fields();

        /**
         * What builds a value of it, where something does: a product, a newtype and a unit say
         * whether and why not, and a sum is never built.
         */
        static @Nullable Construct built(Declaration declaration) {
            return switch (declaration) {
                case Product it -> it.construct().available();
                case Newtype it -> it.construct().available();
                case Unit it -> it.construct().available();
                case Sum it -> null;
            };
        }

        /** Reads a value of it out of text in its external form. */
        @Nullable Function decode();

        /**
         * Reads a value of it out of a value a host built of ordered maps, written with every
         * container as an object: where the reader takes an array, a map keyed by its indices is
         * one.
         */
        @Nullable Function decodeHost();

        @Nullable Function encode();

        record Product(String name, List<Field> fields, Reach<Construct> construct,
                       @Nullable Function decode, @Nullable Function decodeHost,
                       @Nullable Function encode) implements Declaration {

            public Product {
                fields = List.copyOf(fields);
            }
        }

        record Newtype(String name, Field field, Reach<Construct> construct,
                       @Nullable Function decode, @Nullable Function decodeHost,
                       @Nullable Function encode) implements Declaration {

            @Override
            public List<Field> fields() {
                return List.of(field);
            }
        }

        record Unit(String name, Reach<Construct> construct, @Nullable Function decode,
                    @Nullable Function decodeHost, @Nullable Function encode)
                implements Declaration {

            @Override
            public List<Field> fields() {
                return List.of();
            }
        }

        /** A sum, and the cases {@code which} counts, where a host can be handed each of them. */
        record Sum(String name, List<Case> cases, @Nullable Function which,
                   @Nullable Function decode, @Nullable Function decodeHost,
                   @Nullable Function encode) implements Declaration {

            public Sum {
                cases = oneOrMore(cases, "sum `" + name + "`");
                if (which != null) {
                    which.takes(List.of(Parameter.given(Word.VALUE)), List.of(), List.of(),
                            Word.CASE);
                }
            }

            @Override
            public List<Field> fields() {
                return List.of();
            }
        }

    }

    /** A field, and what a host reads it through, or why nothing does. */
    public record Field(String name, Type type, Reach<Read> read) {
    }

    /**
     * A primitive the language has, closed: a generator switches over every one, so a primitive
     * the language gains is a generator that stops compiling rather than one that quietly binds
     * nothing for it.
     */
    public enum Primitive {
        INT("Int"), STRING("String"), BOOL("Bool"), DECIMAL("Decimal"), RATIONAL("Rational"),
        DATE("Date"), TIME("Time"), DATETIME("DateTime"), INSTANT("Instant");

        private final String spelt;

        Primitive(String spelt) {
            this.spelt = spelt;
        }

        /** Its name as the language spells it. */
        public String spelt() {
            return spelt;
        }
    }

    /** A type as the model says it, every one it has. */
    public sealed interface Type {

        record Primitive(Manifest.Primitive primitive) implements Type {
        }

        record Declared(String module, String name) implements Type {
        }

        record Union(List<Case> cases) implements Type {

            public Union {
                cases = oneOrMore(cases, "a union");
            }
        }

        record Option(Type of) implements Type {
        }

        record Tuple(List<Type> of) implements Type {

            public Tuple {
                of = List.copyOf(of);
            }
        }

        record Function(List<Type> takes, Type answers) implements Type {

            public Function {
                takes = List.copyOf(takes);
            }
        }

        record ListOf(Type of) implements Type {
        }

        record SetOf(Type of) implements Type {
        }

        record MapOf(Type key, Type value) implements Type {
        }

        /**
         * What the elements of the list a value of {@code type} crosses as are, where it crosses as
         * one: a list's, a set's members, and a map's entries, each the tuple of its key and its
         * value. A set and a map cross to a host as the list of what they hold, which the library
         * makes one of again where it is handed one, once each and the later pair's value; the
         * order the list is in is no order the language says anything of. Null for any other type.
         */
        static @Nullable Type listed(Type type) {
            return switch (type) {
                case ListOf it -> it.of();
                case SetOf it -> it.of();
                case MapOf it -> new Tuple(List.of(it.key(), it.value()));
                default -> null;
            };
        }

        /** What has no value. */
        record Nothing() implements Type {
        }

        /** What does not answer. */
        record Never() implements Type {
        }
    }

    /** One case of a sum or of a union. */
    public sealed interface Case {

        /** A declared type, whose value is the case's value as it is. */
        record Declared(String module, String name) implements Case {
        }

        /** A primitive, carried: made and read through {@link Manifest#cases}. */
        record Primitive(Manifest.Primitive primitive) implements Case {
        }

        /** A case the language gives, which holds nothing: made through {@link Manifest#cases}. */
        record Language(String name) implements Case {
        }
    }

    /**
     * How a host makes a value of a case no declaration names and reads what it holds: {@code make}
     * takes what the case holds, where it holds something, and answers the value; {@code read}
     * takes a value that says it is the case and answers what it holds. A primitive holds itself
     * and a case the language gives nothing, so one has a {@code read} and the other none.
     */
    public record CaseCrossing(Case of, Function make, @Nullable Function read) {

        public CaseCrossing {
            if (of instanceof Case.Declared) {
                throw new IllegalArgumentException(of + " is a declared type, which a host holds as"
                        + " it is and makes through its own constructor");
            }
            if ((of instanceof Case.Primitive) != (read != null)) {
                throw new IllegalArgumentException(of + " is read " + (read == null ? "no way" : "by "
                        + read.name()) + ", where a primitive holds itself and a case the language"
                        + " gives nothing");
            }
            if (make.answers() != Word.VALUE || (read != null && (!read.takes().equals(
                    List.of(Parameter.given(Word.VALUE))) || read.answers() == null
                    || !make.takes().equals(List.of(Parameter.given(read.answers())))))
                    || (read == null && !make.takes().isEmpty())) {
                throw new IllegalArgumentException(of + " is made by " + make + " and read by " + read
                        + ", which are not one value and what it holds, both ways");
            }
        }

        /** The word what a value of this case holds is handed over as, where it holds one. */
        public @Nullable Word holds() {
            return read == null ? null : read.answers();
        }
    }
}
