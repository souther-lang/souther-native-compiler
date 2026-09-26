package souther.bindings;

import net.unit8.raoh.Result;
import net.unit8.raoh.decode.Decoder;
import org.jspecify.annotations.Nullable;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

import static net.unit8.raoh.decode.Decoders.lazy;
import static net.unit8.raoh.decode.Decoders.oneOf;
import static net.unit8.raoh.json.JsonDecoders.combine;
import static net.unit8.raoh.json.JsonDecoders.field;
import static net.unit8.raoh.json.JsonDecoders.int_;
import static net.unit8.raoh.json.JsonDecoders.list;
import static net.unit8.raoh.json.JsonDecoders.literal;
import static net.unit8.raoh.json.JsonDecoders.map;
import static net.unit8.raoh.json.JsonDecoders.nullableField;
import static net.unit8.raoh.json.JsonDecoders.string;
import static net.unit8.raoh.json.JsonDecoders.strict;

/**
 * What a manifest says, as every host's generator reads it: version {@value #VERSION} of
 * {@value #FORMAT}, and nothing else.
 *
 * <p>Read strictly, as the driver writes it. A member this does not name, or a version or ABI
 * generation it was not written for, is refused rather than read as much of as happens to parse:
 * a binding generated from a manifest that says more than was understood of it would call
 * functions as something they are not.
 *
 * <p>Made only by {@link #read}, so a manifest a generator holds is one read that way; what it is
 * made of is plain records a generator takes apart as it needs. What the driver promises of a
 * manifest is held by the part that holds it, wherever that part is made: a list by the shape of
 * its functions ({@link ListCrossing}), a module by saying one list for each element
 * ({@link Module}), and what constructing a behavior requires by being closed over the whole.
 */
public final class Manifest {

    /** What a manifest says it is. */
    public static final String FORMAT = "souther-native-interface";

    /** The version of what a manifest says that this reads. */
    public static final int VERSION = 9;

    /** The ABI generation the functions this binds answer to. */
    public static final int ABI = 4;

    private final int abi;
    private final Map<String, Integer> statuses;
    private final Map<String, Integer> outcomes;
    private final List<Function> runtime;
    private final List<CaseCrossing> cases;
    private final List<Module> modules;

    /**
     * What constructing a behavior requires is closed, as the driver holds a library's surface to
     * be: every behavior a construction requires is one a module here constructs or asks a host to
     * implement. A construction naming what nothing constructs would be found out by a host, at a
     * call, as something no binding can build.
     */
    private Manifest(int abi, Map<String, Integer> statuses, Map<String, Integer> outcomes,
                     List<Function> runtime, List<CaseCrossing> cases, List<Module> modules) {
        this.abi = abi;
        this.statuses = Collections.unmodifiableMap(new LinkedHashMap<>(statuses));
        this.outcomes = Collections.unmodifiableMap(new LinkedHashMap<>(outcomes));
        this.runtime = List.copyOf(runtime);
        this.cases = List.copyOf(cases);
        this.modules = List.copyOf(modules);
        // One way to make and read each case, and one for every case no declaration names that a
        // host may be handed or hand over, wherever the manifest names it: a behavior's answer, what
        // an injection answers, a parameter, a field, a published value, a sum. Asked of every
        // case the manifest names and not of where a generator happens to need one, so a manifest
        // leaving one out is refused here as incomplete rather than read by a generator as a
        // union its language has no way to hold.
        Set<Case> crossed = new HashSet<>();
        for (CaseCrossing crossing : this.cases) {
            if (!crossed.add(crossing.of())) {
                throw new IllegalArgumentException("it says twice how " + crossing.of()
                        + " is made and read");
            }
        }
        for (Module module : this.modules) {
            for (Case of : casesIn(module)) {
                if (crossesCarried(of) && !crossed.contains(of)) {
                    throw new IllegalArgumentException("it says module `" + module.name()
                            + "` holds " + of + ", and nothing of how a value of it is made or"
                            + " read");
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

    /** The ABI generation every function named here answers to. */
    public int abi() {
        return abi;
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

    /** The runtime's functions a host calls. */
    public List<Function> runtime() {
        return runtime;
    }

    /**
     * How a host makes and reads a value of each case no declaration names, as a union holds one:
     * a property of the case, whatever union it stands in.
     */
    public List<CaseCrossing> cases() {
        return cases;
    }

    /**
     * How a value of {@code of} is made and read: said for every case a host crosses carried, which
     * a manifest is refused where it leaves out.
     *
     * @throws IllegalArgumentException where {@code of} is not a case a host crosses carried
     */
    public CaseCrossing crossing(Case of) {
        return cases.stream().filter(it -> it.of().equals(of)).findFirst()
                .orElseThrow(() -> new IllegalArgumentException(of + " is not a case a host makes"
                        + " or reads through the runtime"));
    }

    /**
     * Whether a host crosses a value of {@code of} carried, through the runtime: a case the language
     * gives, and a primitive a host is handed at all. A declared case is the value as it is, and a
     * primitive no host is handed has no way across however it is held.
     */
    private static boolean crossesCarried(Case of) {
        return switch (of) {
            case Case.Declared d -> false;
            case Case.Language l -> true;
            case Case.Primitive p -> CrossingShape.heldAs(p) != null;
        };
    }

    /** Every case {@code module} names anywhere: in a type, a union answer's cases, a sum's. */
    private static List<Case> casesIn(Module module) {
        List<Type> types = new ArrayList<>();
        List<Case> named = new ArrayList<>();
        for (Behavior behavior : module.behaviors()) {
            types.addAll(behavior.parameters().types());
            types.add(behavior.answers().type());
            if (behavior.answers().union() != null) {
                named.addAll(behavior.answers().union().cases());
            }
        }
        for (Injection injection : module.injections()) {
            injection.parameters().forEach(it -> types.add(it.type()));
            types.add(injection.answers());
        }
        module.values().forEach(it -> types.add(it.type()));
        for (Declaration declaration : module.declarations()) {
            switch (declaration) {
                case Declaration.Product it -> it.fields().forEach(field -> types.add(field.type()));
                case Declaration.Newtype it -> types.add(it.field().type());
                case Declaration.Unit it -> { }
                case Declaration.Sum it -> named.addAll(it.cases());
            }
        }
        for (Type type : types) {
            casesOf(type, named);
        }
        return named;
    }

    private static void casesOf(Type type, List<Case> named) {
        switch (type) {
            case Type.Union it -> named.addAll(it.cases());
            case Type.Option it -> casesOf(it.of(), named);
            case Type.ListOf it -> casesOf(it.of(), named);
            case Type.Primitive it -> { }
            case Type.Declared it -> { }
            case Type.Unrepresented it -> { }
        }
    }

    /** What each module of the library offers a host. */
    public List<Module> modules() {
        return modules;
    }

    /** One word a host hands over or is handed. */
    public enum Word {
        STATUS, INT, BOOL, CASE, OUTCOME, COUNT, MARK, BYTES, VALUE, STRING, DECODED, ISSUE, LIST,
        REQUIREMENTS, CAPABILITY, USERDATA
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
    }

    /**
     * What one module offers a host. It says one list for each way an element crosses, since a
     * list of one element built through two sets of functions is two things said of one list.
     */
    public record Module(String name, List<Behavior> behaviors, List<Construction> constructions,
                         List<Injection> injections,
                         List<PublishedValue> values, List<Declaration> declarations,
                         List<ListCrossing> lists) {

        public Module {
            behaviors = List.copyOf(behaviors);
            constructions = List.copyOf(constructions);
            injections = List.copyOf(injections);
            values = List.copyOf(values);
            declarations = List.copyOf(declarations);
            lists = List.copyOf(lists);
            Set<Element> listed = new HashSet<>();
            for (ListCrossing list : lists) {
                if (!listed.add(list.element())) {
                    throw new IllegalArgumentException("module `" + name + "` says two lists of "
                            + list.element());
                }
            }
        }
    }

    /**
     * What a list whose elements cross as {@code element} is built and read through: a list of one
     * declared type through the same functions as a list of any other. Each function is of the
     * shape a list of that element is: {@code (count, a slice for each word an element crosses as)
     * -> list}, {@code (list) -> count}, and {@code (list, index, room for each word) -> bool}.
     */
    public record ListCrossing(Element element, Function construct, Function length,
                               Function at) {

        public ListCrossing {
            List<Word> words = element.words();
            List<Parameter> built = new ArrayList<>();
            built.add(Parameter.given(Word.COUNT));
            words.forEach(word -> built.add(Parameter.slice(word)));
            List<Parameter> read = new ArrayList<>(
                    List.of(Parameter.given(Word.LIST), Parameter.given(Word.COUNT)));
            words.forEach(word -> read.add(Parameter.room(word)));
            shaped(element, construct, built, Word.LIST);
            shaped(element, length, List.of(Parameter.given(Word.LIST)), Word.COUNT);
            shaped(element, at, read, Word.BOOL);
        }

        private static void shaped(Element element, Function function, List<Parameter> takes,
                                   Word answers) {
            if (!takes.equals(function.takes()) || answers != function.answers()) {
                throw new IllegalArgumentException("a list of " + element + " is built and read"
                        + " through " + function.name() + ", which takes " + function.takes()
                        + " and answers " + function.answers() + " rather than " + takes + " and "
                        + answers);
            }
        }
    }

    /** How an element of a list crosses: one word, or a presence beside one for an optional. */
    public record Element(boolean present, Word word) {

        /** The words an element is handed over as, in order. */
        public List<Word> words() {
            return present ? List.of(Word.BOOL, word) : List.of(word);
        }
    }

    /** A published behavior, and what a host calls it through where it can. */
    public record Behavior(String name, Parameters parameters, Answer answers,
                           @Nullable Function call) {

        /**
         * What tells an answer's cases apart is there exactly where the behavior can be called: a
         * host is handed the answer only by the call, and one told nothing of which case it was
         * handed could name the union and not hold it.
         */
        public Behavior {
            UnionAnswer union = answers.union();
            if (union != null && (union.which() != null) != (call != null)) {
                throw new IllegalArgumentException("it says " + name + " is called "
                        + (call == null ? "no way" : "by " + call.name()) + " and its answer's case"
                        + " is told " + (union.which() == null ? "no way" : "by "
                        + union.which().name()) + ", where the one is there exactly where the"
                        + " other is");
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
     * A behavior a host implements, and what it makes a capability of an implementation of its own
     * through: {@code (room for a capability, room for a souther_hosted, the implementation, what
     * it is handed first)}.
     */
    public record Injection(String name, List<NamedParameter> parameters, Type answers,
                            Implementation implementation, String implement) {

        public Injection {
            parameters = List.copyOf(parameters);
        }
    }

    /** The C type of the function a host implements a behavior as. */
    public record Implementation(String type, List<Parameter> takes, Word answers) {

        public Implementation {
            takes = List.copyOf(takes);
        }
    }

    public record PublishedValue(String name, Type type, @Nullable Function read) {
    }

    /** A published type, with what a host reaches it through. */
    public sealed interface Declaration {

        String name();

        /** Reads a value of it out of text in its external form. */
        @Nullable Function decode();

        /**
         * Reads a value of it out of a value a host built of ordered maps, written with every
         * container as an object: where the reader takes an array, a map keyed by its indices is
         * one.
         */
        @Nullable Function decodeHost();

        @Nullable Function encode();

        record Product(String name, List<Field> fields, @Nullable Function construct,
                       @Nullable Function decode, @Nullable Function decodeHost,
                       @Nullable Function encode) implements Declaration {

            public Product {
                fields = List.copyOf(fields);
            }
        }

        record Newtype(String name, Field field, @Nullable Function construct,
                       @Nullable Function decode, @Nullable Function decodeHost,
                       @Nullable Function encode) implements Declaration {
        }

        record Unit(String name, @Nullable Function construct, @Nullable Function decode,
                    @Nullable Function decodeHost, @Nullable Function encode)
                implements Declaration {
        }

        /** A sum, and the cases {@code which} counts, where a host can be handed each of them. */
        record Sum(String name, List<Case> cases, @Nullable Function which,
                   @Nullable Function decode, @Nullable Function decodeHost,
                   @Nullable Function encode) implements Declaration {

            public Sum {
                cases = oneOrMore(cases, "sum `" + name + "`");
            }
        }
    }

    public record Field(String name, Type type, @Nullable Function read) {
    }

    /** A type as the model says it. */
    public sealed interface Type {

        record Primitive(String name) implements Type {
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

        record ListOf(Type of) implements Type {
        }

        /** A type no host has a representation for yet: a tuple, a function, a set or a map. */
        record Unrepresented(String kind) implements Type {
        }
    }

    /** One case of a sum or of a union. */
    public sealed interface Case {

        /** A declared type, whose value is the case's value as it is. */
        record Declared(String module, String name) implements Case {
        }

        /** A primitive, carried: made and read through {@link Manifest#cases}. */
        record Primitive(String name) implements Case {
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
    }

    /**
     * The manifest at {@code path}.
     *
     * @throws IllegalArgumentException where it is not one this reads, saying where and why
     */
    public static Manifest read(Path path) throws IOException {
        JsonNode read = JsonMapper.builder().build().readTree(Files.readString(path));
        // What it says it is, first and alone: a manifest of another version fails on whichever
        // member moved since, and would say that member is unknown rather than that it is another
        // version.
        Says says = SAYS.decode(read).orElseThrow(issues -> new IllegalArgumentException(
                path + " is not a manifest: " + issues));
        if (!says.format().equals(FORMAT) || says.version() != VERSION || says.abi() != ABI) {
            throw new IllegalArgumentException(path + " is version " + says.version() + " of "
                    + says.format() + " for ABI generation " + says.abi() + ", and this generator"
                    + " reads version " + VERSION + " of " + FORMAT + " for generation " + ABI);
        }
        Result<Manifest> decoded;
        try {
            decoded = MANIFEST.decode(read);
        } catch (IllegalArgumentException broken) {
            // What a part of it holds of itself, refused where the part is made.
            throw new IllegalArgumentException(path + " is not a manifest this generator reads: "
                    + broken.getMessage(), broken);
        }
        return decoded.orElseThrow(issues -> new IllegalArgumentException(
                path + " is not a manifest this generator reads: " + issues));
    }

    /** What a manifest says it is, read past everything else it says. */
    private record Says(String format, int version, int abi) {
    }

    private static final Decoder<JsonNode, Says> SAYS = combine(
            field("format", string()),
            field("version", int_()),
            field("abi", int_())).map(Says::new);

    private static final Decoder<JsonNode, Word> WORD = string().flatMap(written -> {
        for (Word word : Word.values()) {
            if (word.name().toLowerCase(java.util.Locale.ROOT).equals(written)) {
                return Result.ok(word);
            }
        }
        return Result.fail("invalid_value", "no word is spelt " + written);
    });

    private static final Decoder<JsonNode, Parameter> PARAMETER = oneOf(
            strict(field("given", WORD).asDecoder().map(Parameter::given), Set.of("given")),
            strict(field("room", WORD).asDecoder().map(Parameter::room), Set.of("room")),
            strict(field("slice", WORD).asDecoder().map(Parameter::slice), Set.of("slice")));

    private static final Decoder<JsonNode, Function> FUNCTION = combine(
            field("name", string()),
            field("takes", list(PARAMETER)),
            nullableField("answers", WORD)).strict(Function::new);

    private static final Decoder<JsonNode, Case> CASE = oneOf(
            combine(field("kind", literal("declared")), field("module", string()),
                    field("name", string())).strict((kind, module, name) -> new Case.Declared(module, name)),
            combine(field("kind", literal("primitive")), field("name", string()))
                    .strict((kind, name) -> new Case.Primitive(name)),
            combine(field("kind", literal("language")), field("name", string()))
                    .strict((kind, name) -> new Case.Language(name)));

    private static final Decoder<JsonNode, CaseCrossing> CASE_CROSSING = combine(
            field("case", CASE),
            field("make", FUNCTION),
            nullableField("read", FUNCTION)).strict(CaseCrossing::new);

    private static final Decoder<JsonNode, Type> TYPE = lazy(Manifest::type);

    private static Decoder<JsonNode, Type> type() {
        return oneOf(
                combine(field("kind", literal("primitive")), field("name", string()))
                        .strict((kind, name) -> new Type.Primitive(name)),
                combine(field("kind", literal("declared")), field("module", string()),
                        field("name", string()))
                        .strict((kind, module, name) -> new Type.Declared(module, name)),
                combine(field("kind", literal("union")), field("cases", list(CASE)))
                        .strict((kind, cases) -> new Type.Union(cases)),
                combine(field("kind", literal("option")), field("of", TYPE))
                        .strict((kind, of) -> new Type.Option(of)),
                combine(field("kind", literal("tuple")), field("of", list(TYPE)))
                        .strict((kind, of) -> new Type.Unrepresented(kind)),
                combine(field("kind", literal("function")), field("takes", list(TYPE)),
                        field("answers", TYPE))
                        .strict((kind, takes, answers) -> new Type.Unrepresented(kind)),
                combine(field("kind", literal("list")), field("of", TYPE))
                        .strict((kind, of) -> new Type.ListOf(of)),
                combine(field("kind", literal("set")), field("of", TYPE))
                        .strict((kind, of) -> new Type.Unrepresented(kind)),
                combine(field("kind", literal("map")), field("key", TYPE), field("value", TYPE))
                        .strict((kind, key, value) -> new Type.Unrepresented(kind)));
    }

    private static final Decoder<JsonNode, NamedParameter> NAMED_PARAMETER = combine(
            field("name", string()), field("type", TYPE)).strict(NamedParameter::new);

    private static final Decoder<JsonNode, Parameters> PARAMETERS = oneOf(
            strict(field("named", list(NAMED_PARAMETER)).asDecoder()
                    .<Parameters>map(Parameters.Named::new), Set.of("named")),
            strict(field("positional", list(TYPE)).asDecoder()
                    .<Parameters>map(Parameters.Positional::new), Set.of("positional")));

    private static final Decoder<JsonNode, UnionAnswer> UNION_ANSWER = combine(
            field("cases", list(CASE)),
            nullableField("case", FUNCTION)).strict(UnionAnswer::new);

    private static final Decoder<JsonNode, Answer> ANSWER = combine(
            field("type", TYPE),
            nullableField("union", UNION_ANSWER)).strict(Answer::new);

    private static final Decoder<JsonNode, Required> REQUIRED = combine(
            field("module", string()),
            field("name", string())).strict(Required::new);

    private static final Decoder<JsonNode, Behavior> BEHAVIOR = combine(
            field("name", string()),
            field("parameters", PARAMETERS),
            field("answers", ANSWER),
            nullableField("call", FUNCTION)).strict(Behavior::new);

    private static final Decoder<JsonNode, Construction> CONSTRUCTION = combine(
            field("name", string()),
            field("requires", list(REQUIRED)),
            nullableField("bind", FUNCTION)).strict(Construction::new);

    private static final Decoder<JsonNode, Implementation> IMPLEMENTATION = combine(
            field("type", string()),
            field("takes", list(PARAMETER)),
            field("answers", WORD)).strict(Implementation::new);

    private static final Decoder<JsonNode, Injection> INJECTION = combine(
            field("name", string()),
            field("parameters", list(NAMED_PARAMETER)),
            field("answers", TYPE),
            field("implementation", IMPLEMENTATION),
            field("implement", string())).strict(Injection::new);

    private static final Decoder<JsonNode, PublishedValue> VALUE = combine(
            field("name", string()),
            field("type", TYPE),
            nullableField("read", FUNCTION)).strict(PublishedValue::new);

    private static final Decoder<JsonNode, Field> FIELD = combine(
            field("name", string()),
            field("type", TYPE),
            nullableField("read", FUNCTION)).strict(Field::new);

    private static final Decoder<JsonNode, Declaration> DECLARATION = oneOf(
            combine(field("kind", literal("product")), field("name", string()),
                    field("fields", list(FIELD)), nullableField("construct", FUNCTION),
                    nullableField("decode", FUNCTION), nullableField("decodehost", FUNCTION),
                    nullableField("encode", FUNCTION))
                    .strict((kind, name, fields, construct, decode, decodeHost, encode) ->
                            new Declaration.Product(name, fields, construct, decode, decodeHost,
                                    encode)),
            combine(field("kind", literal("newtype")), field("name", string()),
                    field("field", FIELD), nullableField("construct", FUNCTION),
                    nullableField("decode", FUNCTION), nullableField("decodehost", FUNCTION),
                    nullableField("encode", FUNCTION))
                    .strict((kind, name, held, construct, decode, decodeHost, encode) ->
                            new Declaration.Newtype(name, held, construct, decode, decodeHost,
                                    encode)),
            combine(field("kind", literal("unit")), field("name", string()),
                    nullableField("construct", FUNCTION), nullableField("decode", FUNCTION),
                    nullableField("decodehost", FUNCTION), nullableField("encode", FUNCTION))
                    .strict((kind, name, construct, decode, decodeHost, encode) ->
                            new Declaration.Unit(name, construct, decode, decodeHost, encode)),
            combine(field("kind", literal("sum")), field("name", string()),
                    field("cases", list(CASE)), nullableField("case", FUNCTION),
                    nullableField("decode", FUNCTION), nullableField("decodehost", FUNCTION),
                    nullableField("encode", FUNCTION))
                    .strict((kind, name, cases, which, decode, decodeHost, encode) ->
                            new Declaration.Sum(name, cases, which, decode, decodeHost,
                                    encode)));

    private static final Decoder<JsonNode, Element> ELEMENT = oneOf(
            strict(field("whole", WORD).asDecoder().map(word -> new Element(false, word)),
                    Set.of("whole")),
            strict(field("present", WORD).asDecoder().map(word -> new Element(true, word)),
                    Set.of("present")));

    private static final Decoder<JsonNode, ListCrossing> LIST_CROSSING = combine(
            field("element", ELEMENT),
            field("construct", FUNCTION),
            field("length", FUNCTION),
            field("at", FUNCTION)).strict(ListCrossing::new);

    private static final Decoder<JsonNode, Module> MODULE = combine(
            field("name", string()),
            field("behaviors", list(BEHAVIOR)),
            field("constructions", list(CONSTRUCTION)),
            field("injections", list(INJECTION)),
            field("values", list(VALUE)),
            field("declarations", list(DECLARATION)),
            field("lists", list(LIST_CROSSING))).strict(Module::new);

    private static final Decoder<JsonNode, Manifest> MANIFEST = combine(
            field("format", string()),
            field("version", int_()),
            field("abi", int_()),
            field("statuses", map(int_())),
            field("outcomes", map(int_())),
            field("runtime", list(FUNCTION)),
            field("cases", list(CASE_CROSSING)),
            field("modules", list(MODULE)))
            .strict((format, version, abi, statuses, outcomes, runtime, cases, modules) ->
                    new Manifest(abi, statuses, outcomes, runtime, cases, modules));
}
