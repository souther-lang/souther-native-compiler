package souther.nativecode.php;

import org.jspecify.annotations.Nullable;
import souther.nativecode.NativeCompiler;
import souther.nativecode.php.Crossing.Present;
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

    private final Manifest manifest;
    private final String root;
    private final Path into;
    private final List<Path> written = new ArrayList<>();

    /** What each declared type is, by {@code module.Name}. */
    private final Map<String, Declared> declared = new LinkedHashMap<>();

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

    static Generated generate(Path manifest, Path declarations, Path into, String namespace)
            throws IOException {
        PhpBindings binding = new PhpBindings(Manifest.read(manifest),
                PhpNames.rootNamespace(namespace), into);
        binding.write();
        Files.copy(declarations, into.resolve(DECLARATIONS),
                java.nio.file.StandardCopyOption.REPLACE_EXISTING);
        binding.written.add(into.resolve(DECLARATIONS));
        return new Generated(into, List.copyOf(binding.written));
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
        PhpNames.Claimed namespaces = new PhpNames.Claimed("namespace " + root);
        for (Manifest.Module module : manifest.modules()) {
            String namespace = PhpNames.moduleNamespace(root, module.name());
            namespaces.claim(namespace.substring(root.length() + 1), "module `" + module.name() + "`");
            for (Declaration declaration : module.declarations()) {
                Declared it = new Declared(module.name(), declaration, namespace,
                        PhpNames.typeName(declaration.name(),
                                "type `" + module.name() + "." + declaration.name() + "`"));
                declared.put(it.key(), it);
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

    /** How a value of {@code type} crosses, or null where a host has no way to hand one over. */
    private @Nullable Crossing crossing(Type type) {
        return switch (type) {
            case Type.Primitive it -> switch (it.name()) {
                case "Int" -> Whole.integer();
                case "Bool" -> Whole.truth();
                case "String" -> Whole.text();
                default -> null;
            };
            case Type.Declared it -> whole(it.module(), it.name());
            case Type.Option it -> crossing(it.of()) instanceof Whole whole ? new Present(whole) : null;
            case Type.Union it -> null;
            case Type.Unrepresented it -> null;
        };
    }

    private @Nullable Whole whole(String module, String name) {
        Declared it = declared.get(module + "." + name);
        if (it == null) {
            return null;
        }
        return it.declaration() instanceof Declaration.Sum
                ? Whole.sum(it.fqcn(), it.codec()) : Whole.product(it.fqcn());
    }

    /** The crossing of each of {@code types}, or null where any of them has none. */
    private @Nullable List<Crossing> crossings(List<Type> types) {
        List<Crossing> crossings = new ArrayList<>();
        for (Type type : types) {
            Crossing crossing = crossing(type);
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
        given.forEach(word -> expected.add(new Parameter(false, word)));
        rooms.forEach(word -> expected.add(new Parameter(true, word)));
        if (!expected.equals(function.takes()) || answers != function.answers()) {
            throw new IllegalStateException("the manifest says " + function.name() + " takes "
                    + function.takes() + " and answers " + function.answers()
                    + ", and this generator would call it with " + expected + " for " + answers);
        }
    }

    private static List<Word> words(List<Crossing> crossings) {
        return crossings.stream().flatMap(it -> it.words().stream()).toList();
    }

    // ---------------------------------------------------------------------------------------------
    // A module.

    private void module(Manifest.Module module) throws IOException {
        String namespace = PhpNames.moduleNamespace(root, module.name());
        PhpNames.Claimed classes = new PhpNames.Claimed("namespace " + namespace);
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
        return sum.which() == null || sum.cases().stream()
                .anyMatch(it -> !(it instanceof Case.Declared d)
                        || !(whole(d.module(), d.name()) instanceof Whole w)
                        || w.kind() != Whole.Kind.PRODUCT);
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

        PhpNames.Claimed members = new PhpNames.Claimed("class " + it.fqcn());
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
                + "($session->handle($value))");
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
        List<Crossing> crossings = crossings(fields.stream().map(Manifest.Field::type).toList());
        if (crossings == null) {
            return;
        }
        agrees(construct, words(crossings), List.of(Word.VALUE), Word.STATUS);
        List<String> names = new ArrayList<>();
        for (Manifest.Field field : fields) {
            names.add(PhpNames.parameterName(field.name(), "field `" + it.key() + "." + field.name()
                    + "`"));
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
            Crossing crossing = crossings.get(at);
            parameters.add(crossing.phpType() + " $" + names.get(at)
                    + (at >= defaulted ? " = null" : ""));
            given.addAll(crossing.given("$" + names.get(at), "$" + session));
        }
        given.add("\\FFI::addr($" + made + ")");
        php.append("""

                    /**
                     * A value of `%s`, or an `invariant_violation` where what is handed over does not
                     * hold what the type states.
                     *
                     * @return \\Raoh\\Result<%s>
                     */
                    public static function of(%s): \\Raoh\\Result
                    {
                        $%s = $%s->ffi();
                        $%s = $%s->new('souther_value');
                        $%s = $%s->%s(%s);
                        return $%s->constructed($%s,
                            static fn (): %s => new %s($%s->handle($%s)));
                    }
                """.formatted(it.key(), it.fqcn(), String.join(", ", parameters),
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
                        $ffi = $session->ffi();
                        $reading = $ffi->new('souther_decoded');
                        $status = $ffi->%s($session->bytes($json), \\strlen($json), \\FFI::addr($reading));
                        return $session->decoded($status, $reading,
                            static fn (\\FFI\\CData $value): %s => %s);
                    }
                """.formatted(it.key(), answers, decode.name(), answers, made));
    }

    private void getter(StringBuilder php, Declared it, Manifest.Field field) {
        Function read = field.read();
        Crossing crossing = crossing(field.type());
        if (read == null || crossing == null) {
            return;
        }
        String body = switch (crossing) {
            case Whole whole -> {
                agrees(read, List.of(Word.VALUE), List.of(), whole.word());
                yield "return " + whole.of(List.of("$ffi->" + read.name() + "($value)"),
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
        };
        php.append("""

                    /** The `%s` of this value. */
                    public function %s(): %s
                    {
                        $value = $this->handle->read();
                        $session = $this->handle->session();
                        $ffi = $session->ffi();
                        %s
                    }
                """.formatted(field.name(), field.name(), crossing.phpType(), body));
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
            cases = "        return new " + it.opaque() + "($session->handle($value));";
        } else {
            agrees(sum.which(), List.of(Word.VALUE), List.of(), Word.CASE);
            StringBuilder arms = new StringBuilder();
            for (int at = 0; at < sum.cases().size(); at++) {
                Case of = sum.cases().get(at);
                Whole whole = of instanceof Case.Declared d ? whole(d.module(), d.name()) : null;
                String made = whole != null && whole.kind() == Whole.Kind.PRODUCT
                        ? whole.of(List.of("$value"), "$session")
                        : "new " + it.opaque() + "($session->handle($value))";
                arms.append("            ").append(at).append(" => ").append(made).append(",\n");
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
        PhpNames.Claimed members = new PhpNames.Claimed("class " + namespace + "\\Behaviors");
        members.claim("__construct", "the generated `__construct`");
        for (Manifest.Behavior behavior : module.behaviors()) {
            Function call = behavior.call();
            List<Crossing> takes = crossings(behavior.parameters().types());
            Crossing answers = crossing(behavior.answers());
            if (call == null || takes == null || answers == null) {
                continue;
            }
            String what = "behavior `" + module.name() + "." + behavior.name() + "`";
            members.claim(PhpNames.memberName(behavior.name(), what), what);
            List<String> names = switch (behavior.parameters()) {
                case Manifest.Parameters.Named named -> named.parameters().stream()
                        .map(it -> PhpNames.parameterName(it.name(), "a parameter of " + what))
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
        PhpNames.Claimed members = new PhpNames.Claimed("class " + namespace + "\\Values");
        members.claim("__construct", "the generated `__construct`");
        for (Manifest.PublishedValue value : module.values()) {
            Function read = value.read();
            Crossing answers = crossing(value.type());
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
    private static String call(String what, String name, List<String> names, List<Crossing> takes,
                               Crossing answers, Function function) {
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
        body.append("        $").append(ffi).append(" = $").append(session).append("->ffi();\n");
        for (int at = 0; at < rooms.size(); at++) {
            body.append("        $").append(roomNames.get(at)).append(" = $").append(ffi)
                    .append("->new('").append(Whole.cType(rooms.get(at))).append("');\n");
            given.add("\\FFI::addr($" + roomNames.get(at) + ")");
        }
        body.append("        $").append(status).append(" = $").append(ffi).append("->")
                .append(function.name()).append("(").append(String.join(", ", given)).append(");\n");
        body.append("        $").append(session).append("->answered($").append(status).append(");\n");
        List<String> read = new ArrayList<>();
        switch (answers) {
            case Whole whole -> read.add(whole.fromRoom("$" + roomNames.getFirst()));
            case Present present -> {
                read.add("$" + roomNames.get(0) + "->cdata");
                read.add(present.of().fromRoom("$" + roomNames.get(1)));
            }
        }
        body.append("        return ").append(answers.of(read, "$" + session)).append(";\n");
        return """

                    /** Calls %s. */
                    public static function %s(%s): %s
                    {
                %s    }
                """.formatted(what, name, String.join(", ", parameters), answers.phpType(), body);
    }

    // ---------------------------------------------------------------------------------------------
    // What a host implements.

    /** The implementations of a module's behaviors a host implements, for one run. */
    private void injections(Manifest.Module module, String namespace) throws IOException {
        List<String> parameters = new ArrayList<>();
        List<String> entries = new ArrayList<>();
        List<String> described = new ArrayList<>();
        for (Manifest.Injection injection : module.injections()) {
            if (adapter(module, injection) == null) {
                continue;
            }
            String what = "behavior `" + module.name() + "." + injection.name() + "`";
            String name = PhpNames.parameterName(injection.name(), what);
            parameters.add("?callable $" + name + " = null");
            entries.add("'" + module.name() + "." + injection.name() + "' => $" + name);
            List<String> types = new ArrayList<>();
            types.add(RUNTIME + "Session");
            for (Manifest.NamedParameter parameter : injection.parameters()) {
                types.add(crossing(parameter.type()).phpType());
            }
            described.add("     * @param (callable(" + String.join(", ", types) + "): "
                    + crossing(injection.answers()).phpType() + ")|null $" + name);
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
        List<Crossing> takes = crossings(injection.parameters().stream()
                .map(Manifest.NamedParameter::type).toList());
        Crossing answers = crossing(injection.answers());
        if (takes == null || answers == null) {
            return null;
        }
        Manifest.Implementation implementation = injection.implementation();
        List<Parameter> expected = new ArrayList<>();
        words(takes).forEach(word -> expected.add(new Parameter(false, word)));
        answers.words().forEach(word -> expected.add(new Parameter(true, word)));
        if (!expected.equals(implementation.takes()) || implementation.answers() != Word.STATUS) {
            throw new IllegalStateException("the manifest says an implementation of "
                    + module.name() + "." + injection.name() + " takes " + implementation.takes()
                    + ", and this generator would hand it " + expected);
        }
        List<String> arguments = new ArrayList<>();
        arguments.add("$session");
        int at = 0;
        for (Crossing crossing : takes) {
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

                    /** The library `opcache.preload` declared under `$scope`, for `ffi.enable=preload`. */
                    public static function preloaded(string $scope): self
                    {
                        return self::over(\\Souther\\Runtime\\NativeLibrary::preloaded(
                            $scope, self::STATUSES, self::OUTCOMES));
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
                        return self::$bindings[spl_object_id($library)] ??= new self($library, [
                %s        ]);
                    }
                }
                """.formatted(DECLARATIONS, DECLARATIONS, slots));
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
