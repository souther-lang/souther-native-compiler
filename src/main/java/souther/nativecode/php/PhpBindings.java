package souther.nativecode.php;

import org.jspecify.annotations.Nullable;
import souther.nativecode.NativeCompiler;
import souther.nativecode.php.Crossing.Both;
import souther.nativecode.php.Crossing.Given;
import souther.nativecode.php.Crossing.Listed;
import souther.nativecode.php.Crossing.OneOf;
import souther.nativecode.php.Crossing.Present;
import souther.nativecode.php.Crossing.Received;
import souther.nativecode.php.Crossing.Single;
import souther.nativecode.php.Crossing.Told;
import souther.nativecode.php.Crossing.Whole;
import souther.nativecode.php.Manifest.Case;
import souther.nativecode.php.Manifest.Declaration;
import souther.nativecode.php.Manifest.Function;
import souther.nativecode.php.Manifest.Parameter;
import souther.nativecode.php.Manifest.Type;
import souther.nativecode.php.Manifest.Word;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.stream.Collectors;

/**
 * Writes the PHP a host calls a library through, from the manifest the build wrote beside it.
 *
 * <p>Reads the manifest and nothing else: not the checked program, and nothing of how a value is
 * laid out. What it writes is the manifest's surface as PHP types — a class for each published
 * type, an interface for each sum, a static function for each behavior — over the runtime package
 * in {@code bindings/php/runtime}, which is where a run's arena, a value's lifetime and what each
 * status means are kept. The FFI declarations are the ones the build wrote, copied beside it.
 *
 * <p>What a host has no way to reach is not written: a behavior the manifest gives no {@code call},
 * a field with no {@code read}, a type no host has a representation for. A function that could not
 * be called would be one a caller finds out about by calling it.
 */
public final class PhpBindings {

    /** What a binding is written as: the directory its namespace stands in, and every file in it. */
    public record Generated(Path root, List<Path> files) {
    }

    /** A model a PHP binding cannot be generated for as it stands, and why. */
    public static final class NotBindable extends IllegalArgumentException {
        NotBindable(String why) {
            super(why);
        }
    }

    /** Where the declarations are copied to, beside the binding that loads them. */
    static final String DECLARATIONS = "souther.ffi.h";

    private static final String RUNTIME = "\\Souther\\Runtime\\";

    /**
     * The version of what generated code calls of the runtime package that this writes against:
     * {@code Binding::PROTOCOL} in {@code bindings/php/runtime}, which a test holds to this.
     */
    static final int RUNTIME_PROTOCOL = 2;

    private final Manifest manifest;
    private final String root;
    private final Path into;
    private final List<Path> written = new ArrayList<>();

    /** What each declared type is, by {@code module.Name}. */
    private final Map<String, Declared> declared = new LinkedHashMap<>();

    /**
     * What a list is built and read through, by how its element crosses. The first module's that
     * has one: every module's for one way an element crosses does the same to the same list.
     */
    private final Map<Manifest.Element, Manifest.ListCrossing> lists = new LinkedHashMap<>();

    private PhpBindings(Manifest manifest, String root, Path into) {
        this.manifest = manifest;
        this.root = root;
        this.into = into;
    }

    /**
     * Writes the binding of {@code library} into {@code into}, under the namespace {@code namespace}.
     *
     * <p>The namespace is the binding's own, and not read off the model, so that two libraries
     * publishing a module of the same name can stand in one application.
     *
     * @throws NotBindable where a name in the model is not one PHP takes
     */
    public static Generated generate(NativeCompiler.Library library, Path into, String namespace)
            throws IOException {
        return generate(library.manifest(), library.declarations(), into, namespace);
    }

    /**
     * Refuses what a generation into {@code into} under {@code namespace} would refuse whatever the
     * manifest said: a namespace PHP will not take, and a directory holding what no generation
     * wrote. For a caller that builds the library in the same step, so that a binding it was never
     * going to write is refused before the library is.
     *
     * @throws NotBindable where the namespace or the directory would be refused
     */
    public static void refuseAhead(Path into, String namespace) throws IOException {
        PhpNames.rootNamespace(namespace);
        Output.replaceable(into);
    }

    /**
     * Writes the binding of what {@code manifest} describes into {@code into}, which is then that
     * binding and nothing else: it is written beside it and put in place whole ({@link Output}), so
     * a class the model no longer declares does not survive a generation, and a refused one leaves
     * what was there as it was.
     */
    static Generated generate(Path manifest, Path declarations, Path into, String namespace)
            throws IOException {
        Manifest read = Manifest.read(manifest);
        String root = PhpNames.rootNamespace(namespace);
        Output output = Output.replacing(into);
        PhpBindings binding = new PhpBindings(read, root, output.staging());
        try {
            binding.write();
            Path copied = output.staging().resolve(DECLARATIONS);
            Files.copy(declarations, copied);
            binding.written.add(copied);
            output.commit();
        } catch (IOException | RuntimeException e) {
            output.abandon();
            throw e;
        }
        return new Generated(output.placed(output.staging()),
                binding.written.stream().map(output::placed).toList());
    }

    /** A declared type, and what PHP calls what is generated for it. */
    private record Declared(String module, Declaration declaration, String namespace, String name) {

        String fqcn() {
            return "\\" + namespace + "\\" + name;
        }

        String codec() {
            return fqcn() + "Codec";
        }

        String opaque() {
            return fqcn() + "Value";
        }

        String key() {
            return module + "." + declaration.name();
        }
    }

    private void write() throws IOException {
        PhpNames.Claimed namespaces = PhpNames.Claimed.classes("namespace " + root);
        for (Manifest.Module module : manifest.modules()) {
            String namespace = PhpNames.moduleNamespace(root, module.name());
            namespaces.claim(namespace.substring(root.length() + 1), "module `" + module.name() + "`");
            for (Declaration declaration : module.declarations()) {
                Declared it = new Declared(module.name(), declaration, namespace,
                        PhpNames.typeName(declaration.name(),
                                "type `" + module.name() + "." + declaration.name() + "`"));
                declared.put(it.key(), it);
            }
            for (Manifest.ListCrossing list : module.lists()) {
                lists.putIfAbsent(list.element(), list);
            }
        }
        for (Manifest.Module module : manifest.modules()) {
            module(module);
        }
        binding();
        autoload();
    }

    // ---------------------------------------------------------------------------------------------
    // What a model type crosses as.

    /**
     * How a value of {@code type} crosses as one word, or null where it does not: the same both
     * ways.
     */
    private @Nullable Single single(Type type) {
        return type instanceof Type.ListOf list ? listed(list) : whole(type);
    }

    /**
     * How a value of {@code type} crosses as one word or an optional of one, or null where it does
     * not: the same both ways.
     */
    private @Nullable Both both(Type type) {
        if (type instanceof Type.Option option) {
            Single of = single(option.of());
            return of == null ? null : new Present(of);
        }
        return single(type);
    }

    /**
     * How a list crosses, where its element crosses both ways and the library defines what a list
     * of such elements is built and read through.
     *
     * <p>Both ways even where a list is only handed one way, since an element of either is the same
     * words: a union no declaration names is refused as an element, having no way to say which
     * case one read out of a list is.
     */
    private @Nullable Listed listed(Type.ListOf list) {
        Both element = both(list.of());
        if (element == null) {
            return null;
        }
        Manifest.Element shape = switch (element) {
            case Present present -> new Manifest.Element(true, present.of().word());
            case Single single -> new Manifest.Element(false, single.word());
        };
        Manifest.ListCrossing crossing = lists.get(shape);
        if (crossing == null) {
            return null;
        }
        List<Parameter> built = new ArrayList<>();
        built.add(Parameter.given(Word.COUNT));
        element.words().forEach(word -> built.add(Parameter.slice(word)));
        agrees(crossing.construct(), built, Word.LIST);
        agrees(crossing.length(), List.of(Word.LIST), List.of(), Word.COUNT);
        agrees(crossing.at(), List.of(Word.LIST, Word.COUNT), element.words(), Word.BOOL);
        return new Listed(element, crossing.construct().name(), crossing.length().name(),
                crossing.at().name());
    }

    /** How a value of {@code type} crosses as one word of the library's, or null where it does not. */
    private @Nullable Whole whole(Type type) {
        return switch (type) {
            case Type.Primitive it -> switch (it.name()) {
                case "Int" -> Whole.integer();
                case "Bool" -> Whole.truth();
                case "String" -> Whole.text();
                default -> null;
            };
            case Type.Declared it -> whole(it.module(), it.name());
            case Type.Option it -> null;
            case Type.Union it -> null;
            case Type.ListOf it -> null;
            case Type.Unrepresented it -> null;
        };
    }

    /** How PHP hands the library a value of {@code type}, or null where it has no way to. */
    private @Nullable Given given(Type type) {
        if (type instanceof Type.Union union) {
            List<Whole> members = members(union);
            return members == null ? null : new OneOf(members);
        }
        return both(type);
    }

    /**
     * How the library hands PHP a value of {@code type}, or null where it has no way to. A union
     * no declaration names is never handed this way: nothing says which case a value of one is
     * where it is not a behavior's answer ({@link #received(Manifest.Answer, String)}).
     */
    private @Nullable Received received(Type type) {
        return both(type);
    }

    /**
     * How the library hands PHP what a behavior answers, or null where it has no way to: a union
     * no declaration names as the class of the case the library says a value is, and anything else
     * as a value of its type is handed.
     *
     * <p>Each case is made as its own class where it has one, and otherwise as a value of the
     * member it is a case of, whose codec decides again: a case the model keeps has no class, and
     * is still a value of the sum the union names. A case neither way is a union PHP cannot be
     * handed.
     */
    private @Nullable Received received(Manifest.Answer answer, String what) {
        if (!(answer.type() instanceof Type.Union union)) {
            return received(answer.type());
        }
        Manifest.UnionAnswer cases = answer.union();
        List<Whole> members = members(union);
        if (cases == null || cases.which() == null || members == null) {
            return null;
        }
        agrees(cases.which(), List.of(Word.VALUE), List.of(), Word.CASE);
        List<Whole> made = new ArrayList<>();
        for (Case of : cases.cases()) {
            Whole it = caseClass(of);
            if (it == null) {
                it = memberHolding(union, of);
            }
            if (it == null) {
                return null;
            }
            made.add(it);
        }
        return new Told(new OneOf(members).phpType(), cases.which().name(), made,
                quotedInSingle("`" + what + "`"));
    }

    /** What each member of {@code union} crosses as, or null where any has no class. */
    private @Nullable List<Whole> members(Type.Union union) {
        List<Whole> members = new ArrayList<>();
        for (Case member : union.cases()) {
            if (!(member instanceof Case.Declared d) || !(whole(d.module(), d.name()) instanceof Whole w)) {
                return null;
            }
            members.add(w);
        }
        return members;
    }

    /** The first member of {@code union} that is a sum {@code of} is a case of. */
    private @Nullable Whole memberHolding(Type.Union union, Case of) {
        if (!(of instanceof Case.Declared leaf)) {
            return null;
        }
        for (Case member : union.cases()) {
            if (member instanceof Case.Declared d
                    && declared.get(d.module() + "." + d.name()) instanceof Declared it
                    && it.declaration() instanceof Declaration.Sum sum
                    && cases(sum).contains(leaf.module() + "." + leaf.name())) {
                return Whole.sum(it.fqcn(), it.codec());
            }
        }
        return null;
    }

    private @Nullable Whole whole(String module, String name) {
        Declared it = declared.get(module + "." + name);
        if (it == null) {
            return null;
        }
        return it.declaration() instanceof Declaration.Sum
                ? Whole.sum(it.fqcn(), it.codec()) : Whole.product(it.fqcn());
    }

    /** How PHP hands over each of {@code types}, or null where any of them has no way. */
    private @Nullable List<Given> givens(List<Type> types) {
        List<Given> crossings = new ArrayList<>();
        for (Type type : types) {
            Given crossing = given(type);
            if (crossing == null) {
                return null;
            }
            crossings.add(crossing);
        }
        return crossings;
    }

    /** How PHP is handed each of {@code types}, or null where any of them has no way. */
    private @Nullable List<Received> receiveds(List<Type> types) {
        List<Received> crossings = new ArrayList<>();
        for (Type type : types) {
            Received crossing = received(type);
            if (crossing == null) {
                return null;
            }
            crossings.add(crossing);
        }
        return crossings;
    }

    /**
     * Holds {@code function} to what this generator hands it and reads of it. The manifest and the
     * crossings above are two readings of one thing, and where they disagree the binding would call
     * the function as something it is not.
     */
    private static void agrees(Function function, List<Word> given, List<Word> rooms,
                               @Nullable Word answers) {
        List<Parameter> expected = new ArrayList<>();
        given.forEach(word -> expected.add(Parameter.given(word)));
        rooms.forEach(word -> expected.add(Parameter.room(word)));
        agrees(function, expected, answers);
    }

    private static void agrees(Function function, List<Parameter> expected, @Nullable Word answers) {
        if (!expected.equals(function.takes()) || answers != function.answers()) {
            throw new IllegalStateException("the manifest says " + function.name() + " takes "
                    + function.takes() + " and answers " + function.answers()
                    + ", and this generator would call it with " + expected + " for " + answers);
        }
    }

    private static List<Word> words(List<? extends Crossing> crossings) {
        return crossings.stream().flatMap(it -> it.words().stream()).toList();
    }

    // ---------------------------------------------------------------------------------------------
    // A module.

    private void module(Manifest.Module module) throws IOException {
        String namespace = PhpNames.moduleNamespace(root, module.name());
        PhpNames.Claimed classes = PhpNames.Claimed.classes("namespace " + namespace);
        classes.claim("Behaviors", "the generated `Behaviors`");
        classes.claim("Values", "the generated `Values`");
        classes.claim("Injections", "the generated `Injections`");
        for (Declaration declaration : module.declarations()) {
            Declared it = declared.get(module.name() + "." + declaration.name());
            classes.claim(it.name(), "type `" + it.key() + "`");
            if (declaration instanceof Declaration.Sum sum) {
                classes.claim(it.name() + "Codec", "the codec of `" + it.key() + "`");
                if (opaque(sum)) {
                    classes.claim(it.name() + "Value", "a value of `" + it.key() + "` no class names");
                }
            }
        }

        for (Declaration declaration : module.declarations()) {
            Declared it = declared.get(module.name() + "." + declaration.name());
            switch (declaration) {
                case Declaration.Sum sum -> sum(it, sum);
                default -> valueClass(it);
            }
        }
        behaviors(module, namespace);
        values(module, namespace);
        injections(module, namespace);
    }

    /**
     * Whether a value of {@code sum} can be one no generated class names: where the library says
     * nothing of which case a value is, or where a case is one the model keeps or a host has no
     * class for.
     */
    private boolean opaque(Declaration.Sum sum) {
        return sum.which() == null || sum.cases().stream().anyMatch(it -> caseClass(it) == null);
    }

    /**
     * The class a value that is {@code of} is made as, or null where no generated class is one: a
     * case the model keeps, a primitive or a language's case, or a declared type of another sum.
     * The one answer both to whether a sum needs a class for values no other class names and to
     * which class each of its cases is made as.
     */
    private @Nullable Whole caseClass(Case of) {
        return of instanceof Case.Declared d && whole(d.module(), d.name()) instanceof Whole w
                && w.kind() == Whole.Kind.PRODUCT ? w : null;
    }

    /** Every sum {@code key} is a case of. */
    private List<Declared> sumsOf(String key) {
        return declared.values().stream()
                .filter(it -> it.declaration() instanceof Declaration.Sum sum
                        && cases(sum).contains(key))
                .toList();
    }

    private static Set<String> cases(Declaration.Sum sum) {
        return sum.cases().stream().map(it -> switch (it) {
            case Case.Declared d -> d.module() + "." + d.name();
            case Case.Other o -> o.kind() + ":" + o.name();
        }).collect(Collectors.toCollection(LinkedHashSet::new));
    }

    /**
     * Of {@code sums}, the ones no other of them already stands for: a sum whose cases are all
     * cases of another is written as extending it, so naming both says one twice.
     */
    private static List<Declared> narrowest(List<Declared> sums) {
        return sums.stream().filter(wide -> sums.stream().noneMatch(narrow -> narrow != wide
                && strictlyWithin(narrow, wide))).toList();
    }

    private static boolean strictlyWithin(Declared narrow, Declared wide) {
        Set<String> in = cases((Declaration.Sum) narrow.declaration());
        Set<String> of = cases((Declaration.Sum) wide.declaration());
        return of.containsAll(in) && !in.equals(of);
    }

    // ---------------------------------------------------------------------------------------------
    // A product, a newtype or a unit.

    private void valueClass(Declared it) throws IOException {
        List<Manifest.Field> fields = switch (it.declaration()) {
            case Declaration.Product product -> product.fields();
            case Declaration.Newtype newtype -> List.of(newtype.field());
            case Declaration.Unit unit -> List.of();
            case Declaration.Sum sum -> throw new IllegalStateException("a sum has no class");
        };
        @Nullable Function construct = switch (it.declaration()) {
            case Declaration.Product product -> product.construct();
            case Declaration.Newtype newtype -> newtype.construct();
            case Declaration.Unit unit -> unit.construct();
            case Declaration.Sum sum -> null;
        };

        PhpNames.Claimed members = PhpNames.Claimed.methods("class " + it.fqcn());
        for (String fixed : List.of("__construct", "nativeHandle", "of", "decode", "encode")) {
            members.claim(fixed, "the generated `" + fixed + "`");
        }
        for (Manifest.Field field : fields) {
            members.claim(PhpNames.memberName(field.name(), "field `" + it.key() + "." + field.name()
                    + "`"), "field `" + field.name() + "`");
        }

        // A sum's interface is a NativeValue already, so a case of one names only the sums.
        List<String> implemented = new ArrayList<>();
        narrowest(sumsOf(it.key())).forEach(sum -> implemented.add(sum.fqcn()));
        if (implemented.isEmpty()) {
            implemented.add(RUNTIME + "NativeValue");
        }

        StringBuilder php = header(it.namespace());
        php.append(doc("", "A value of `" + it.key() + "`, held where the library made it."));
        php.append("final readonly class ").append(it.name()).append(" implements ")
                .append(String.join(", ", implemented)).append("\n{\n");
        php.append(handleMembers());
        if (construct != null) {
            of(php, it, fields, construct);
        }
        decode(php, it, it.declaration().decode(), it.fqcn(), "new " + it.fqcn()
                + "($session->held($value))");
        for (Manifest.Field field : fields) {
            getter(php, it, field);
        }
        Function encode = it.declaration().encode();
        if (encode != null) {
            agrees(encode, List.of(Word.VALUE), List.of(), Word.STRING);
            php.append("""

                        /** This value in the external form of `%s`. */
                        public function encode(): string
                        {
                            $value = $this->handle->read();
                            $session = $this->handle->session();
                            return $session->text($session->ffi()->%s($value));
                        }
                    """.formatted(it.key(), encode.name()));
        }
        php.append("}\n");
        file(it.namespace(), it.name(), php);
    }

    private static String handleMembers() {
        return """
                    /** @internal */
                    public function __construct(private \\Souther\\Runtime\\NativeHandle $handle)
                    {
                    }

                    /** @internal */
                    public function nativeHandle(): \\Souther\\Runtime\\NativeHandle
                    {
                        return $this->handle;
                    }
                """;
    }

    /** The static constructor: the value, or the invariant it does not hold as an issue. */
    private void of(StringBuilder php, Declared it, List<Manifest.Field> fields, Function construct) {
        List<Given> crossings = givens(fields.stream().map(Manifest.Field::type).toList());
        if (crossings == null) {
            return;
        }
        agrees(construct, words(crossings), List.of(Word.VALUE), Word.STATUS);
        PhpNames.Claimed claimed = PhpNames.Claimed.parameters(it.fqcn() + "::of");
        List<String> names = new ArrayList<>();
        for (Manifest.Field field : fields) {
            String what = "field `" + it.key() + "." + field.name() + "`";
            names.add(claimed.claim(PhpNames.parameterName(field.name(), what), what));
        }
        String session = PhpNames.freeOf("session", names);
        String ffi = PhpNames.freeOf("ffi", names);
        String made = PhpNames.freeOf("made", names);
        String status = PhpNames.freeOf("status", names);

        List<String> parameters = new ArrayList<>();
        parameters.add(RUNTIME + "Session $" + session);
        // An optional takes null where nothing after it has to be named: PHP reads a default
        // before a parameter that has none as a mistake.
        int defaulted = crossings.size();
        while (defaulted > 0 && crossings.get(defaulted - 1) instanceof Present) {
            defaulted--;
        }
        List<String> given = new ArrayList<>();
        for (int at = 0; at < crossings.size(); at++) {
            Given crossing = crossings.get(at);
            parameters.add(crossing.phpType() + " $" + names.get(at)
                    + (at >= defaulted ? " = null" : ""));
            given.addAll(crossing.given("$" + names.get(at), "$" + session));
        }
        given.add("\\FFI::addr($" + made + ")");
        StringBuilder described = new StringBuilder();
        for (int at = 0; at < crossings.size(); at++) {
            described.append(paramLine(crossings.get(at), names.get(at)));
        }
        php.append("""

                    /**
                     * A value of `%s`, or an `invariant_violation` where what is handed over does not
                     * hold what the type states.
                     *
                %s     * @return \\Raoh\\Result<%s>
                     */
                    public static function of(%s): \\Raoh\\Result
                    {
                        $%s = $%s->call();
                        $%s = $%s->new('souther_value');
                        $%s = $%s->%s(%s);
                        return $%s->constructed($%s,
                            static fn (): %s => new %s($%s->held($%s)));
                    }
                """.formatted(it.key(), described, it.fqcn(), String.join(", ", parameters),
                ffi, session, made, ffi, status, ffi, construct.name(), String.join(", ", given),
                session, status, it.fqcn(), it.fqcn(), session, made));
    }

    /** Reading a value of the type out of its external form. */
    private static void decode(StringBuilder php, Declared it, @Nullable Function decode,
                               String answers, String made) {
        if (decode == null) {
            return;
        }
        agrees(decode, List.of(Word.BYTES, Word.COUNT), List.of(Word.DECODED), Word.STATUS);
        php.append("""

                    /**
                     * A value of `%s` read out of its external form, or the issues found in it.
                     *
                     * @return \\Raoh\\Result<%s>
                     */
                    public static function decode(\\Souther\\Runtime\\Session $session, string $json): \\Raoh\\Result
                    {
                        $ffi = $session->call();
                        $reading = $ffi->new('souther_decoded');
                        $status = $ffi->%s($session->bytes($json), \\strlen($json), \\FFI::addr($reading));
                        return $session->decoded($status, $reading,
                            static fn (\\FFI\\CData $value): %s => %s);
                    }
                """.formatted(it.key(), answers, decode.name(), answers, made));
    }

    private void getter(StringBuilder php, Declared it, Manifest.Field field) {
        Function read = field.read();
        Received crossing = received(field.type());
        if (read == null || crossing == null) {
            return;
        }
        String body = switch (crossing) {
            case Single single -> {
                agrees(read, List.of(Word.VALUE), List.of(), single.word());
                yield "return " + single.of(List.of("$ffi->" + read.name() + "($value)"),
                        "$session") + ";";
            }
            case Present present -> {
                agrees(read, List.of(Word.VALUE), List.of(present.of().word()), Word.BOOL);
                yield "$room = $ffi->new('" + present.of().cType() + "');\n"
                        + "        $present = $ffi->" + read.name()
                        + "($value, \\FFI::addr($room));\n"
                        + "        return " + present.of(List.of("$present",
                        present.of().fromRoom("$room")), "$session") + ";";
            }
            case Told told -> throw new IllegalStateException("a field is not told its case");
        };
        php.append("\n").append(docLines(List.of("The `" + field.name() + "` of this value."),
                List.of(), crossing));
        php.append("""
                    public function %s(): %s
                    {
                        $value = $this->handle->read();
                        $session = $this->handle->session();
                        $ffi = $session->ffi();
                        %s
                    }
                """.formatted(field.name(), crossing.phpType(), body));
    }

    // ---------------------------------------------------------------------------------------------
    // A sum.

    private void sum(Declared it, Declaration.Sum sum) throws IOException {
        List<String> extended = new ArrayList<>();
        List<Declared> wider = declared.values().stream()
                .filter(other -> other.declaration() instanceof Declaration.Sum
                        && strictlyWithin(it, other))
                .toList();
        narrowest(wider).forEach(other -> extended.add(other.fqcn()));
        if (extended.isEmpty()) {
            extended.add(RUNTIME + "NativeValue");
        }
        StringBuilder face = header(it.namespace());
        face.append(doc("", "A value of `" + it.key() + "`: one of its cases, each a class of its own. "
                + it.name() + "Codec reads and writes it in the sum's external form."));
        face.append("interface ").append(it.name()).append(" extends ")
                .append(String.join(", ", extended)).append("\n{\n}\n");
        file(it.namespace(), it.name(), face);

        boolean opaque = opaque(sum);
        if (opaque) {
            StringBuilder value = header(it.namespace());
            value.append(doc("", "A value of `" + it.key() + "` that is a case no generated class"
                    + " names: one the model keeps, or one a host has no class for."));
            value.append("final readonly class ").append(it.name()).append("Value implements ")
                    .append(it.fqcn()).append("\n{\n").append(handleMembers()).append("}\n");
            file(it.namespace(), it.name() + "Value", value);
        }

        StringBuilder codec = header(it.namespace());
        codec.append(doc("", "Reads a value of `" + it.key() + "` into the class of the case it is,"
                + " and reads and writes the sum's external form."));
        codec.append("final class ").append(it.name()).append("Codec\n{\n");
        codec.append("""
                    private function __construct()
                    {
                    }
                """);
        String cases;
        if (sum.which() == null) {
            cases = "        return new " + it.opaque() + "($session->held($value));";
        } else {
            agrees(sum.which(), List.of(Word.VALUE), List.of(), Word.CASE);
            StringBuilder arms = new StringBuilder();
            for (int at = 0; at < sum.cases().size(); at++) {
                Whole made = caseClass(sum.cases().get(at));
                String arm = made != null ? made.of(List.of("$value"), "$session")
                        : "new " + it.opaque() + "($session->held($value))";
                arms.append("            ").append(at).append(" => ").append(arm).append(",\n");
            }
            cases = "        return match ($session->ffi()->" + sum.which().name() + "($value)) {\n"
                    + arms
                    + "            default => throw new \\LogicException('the library answered a case"
                    + " `" + it.key() + "` does not have'),\n        };";
        }
        codec.append("""

                    /** @internal The value at `$value` as the class of the case it is. */
                    public static function wrap(\\Souther\\Runtime\\Session $session, \\FFI\\CData $value): %s
                    {
                %s
                    }
                """.formatted(it.fqcn(), cases));
        decode(codec, it, sum.decode(), it.fqcn(), it.codec() + "::wrap($session, $value)");
        Function encode = sum.encode();
        if (encode != null) {
            agrees(encode, List.of(Word.VALUE), List.of(), Word.STRING);
            codec.append("""

                        /** `$value` in the external form of `%s`, which says which case it is. */
                        public static function encode(%s $value): string
                        {
                            $handle = $value->nativeHandle();
                            $held = $handle->read();
                            $session = $handle->session();
                            return $session->text($session->ffi()->%s($held));
                        }
                    """.formatted(it.key(), it.fqcn(), encode.name()));
        }
        codec.append("}\n");
        file(it.namespace(), it.name() + "Codec", codec);
    }

    // ---------------------------------------------------------------------------------------------
    // Behaviors and values.

    private void behaviors(Manifest.Module module, String namespace) throws IOException {
        StringBuilder functions = new StringBuilder();
        PhpNames.Claimed members = PhpNames.Claimed.methods("class " + namespace + "\\Behaviors");
        members.claim("__construct", "the generated `__construct`");
        for (Manifest.Behavior behavior : module.behaviors()) {
            Function call = behavior.call();
            List<Given> takes = givens(behavior.parameters().types());
            Received answers = received(behavior.answers(), module.name() + "." + behavior.name());
            if (call == null || takes == null || answers == null) {
                continue;
            }
            String what = "behavior `" + module.name() + "." + behavior.name() + "`";
            members.claim(PhpNames.memberName(behavior.name(), what), what);
            PhpNames.Claimed claimed = PhpNames.Claimed.parameters(what);
            List<String> names = switch (behavior.parameters()) {
                case Manifest.Parameters.Named named -> named.parameters().stream()
                        .map(it -> claimed.claim(PhpNames.parameterName(it.name(),
                                "parameter `" + it.name() + "` of " + what),
                                "parameter `" + it.name() + "`"))
                        .toList();
                case Manifest.Parameters.Positional positional ->
                        PhpNames.positional(positional.types().size());
            };
            functions.append(call(what, behavior.name(), names, takes, answers, call));
        }
        if (functions.isEmpty()) {
            return;
        }
        StringBuilder php = header(namespace);
        php.append(doc("", "The behaviors `" + module.name() + "` publishes, each called in a session"
                + " and answering its value, or throwing where the computation ends without one."));
        php.append("final class Behaviors\n{\n");
        php.append("""
                    private function __construct()
                    {
                    }
                """);
        php.append(functions).append("}\n");
        file(namespace, "Behaviors", php);
    }

    private void values(Manifest.Module module, String namespace) throws IOException {
        StringBuilder functions = new StringBuilder();
        PhpNames.Claimed members = PhpNames.Claimed.methods("class " + namespace + "\\Values");
        members.claim("__construct", "the generated `__construct`");
        for (Manifest.PublishedValue value : module.values()) {
            Function read = value.read();
            Received answers = received(value.type());
            if (read == null || answers == null) {
                continue;
            }
            String what = "value `" + module.name() + "." + value.name() + "`";
            members.claim(PhpNames.memberName(value.name(), what), what);
            functions.append(call(what, value.name(), List.of(), List.of(), answers, read));
        }
        if (functions.isEmpty()) {
            return;
        }
        StringBuilder php = header(namespace);
        php.append(doc("", "The values `" + module.name() + "` publishes, each read in a session."));
        php.append("final class Values\n{\n");
        php.append("""
                    private function __construct()
                    {
                    }
                """);
        php.append(functions).append("}\n");
        file(namespace, "Values", php);
    }

    /** A static function calling {@code function} and answering what it wrote. */
    private static String call(String what, String name, List<String> names, List<Given> takes,
                               Received answers, Function function) {
        List<Word> rooms = answers.words();
        agrees(function, words(takes), rooms, Word.STATUS);
        String session = PhpNames.freeOf("session", names);
        Set<String> taken = new HashSet<>(names);
        taken.add(session);
        String ffi = PhpNames.freeOf("ffi", taken);
        taken.add(ffi);
        String status = PhpNames.freeOf("status", taken);
        taken.add(status);
        List<String> roomNames = new ArrayList<>();
        for (int at = 0; at < rooms.size(); at++) {
            String room = PhpNames.freeOf(at == 0 && rooms.size() == 1 ? "answer" : "answer" + at,
                    taken);
            taken.add(room);
            roomNames.add(room);
        }

        List<String> parameters = new ArrayList<>();
        parameters.add(RUNTIME + "Session $" + session);
        List<String> given = new ArrayList<>();
        for (int at = 0; at < takes.size(); at++) {
            parameters.add(takes.get(at).phpType() + " $" + names.get(at));
            given.addAll(takes.get(at).given("$" + names.get(at), "$" + session));
        }
        StringBuilder body = new StringBuilder();
        body.append("        $").append(ffi).append(" = $").append(session).append("->call();\n");
        for (int at = 0; at < rooms.size(); at++) {
            body.append("        $").append(roomNames.get(at)).append(" = $").append(ffi)
                    .append("->new('").append(Whole.cType(rooms.get(at))).append("');\n");
            given.add("\\FFI::addr($" + roomNames.get(at) + ")");
        }
        body.append("        $").append(status).append(" = $").append(ffi).append("->")
                .append(function.name()).append("(").append(String.join(", ", given)).append(");\n");
        body.append("        $").append(session).append("->answered($").append(status).append(");\n");
        List<String> read = answers.fromRooms(roomNames.stream().map(it -> "$" + it).toList());
        body.append("        return ").append(answers.of(read, "$" + session)).append(";\n");
        List<String> described = new ArrayList<>();
        for (int at = 0; at < takes.size(); at++) {
            String line = paramLine(takes.get(at), names.get(at));
            if (!line.isEmpty()) {
                described.add(line.substring("     * ".length(), line.length() - 1));
            }
        }
        return "\n" + docLines(List.of("Calls " + what + "."), described, answers) + """
                    public static function %s(%s): %s
                    {
                %s    }
                """.formatted(name, String.join(", ", parameters), answers.phpType(), body);
    }

    // ---------------------------------------------------------------------------------------------
    // What a host implements.

    /** The implementations of a module's behaviors a host implements, for one run. */
    private void injections(Manifest.Module module, String namespace) throws IOException {
        List<String> parameters = new ArrayList<>();
        List<String> entries = new ArrayList<>();
        List<String> described = new ArrayList<>();
        PhpNames.Claimed claimed = PhpNames.Claimed.parameters(namespace + "\\Injections::of");
        for (Manifest.Injection injection : module.injections()) {
            if (adapter(module, injection) == null) {
                continue;
            }
            String what = "behavior `" + module.name() + "." + injection.name() + "`";
            String name = claimed.claim(PhpNames.parameterName(injection.name(), what), what);
            parameters.add("?callable $" + name + " = null");
            entries.add("'" + module.name() + "." + injection.name() + "' => $" + name);
            List<String> types = new ArrayList<>();
            types.add(RUNTIME + "Session");
            for (Manifest.NamedParameter parameter : injection.parameters()) {
                types.add(received(parameter.type()).phpDocType());
            }
            described.add("     * @param (callable(" + String.join(", ", types) + "): "
                    + given(injection.answers()).phpDocType() + ")|null $" + name);
        }
        if (parameters.isEmpty()) {
            return;
        }
        StringBuilder php = header(namespace);
        php.append(doc("", "Implementations of the behaviors `" + module.name() + "` asks a host to"
                + " implement, handed to a run. Each is called with the session of the innermost run"
                + " going and what the behavior takes; an exception it throws comes back out of the"
                + " call into the library that reached it."));
        php.append("final class Injections extends \\Souther\\Runtime\\Injections\n{\n");
        php.append("""
                    /**
                %s
                     */
                    public static function of(%s): self
                    {
                        $given = array_filter([%s], static fn (?callable $it): bool => $it !== null);
                        return new self(array_map(static fn (callable $it): \\Closure => \\Closure::fromCallable($it), $given));
                    }
                }
                """.formatted(String.join("\n", described), String.join(", ", parameters),
                String.join(", ", entries)));
        file(namespace, "Injections", php);
    }

    /**
     * What the runtime hands what C passes an implementation of {@code injection} through: PHP
     * values made of each word, the implementation called with them, and its answer written through
     * the room C handed over. Null where a host could not be handed what it takes or hand back
     * what it answers.
     */
    private @Nullable String adapter(Manifest.Module module, Manifest.Injection injection) {
        List<Received> takes = receiveds(injection.parameters().stream()
                .map(Manifest.NamedParameter::type).toList());
        Given answers = given(injection.answers());
        if (takes == null || answers == null) {
            return null;
        }
        Manifest.Implementation implementation = injection.implementation();
        List<Parameter> expected = new ArrayList<>();
        words(takes).forEach(word -> expected.add(Parameter.given(word)));
        answers.words().forEach(word -> expected.add(Parameter.room(word)));
        if (!expected.equals(implementation.takes()) || implementation.answers() != Word.STATUS) {
            throw new IllegalStateException("the manifest says an implementation of "
                    + module.name() + "." + injection.name() + " takes " + implementation.takes()
                    + ", and this generator would hand it " + expected);
        }
        List<String> arguments = new ArrayList<>();
        arguments.add("$session");
        int at = 0;
        for (Received crossing : takes) {
            List<String> handed = new ArrayList<>();
            for (int word = 0; word < crossing.words().size(); word++) {
                handed.add("$handed[" + at++ + "]");
            }
            arguments.add(crossing.of(handed, "$session"));
        }
        StringBuilder written = new StringBuilder();
        List<String> given = answers.given("$answer", "$session");
        for (String word : given) {
            written.append("                    $handed[").append(at++).append("][0] = ").append(word)
                    .append(";\n");
        }
        return """
                static function (\\Souther\\Runtime\\Session $session, \\Closure $implementation, array $handed): void {
                                    $answer = $implementation(%s);
                                    if (!(%s)) {
                                        throw new \\TypeError('an implementation of %s answered other than %s');
                                    }
                %s                }""".formatted(String.join(", ", arguments), answers.holds("$answer"),
                quotedInSingle(module.name() + "." + injection.name()),
                quotedInSingle(answers.phpType()), written);
    }

    // ---------------------------------------------------------------------------------------------
    // The binding, and loading it.

    private void binding() throws IOException {
        StringBuilder slots = new StringBuilder();
        for (Manifest.Module module : manifest.modules()) {
            for (Manifest.Injection injection : module.injections()) {
                String adapter = adapter(module, injection);
                if (adapter == null) {
                    continue;
                }
                slots.append("            '").append(module.name()).append('.')
                        .append(injection.name()).append("' => new \\Souther\\Runtime\\InjectionSlot(")
                        .append("$library, '").append(injection.implementation().type()).append("', '")
                        .append(injection.register()).append("',\n                ").append(adapter)
                        .append("),\n");
            }
        }
        StringBuilder php = header(root);
        php.append(doc("", "The library this binding was generated for, and the runs a host makes of"
                + " it."));
        php.append("final class Binding extends \\Souther\\Runtime\\Binding\n{\n");
        php.append("    private const STATUSES = ").append(array(manifest.statuses())).append(";\n\n");
        php.append("    private const OUTCOMES = ").append(array(manifest.outcomes())).append(";\n");
        php.append("""

                    /**
                     * The library at `$library`, declared by the declarations the build wrote beside it,
                     * which were copied here.
                     */
                    public static function load(string $library, ?string $declarations = null): self
                    {
                        return self::over(\\Souther\\Runtime\\NativeLibrary::load(
                            $declarations ?? __DIR__ . '/%s', $library, self::STATUSES, self::OUTCOMES));
                    }

                    /**
                     * The library at `$library`, as `opcache.preload` declared it under `$scope`, for
                     * `ffi.enable=preload`.
                     */
                    public static function preloaded(string $scope, string $library): self
                    {
                        return self::over(\\Souther\\Runtime\\NativeLibrary::preloaded(
                            $scope, $library, self::STATUSES, self::OUTCOMES));
                    }

                    /**
                     * What a preload script hands `FFI::load()` for this library, under `$scope`, at
                     * `$library` on the machine it is deployed to.
                     */
                    public static function preloadHeader(string $scope, string $library): string
                    {
                        return \\Souther\\Runtime\\NativeLibrary::preloadHeader(
                            __DIR__ . '/%s', $scope, $library);
                    }

                    /** @var array<int, self> */
                    private static array $bindings = [];

                    /**
                     * One binding for each library, made the first time it is asked for: what each
                     * implementation is registered through is made into a C entry here, and PHP keeps
                     * every entry it makes until the request ends, which a worker never does.
                     */
                    private static function over(\\Souther\\Runtime\\NativeLibrary $library): self
                    {
                        $speaks = \\defined('\\Souther\\Runtime\\Binding::PROTOCOL')
                            ? \\Souther\\Runtime\\Binding::PROTOCOL : 0;
                        if ($speaks !== %d) {
                            throw new \\LogicException('this binding was generated for version %d of'
                                . ' what it calls of souther-lang/php-runtime, and the runtime installed'
                                . ' is version ' . $speaks);
                        }
                        return self::$bindings[spl_object_id($library)] ??= new self($library, [
                %s        ]);
                    }
                }
                """.formatted(DECLARATIONS, DECLARATIONS, RUNTIME_PROTOCOL, RUNTIME_PROTOCOL, slots));
        file(root, "Binding", php);
    }

    /** A PHP array of {@code numbers}, in the order of the names so a build writes it the same. */
    private static String array(Map<String, Integer> numbers) {
        return new java.util.TreeMap<>(numbers).entrySet().stream()
                .map(it -> "'" + it.getKey() + "' => " + it.getValue())
                .collect(Collectors.joining(", ", "[", "]"));
    }

    /** A loader for the binding's classes, for a host that does not map the namespace itself. */
    private void autoload() throws IOException {
        String php = """
                <?php

                declare(strict_types=1);

                // Loads the classes of the binding generated under %s, from where they stand here.
                spl_autoload_register(static function (string $class): void {
                    $prefix = '%s\\\\';
                    if (strncmp($class, $prefix, strlen($prefix)) !== 0) {
                        return;
                    }
                    $file = __DIR__ . '/' . str_replace('\\\\', '/', substr($class, strlen($prefix))) . '.php';
                    if (is_file($file)) {
                        require $file;
                    }
                });
                """.formatted(root, quotedInSingle(root));
        Path at = into.resolve("autoload.php");
        Files.createDirectories(into);
        Files.writeString(at, php, StandardCharsets.UTF_8);
        written.add(at);
    }

    // ---------------------------------------------------------------------------------------------
    // Writing.

    private static StringBuilder header(String namespace) {
        return new StringBuilder("<?php\n\n// Generated by souther-native-compiler from souther.json."
                + " Written again on every build.\n\ndeclare(strict_types=1);\n\nnamespace ")
                .append(namespace).append(";\n\n");
    }

    /**
     * A {@code @param} line of a docblock for {@code crossing} under {@code name}, where the
     * docblock says more of it than its PHP type does, and nothing where it does not.
     */
    private static String paramLine(Crossing crossing, String name) {
        return crossing.phpDocType().equals(crossing.phpType()) ? ""
                : "     * @param " + crossing.phpDocType() + " $" + name + "\n";
    }

    /**
     * A member's docblock: {@code text}, then each of {@code params}, then {@code @return} where
     * what it answers is more than its PHP type says.
     */
    private static String docLines(List<String> text, List<String> params, Crossing answers) {
        List<String> lines = new ArrayList<>(text);
        List<String> tags = new ArrayList<>(params);
        if (!answers.phpDocType().equals(answers.phpType())) {
            tags.add("@return " + answers.phpDocType());
        }
        if (tags.isEmpty() && lines.size() == 1) {
            return "    /** " + lines.getFirst() + " */\n";
        }
        if (!tags.isEmpty()) {
            lines.add("");
            lines.addAll(tags);
        }
        return lines.stream().map(it -> it.isEmpty() ? "     *" : "     * " + it)
                .collect(Collectors.joining("\n", "    /**\n", "\n     */\n"));
    }

    private static String doc(String indent, String text) {
        return indent + "/**\n" + indent + " * " + text + "\n" + indent + " */\n";
    }

    private static String quotedInSingle(String text) {
        return text.replace("\\", "\\\\").replace("'", "\\'");
    }

    /** Writes {@code php} where PSR-4 puts {@code namespace\name} under the root namespace. */
    private void file(String namespace, String name, CharSequence php) throws IOException {
        Path directory = into;
        if (!namespace.equals(root)) {
            for (String part : namespace.substring(root.length() + 1).split("\\\\")) {
                directory = directory.resolve(part);
            }
        }
        Files.createDirectories(directory);
        Path at = directory.resolve(name + ".php");
        Files.writeString(at, php, StandardCharsets.UTF_8);
        written.add(at);
    }
}
