package souther.nativecode.php;

import net.unit8.raoh.Result;
import net.unit8.raoh.decode.Decoder;
import org.jspecify.annotations.Nullable;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
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
 * What a manifest says, as this generator reads it: version 6 of {@code souther-native-interface},
 * and nothing else.
 *
 * <p>Read strictly, as the driver writes it. A member this does not name, or a version or ABI
 * generation it was not written for, is refused rather than read as much of as happens to parse:
 * a binding generated from a manifest that says more than was understood of it would call
 * functions as something they are not.
 */
record Manifest(int abi, Map<String, Integer> statuses, Map<String, Integer> outcomes,
                List<Module> modules) {

    /** What a manifest says it is. */
    static final String FORMAT = "souther-native-interface";

    /** The version of what a manifest says that this reads. */
    static final int VERSION = 7;

    /** The ABI generation the functions this binds answer to. */
    static final int ABI = 4;

    /** One word a host hands over or is handed. */
    enum Word {
        STATUS, INT, BOOL, CASE, OUTCOME, COUNT, MARK, BYTES, VALUE, STRING, DECODED, ISSUE, LIST,
        REQUIREMENTS, CAPABILITY, USERDATA
    }

    /**
     * One parameter of a function: a word handed over, room the function writes one through, or as
     * many of a word as another parameter counts, which the function reads.
     */
    record Parameter(Mode mode, Word word) {

        enum Mode { GIVEN, ROOM, SLICE }

        static Parameter given(Word word) {
            return new Parameter(Mode.GIVEN, word);
        }

        static Parameter room(Word word) {
            return new Parameter(Mode.ROOM, word);
        }

        static Parameter slice(Word word) {
            return new Parameter(Mode.SLICE, word);
        }
    }

    /** A function the library defines, by its symbol. */
    record Function(String name, List<Parameter> takes, @Nullable Word answers) {
    }

    record Module(String name, List<Behavior> behaviors, List<Injection> injections,
                  List<PublishedValue> values, List<Declaration> declarations,
                  List<ListCrossing> lists) {
    }

    /**
     * What a list whose elements cross as {@code element} is built and read through: a list of one
     * declared type through the same functions as a list of any other.
     */
    record ListCrossing(Element element, Function construct, Function length, Function at) {
    }

    /** How an element of a list crosses: one word, or a presence beside one for an optional. */
    record Element(boolean present, Word word) {
    }

    /**
     * A published behavior, what constructing it requires injected, what a host calls it through
     * where it can, and what a host makes a capability of it through where something may require it.
     */
    record Behavior(String name, Parameters parameters, Answer answers, List<Required> requires,
                    @Nullable Function call, @Nullable Function bind) {
    }

    /** A behavior another requires injected, by its module and its name. */
    record Required(String module, String name) {

        /** The module and the name joined the one way, which no two behaviors share. */
        String key() {
            return module + "." + name;
        }
    }

    /**
     * What a behavior answers, and where that is a union no declaration names, what a host tells
     * its cases apart by.
     */
    record Answer(Type type, @Nullable UnionAnswer union) {
    }

    /**
     * The cases a union a behavior answers descends to, a member that is a sum as its own cases,
     * and what says which of them a value is, in the order they are listed.
     */
    record UnionAnswer(List<Case> cases, @Nullable Function which) {
    }

    /** What a behavior takes: named as its declaration names them, or in order for a composition. */
    sealed interface Parameters {

        /** Each type a behavior takes, in order, with or without the name it is declared under. */
        List<Type> types();

        record Named(List<NamedParameter> parameters) implements Parameters {
            @Override
            public List<Type> types() {
                return parameters.stream().map(NamedParameter::type).toList();
            }
        }

        record Positional(List<Type> types) implements Parameters {
        }
    }

    record NamedParameter(String name, Type type) {
    }

    /**
     * A behavior a host implements, and what it makes a capability of an implementation of its own
     * through: {@code (room for a capability, room for a souther_hosted, the implementation, what
     * it is handed first)}.
     */
    record Injection(String name, List<NamedParameter> parameters, Type answers,
                     Implementation implementation, String implement) {
    }

    /** The C type of the function a host implements a behavior as. */
    record Implementation(String type, List<Parameter> takes, Word answers) {
    }

    record PublishedValue(String name, Type type, @Nullable Function read) {
    }

    /** A published type, with what a host reaches it through. */
    sealed interface Declaration {

        String name();

        @Nullable Function decode();

        @Nullable Function encode();

        record Product(String name, List<Field> fields, @Nullable Function construct,
                       @Nullable Function decode, @Nullable Function encode) implements Declaration {
        }

        record Newtype(String name, Field field, @Nullable Function construct,
                       @Nullable Function decode, @Nullable Function encode) implements Declaration {
        }

        record Unit(String name, @Nullable Function construct, @Nullable Function decode,
                    @Nullable Function encode) implements Declaration {
        }

        /** A sum, and the cases {@code which} counts, where every case is a declared type. */
        record Sum(String name, List<Case> cases, @Nullable Function which,
                   @Nullable Function decode, @Nullable Function encode) implements Declaration {
        }
    }

    record Field(String name, Type type, @Nullable Function read) {
    }

    /** A type as the model says it. */
    sealed interface Type {

        record Primitive(String name) implements Type {
        }

        record Declared(String module, String name) implements Type {
        }

        record Union(List<Case> cases) implements Type {
        }

        record Option(Type of) implements Type {
        }

        record ListOf(Type of) implements Type {
        }

        /** A type no host has a representation for yet: a tuple, a function, a set or a map. */
        record Unrepresented(String kind) implements Type {
        }
    }

    /** One case of a sum. */
    sealed interface Case {

        record Declared(String module, String name) implements Case {
        }

        /** A primitive or a case the language gives, which a host reaches nothing of. */
        record Other(String kind, String name) implements Case {
        }
    }

    /**
     * The manifest at {@code path}.
     *
     * @throws IllegalArgumentException where it is not one this reads, saying where and why
     */
    static Manifest read(Path path) throws IOException {
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
        return MANIFEST.decode(read).orElseThrow(issues -> new IllegalArgumentException(
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
                    .strict(Case.Other::new),
            combine(field("kind", literal("language")), field("name", string()))
                    .strict(Case.Other::new));

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
            field("requires", list(REQUIRED)),
            nullableField("call", FUNCTION),
            nullableField("bind", FUNCTION)).strict(Behavior::new);

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
                    nullableField("decode", FUNCTION), nullableField("encode", FUNCTION))
                    .strict((kind, name, fields, construct, decode, encode) ->
                            new Declaration.Product(name, fields, construct, decode, encode)),
            combine(field("kind", literal("newtype")), field("name", string()),
                    field("field", FIELD), nullableField("construct", FUNCTION),
                    nullableField("decode", FUNCTION), nullableField("encode", FUNCTION))
                    .strict((kind, name, held, construct, decode, encode) ->
                            new Declaration.Newtype(name, held, construct, decode, encode)),
            combine(field("kind", literal("unit")), field("name", string()),
                    nullableField("construct", FUNCTION), nullableField("decode", FUNCTION),
                    nullableField("encode", FUNCTION))
                    .strict((kind, name, construct, decode, encode) ->
                            new Declaration.Unit(name, construct, decode, encode)),
            combine(field("kind", literal("sum")), field("name", string()),
                    field("cases", list(CASE)), nullableField("case", FUNCTION),
                    nullableField("decode", FUNCTION), nullableField("encode", FUNCTION))
                    .strict((kind, name, cases, which, decode, encode) ->
                            new Declaration.Sum(name, cases, which, decode, encode)));

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
            field("modules", list(MODULE)))
            .strict((format, version, abi, statuses, outcomes, runtime, modules) ->
                    new Manifest(abi, statuses, outcomes, modules));
}
