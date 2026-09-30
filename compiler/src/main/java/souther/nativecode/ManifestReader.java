package souther.nativecode;

import net.unit8.raoh.Result;
import net.unit8.raoh.decode.Decoder;
import souther.bindings.Manifest;
import souther.bindings.Manifest.*;
import souther.bindings.ValueCrossing;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
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
 * The manifest the driver writes beside a library, read into the model a generator is handed
 * ({@link Manifest}): version {@value #VERSION} of {@value #FORMAT}, at ABI generation
 * {@value #ABI}, and nothing else.
 *
 * <p>How the manifest is written is between the driver and this command, and no generator's: the
 * two are released together and read one version, exactly. Read strictly, as the driver writes it.
 * A member this does not name, or a version or ABI generation it was not written for, is refused
 * rather than read as much of as happens to parse: a binding generated from a manifest that says
 * more than was understood of it would call functions as something they are not.
 */
public final class ManifestReader {

    /** What a manifest says it is. */
    public static final String FORMAT = "souther-native-interface";

    /** The version of what a manifest says that this reads. */
    public static final int VERSION = 15;

    /** The ABI generation the functions a manifest names answer to, which this reads. */
    public static final int ABI = 9;

    private ManifestReader() {
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

    private static final Decoder<JsonNode, Manifest.Primitive> PRIMITIVE = string().flatMap(written -> {
        for (Manifest.Primitive primitive : Manifest.Primitive.values()) {
            if (primitive.spelt().equals(written)) {
                return Result.ok(primitive);
            }
        }
        return Result.fail("invalid_value", "no primitive is spelt " + written);
    });

    private static final Decoder<JsonNode, Parameter> PARAMETER = oneOf(
            strict(field("given", WORD).asDecoder().map(Parameter::given), Set.of("given")),
            strict(field("room", WORD).asDecoder().map(Parameter::room), Set.of("room")),
            strict(field("slice", WORD).asDecoder().map(Parameter::slice), Set.of("slice")));

    private static final Decoder<JsonNode, Function> FUNCTION = combine(
            field("name", string()),
            field("takes", list(PARAMETER)),
            nullableField("answers", WORD)).strict(Function::new);

    /** A case a declaration names: what every case of a sum is, since the language says so. */
    private static final Decoder<JsonNode, Case.Declared> DECLARED_CASE =
            combine(field("kind", literal("declared")), field("module", string()),
                    field("name", string())).strict((kind, module, name) -> new Case.Declared(module, name));

    private static final Decoder<JsonNode, Case> CASE = oneOf(
            DECLARED_CASE.map(it -> it),
            combine(field("kind", literal("primitive")), field("name", PRIMITIVE))
                    .strict((kind, name) -> new Case.Primitive(name)),
            combine(field("kind", literal("language")), field("name", string()))
                    .strict((kind, name) -> new Case.Language(name)));

    private static final Decoder<JsonNode, CaseCrossing> CASE_CROSSING = combine(
            field("case", CASE),
            field("make", FUNCTION),
            nullableField("read", FUNCTION)).strict(CaseCrossing::new);

    private static final Decoder<JsonNode, Type> TYPE = lazy(ManifestReader::type);

    private static Decoder<JsonNode, Type> type() {
        return oneOf(
                combine(field("kind", literal("primitive")), field("name", PRIMITIVE))
                        .strict((kind, name) -> new Type.Primitive(name)),
                combine(field("kind", literal("declared")), field("module", string()),
                        field("name", string()))
                        .strict((kind, module, name) -> new Type.Declared(module, name)),
                combine(field("kind", literal("union")), field("cases", list(CASE)))
                        .strict((kind, cases) -> new Type.Union(cases)),
                combine(field("kind", literal("option")), field("of", TYPE))
                        .strict((kind, of) -> new Type.Option(of)),
                combine(field("kind", literal("tuple")), field("of", list(TYPE)))
                        .strict((kind, of) -> new Type.Tuple(of)),
                combine(field("kind", literal("function")), field("takes", list(TYPE)),
                        field("answers", TYPE))
                        .strict((kind, takes, answers) -> new Type.Function(takes, answers)),
                combine(field("kind", literal("list")), field("of", TYPE))
                        .strict((kind, of) -> new Type.ListOf(of)),
                combine(field("kind", literal("set")), field("of", TYPE))
                        .strict((kind, of) -> new Type.SetOf(of)),
                combine(field("kind", literal("map")), field("key", TYPE), field("value", TYPE))
                        .strict((kind, key, value) -> new Type.MapOf(key, value)),
                strict(field("kind", literal("nothing")).asDecoder()
                        .<Type>map(kind -> new Type.Nothing()), Set.of("kind")),
                strict(field("kind", literal("never")).asDecoder()
                        .<Type>map(kind -> new Type.Never()), Set.of("kind")));
    }

    private static final Decoder<JsonNode, Shape> SHAPE = lazy(ManifestReader::shape);

    private static final Decoder<JsonNode, Signature> SIGNATURE = lazy(() -> combine(
            field("takes", list(SHAPE)),
            field("answers", SHAPE)).strict(Signature::new));

    /** A leaf's word, which is one of {@link Shape.Leaf#WORDS}: what a value is handed over whole as. */
    private static final Decoder<JsonNode, Word> LEAF = WORD.flatMap(word ->
            Shape.Leaf.WORDS.contains(word)
                    ? Result.ok(word)
                    : Result.fail("invalid_value", word + " is not a value handed over whole"));

    private static Decoder<JsonNode, Shape> shape() {
        return oneOf(
                strict(field("leaf", LEAF).asDecoder().<Shape>map(Shape.Leaf::new), Set.of("leaf")),
                strict(field("option", SHAPE).asDecoder().<Shape>map(Shape.Option::new),
                        Set.of("option")),
                strict(field("product", list(SHAPE)).asDecoder().<Shape>map(Shape.Product::new),
                        Set.of("product")),
                strict(field("list", SHAPE).asDecoder().<Shape>map(Shape.ListOf::new),
                        Set.of("list")),
                strict(field("function", SIGNATURE).asDecoder().<Shape>map(Shape.FunctionOf::new),
                        Set.of("function")));
    }

    /**
     * Why nothing reaches a value, as the driver writes it: what stands in the way, and where in
     * what a function hands over or is handed. Read strictly, as everything is, and kept by no
     * generator: the model says only that nothing reaches it.
     */
    private record Refusal(String reason, List<Object> path) {
    }

    private static final Set<String> REASONS = Set.of("no_representation", "no_value",
            "no_discriminator");

    private static final Decoder<JsonNode, String> REASON = string().flatMap(written ->
            REASONS.contains(written) ? Result.ok(written)
                    : Result.fail("invalid_value", "no reason is spelt " + written));

    private static final Decoder<JsonNode, Object> STEP = oneOf(
            literal("answers").<Object>map(it -> it),
            literal("option").<Object>map(it -> it),
            literal("element").<Object>map(it -> it),
            strict(field("takes", int_()).asDecoder().<Object>map(it -> it), Set.of("takes")),
            strict(field("member", int_()).asDecoder().<Object>map(it -> it), Set.of("member")),
            strict(field("field", string()).asDecoder().<Object>map(it -> it), Set.of("field")));

    private static final Decoder<JsonNode, Refusal> REFUSAL = combine(
            field("reason", REASON),
            field("path", list(STEP))).strict(Refusal::new);

    /** What reaches a value as {@code of} reads it, or why nothing does. */
    private static <T> Decoder<JsonNode, Reach<T>> reach(Decoder<JsonNode, T> of) {
        return oneOf(
                strict(field("available", of).asDecoder()
                        .<Reach<T>>map(Reach.Available::new), Set.of("available")),
                strict(field("unavailable", REFUSAL).asDecoder()
                        .<Reach<T>>map(it -> new Reach.Unavailable<>()), Set.of("unavailable")));
    }

    /** A call as the manifest writes it: the function, and the shapes beside it. */
    private record RawCall(Function function, Signature signature) {
    }

    private record RawConstruct(Function function, List<Shape> takes) {
    }

    private record RawRead(Function function, Shape answers) {
    }

    private static final Decoder<JsonNode, RawCall> CALL = combine(
            field("function", FUNCTION),
            field("signature", SIGNATURE)).strict(RawCall::new);

    private static final Decoder<JsonNode, RawConstruct> CONSTRUCT = combine(
            field("function", FUNCTION),
            field("takes", list(SHAPE))).strict(RawConstruct::new);

    private static final Decoder<JsonNode, RawRead> READ = combine(
            field("function", FUNCTION),
            field("answers", SHAPE)).strict(RawRead::new);

    /** {@code reach} with what reaches the value made by {@code made}, and why nothing does kept. */
    private static <A, B> Reach<B> mapped(Reach<A> reach, java.util.function.Function<A, B> made) {
        return switch (reach) {
            case Reach.Available<A>(A it) -> new Reach.Available<>(made.apply(it));
            case Reach.Unavailable<A> it -> new Reach.Unavailable<>();
        };
    }

    /**
     * What a module says each value that crosses its functions is, taken apart with the shape it
     * crosses in: the one place a type and a shape are paired, so a generator is handed the pair
     * and never pairs them itself.
     *
     * <p>A leaf pairs with any word, since which word a type crosses as is the driver's to choose;
     * anything else that does not pair — a tuple's count, an option beside what is none, a list
     * or a function value the module says nothing to reach — is the driver and this command
     * disagreeing, and the manifest is refused.
     */
    private record Zip(String module, List<ListCrossing> lists, List<FunctionCrossing> functions) {

        ValueCrossing of(Type type, Shape shape) {
            return switch (shape) {
                case Shape.Leaf leaf -> switch (type) {
                    case Type.Primitive it -> new ValueCrossing.Primitive(it, leaf.word());
                    case Type.Declared it -> new ValueCrossing.Handle(it, leaf.word());
                    case Type.Union it -> new ValueCrossing.Union(it, leaf.word());
                    default -> throw disagreeing(type, shape);
                };
                case Shape.Option option -> {
                    if (!(type instanceof Type.Option it)) {
                        throw disagreeing(type, shape);
                    }
                    yield new ValueCrossing.Optional(it, of(it.of(), option.of()));
                }
                case Shape.Product product -> {
                    if (!(type instanceof Type.Tuple it)) {
                        throw disagreeing(type, shape);
                    }
                    yield new ValueCrossing.Tuple(it, all(it.of(), product.of()));
                }
                case Shape.ListOf list -> {
                    Type listed = Type.listed(type);
                    if (listed == null) {
                        throw disagreeing(type, shape);
                    }
                    ListCrossing crossing = lists.stream()
                            .filter(it -> it.element().equals(list.element())).findFirst()
                            .orElseThrow(() -> new IllegalArgumentException("module `" + module
                                    + "` hands a list of " + list.element() + " across and says"
                                    + " nothing to reach one through"));
                    yield new ValueCrossing.Listed(type, of(listed, list.element()), crossing);
                }
                case Shape.FunctionOf function -> {
                    if (!(type instanceof Type.Function it)) {
                        throw disagreeing(type, shape);
                    }
                    FunctionCrossing crossing = functions.stream()
                            .filter(f -> f.signature().equals(function.signature())).findFirst()
                            .orElseThrow(() -> new IllegalArgumentException("module `" + module
                                    + "` hands a function of " + function.signature()
                                    + " across and says nothing to reach one through"));
                    yield new ValueCrossing.FunctionValue(it,
                            all(it.takes(), function.signature().takes()),
                            of(it.answers(), function.signature().answers()), crossing);
                }
            };
        }

        List<ValueCrossing> all(List<Type> types, List<Shape> shapes) {
            if (types.size() != shapes.size()) {
                throw new IllegalArgumentException("module `" + module + "` says " + types.size()
                        + " values cross as " + shapes.size() + " shapes: " + types + " as "
                        + shapes);
            }
            List<ValueCrossing> zipped = new java.util.ArrayList<>();
            for (int at = 0; at < types.size(); at++) {
                zipped.add(of(types.get(at), shapes.get(at)));
            }
            return zipped;
        }

        Crossings crossings(List<Type> takes, Type answers, Signature signature) {
            return new Crossings(all(takes, signature.takes()), of(answers, signature.answers()));
        }

        Call call(RawCall raw, List<Type> takes, Type answers) {
            return new Call(raw.function(), crossings(takes, answers, raw.signature()));
        }

        private IllegalArgumentException disagreeing(Type type, Shape shape) {
            return new IllegalArgumentException("module `" + module + "` says " + type
                    + " crosses in " + shape + ", which no value of it is");
        }
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

    private static Decoder<JsonNode, Behavior> behavior(Zip zip) {
        return combine(
                field("name", string()),
                field("parameters", PARAMETERS),
                field("answers", ANSWER),
                field("call", reach(CALL))).strict((name, parameters, answers, call) ->
                new Behavior(name, parameters, answers, mapped(call,
                        raw -> zip.call(raw, parameters.types(), answers.type()))));
    }

    private static final Decoder<JsonNode, Construction> CONSTRUCTION = combine(
            field("name", string()),
            field("requires", list(REQUIRED)),
            nullableField("bind", FUNCTION)).strict(Construction::new);

    private static final Decoder<JsonNode, Implementation> IMPLEMENTATION = combine(
            field("type", string()),
            field("takes", list(PARAMETER)),
            field("answers", WORD)).strict(Implementation::new);

    private static Decoder<JsonNode, Injection> injection(Zip zip) {
        return combine(
                field("name", string()),
                field("parameters", list(NAMED_PARAMETER)),
                field("answers", TYPE),
                field("signature", SIGNATURE),
                field("implementation", IMPLEMENTATION),
                field("implement", string()))
                .strict((name, parameters, answers, signature, implementation, implement) ->
                        new Injection(name, parameters, answers, zip.crossings(
                                parameters.stream().map(NamedParameter::type).toList(), answers,
                                signature), implementation, implement));
    }

    private static Decoder<JsonNode, PublishedValue> value(Zip zip) {
        return combine(
                field("name", string()),
                field("type", TYPE),
                field("read", reach(CALL))).strict((name, type, read) ->
                new PublishedValue(name, type, mapped(read, raw -> zip.call(raw, List.of(), type))));
    }

    private static Decoder<JsonNode, Field> fieldOf(Zip zip) {
        return combine(
                field("name", string()),
                field("type", TYPE),
                field("read", reach(READ))).strict((name, type, read) ->
                new Field(name, type, mapped(read,
                        raw -> new Read(raw.function(), zip.of(type, raw.answers())))));
    }

    /** What constructs a value of a declaration of {@code fields}, each field's crossing paired. */
    private static Reach<Construct> built(Zip zip, List<Field> fields, Reach<RawConstruct> raw) {
        return mapped(raw, it -> new Construct(it.function(),
                zip.all(fields.stream().map(Field::type).toList(), it.takes())));
    }

    private static Decoder<JsonNode, Declaration> declaration(Zip zip) {
        return oneOf(
            combine(field("kind", literal("product")), field("name", string()),
                    field("fields", list(fieldOf(zip))), field("construct", reach(CONSTRUCT)),
                    nullableField("decode", FUNCTION), nullableField("decodehost", FUNCTION),
                    nullableField("encode", FUNCTION))
                    .strict((kind, name, fields, construct, decode, decodeHost, encode) ->
                            new Declaration.Product(name, fields, built(zip, fields, construct), decode, decodeHost,
                                    encode)),
            combine(field("kind", literal("newtype")), field("name", string()),
                    field("field", fieldOf(zip)), field("construct", reach(CONSTRUCT)),
                    nullableField("decode", FUNCTION), nullableField("decodehost", FUNCTION),
                    nullableField("encode", FUNCTION))
                    .strict((kind, name, held, construct, decode, decodeHost, encode) ->
                            new Declaration.Newtype(name, held, built(zip, List.of(held), construct), decode, decodeHost,
                                    encode)),
            combine(field("kind", literal("unit")), field("name", string()),
                    field("construct", reach(CONSTRUCT)), nullableField("decode", FUNCTION),
                    nullableField("decodehost", FUNCTION), nullableField("encode", FUNCTION))
                    .strict((kind, name, construct, decode, decodeHost, encode) ->
                            new Declaration.Unit(name, built(zip, List.of(), construct), decode, decodeHost, encode)),
            combine(field("kind", literal("sum")), field("name", string()),
                    field("cases", list(DECLARED_CASE)), nullableField("case", FUNCTION),
                    nullableField("decode", FUNCTION), nullableField("decodehost", FUNCTION),
                    nullableField("encode", FUNCTION))
                    .strict((kind, name, cases, which, decode, decodeHost, encode) ->
                            new Declaration.Sum(name, cases, which, decode, decodeHost,
                                    encode)));
    }

    private static final Decoder<JsonNode, ListRead> LIST_READ = combine(
            field("length", FUNCTION),
            field("at", FUNCTION)).strict(ListRead::new);

    private static final Decoder<JsonNode, ListCrossing> LIST_CROSSING = combine(
            field("element", SHAPE),
            nullableField("construct", FUNCTION),
            nullableField("read", LIST_READ)).strict(ListCrossing::new);

    private static final Decoder<JsonNode, FunctionMaking> FUNCTION_MAKING = combine(
            field("implementation", IMPLEMENTATION),
            field("implement", string())).strict(FunctionMaking::new);

    private static final Decoder<JsonNode, FunctionCrossing> FUNCTION_CROSSING = combine(
            field("signature", SIGNATURE),
            nullableField("call", FUNCTION),
            nullableField("make", FUNCTION_MAKING)).strict(FunctionCrossing::new);

    /**
     * A module, in two steps: what it says each list and function value is reached through, and
     * then everything else, each value that crosses paired with its shape against those.
     */
    private static final Decoder<JsonNode, Manifest.Module> MODULE = (node, path) ->
            combine(field("name", string()),
                    field("lists", list(LIST_CROSSING)),
                    field("functions", list(FUNCTION_CROSSING))).map(Zip::new).decode(node, path)
                    .flatMap(zip -> combine(
                            field("name", string()),
                            field("behaviors", list(behavior(zip))),
                            field("constructions", list(CONSTRUCTION)),
                            field("injections", list(injection(zip))),
                            field("values", list(value(zip))),
                            field("declarations", list(declaration(zip))),
                            field("lists", list(LIST_CROSSING)),
                            field("functions", list(FUNCTION_CROSSING)))
                            .strict(Manifest.Module::new).decode(node, path));

    private static final Decoder<JsonNode, Manifest> MANIFEST = combine(
            field("format", string()),
            field("version", int_()),
            field("abi", int_()),
            field("statuses", map(int_())),
            field("outcomes", map(int_())),
            field("cases", list(CASE_CROSSING)),
            field("modules", list(MODULE)))
            .strict((format, version, abi, statuses, outcomes, cases, modules) ->
                    Manifest.of(statuses, outcomes, cases, modules));
}
