package souther.bindings.php;

import org.jspecify.annotations.Nullable;
import souther.bindings.Manifest;
import souther.bindings.Manifest.Case;
import souther.bindings.Manifest.Declaration;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.Word;
import souther.bindings.NotBindable;
import souther.bindings.Output;
import souther.bindings.php.Crossing.Both;
import souther.bindings.php.Crossing.Callable;
import souther.bindings.php.Crossing.Given;
import souther.bindings.php.Crossing.Listed;
import souther.bindings.php.Crossing.Member;
import souther.bindings.php.Crossing.OneOf;
import souther.bindings.php.Crossing.Optional;
import souther.bindings.php.Crossing.Received;
import souther.bindings.php.Crossing.Told;
import souther.bindings.php.Crossing.Tuple;
import souther.bindings.php.Crossing.Whole;

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
import java.util.Objects;
import java.util.Set;
import java.util.stream.Collectors;

/**
 * Writes the PHP a host calls a library through, from the manifest the build wrote beside it.
 *
 * <p>Reads the manifest and nothing else: not the checked program, and nothing of how a value is
 * laid out. What it writes is the manifest's surface as PHP types — a class for each published
 * type, an interface for each sum, a static function for each behavior, and a class for each
 * behavior as an application holds one, bound to what it requires or extended to implement it —
 * over the runtime package in {@code bindings/php/runtime}, which is where a run's arena, a value's
 * lifetime and what each status means are kept. The FFI declarations are the ones the build wrote,
 * copied beside it.
 *
 * <p>What a host has no way to reach is not written: a behavior the manifest says is unavailable, a
 * field with no {@code read}, a type PHP has no representation for. A function that could not be
 * called would be one a caller finds out about by calling it. How a value crosses is read off the
 * manifest's shape for it, and what is decided here is only how PHP holds it ({@link Crossing}).
 */
public final class PhpBindings {

    /** What a binding is written as: the directory its namespace stands in, and every file in it. */
    public record Generated(Path root, List<Path> files) {

        public Generated {
            files = List.copyOf(files);
        }
    }

    /** Where the declarations are copied to, beside the binding that loads them. */
    static final String DECLARATIONS = "souther.ffi.h";

    /** What says a directory is a PHP binding this wrote, and may be replaced whole. */
    static final String MARK = ".souther-php-binding";

    private static final String RUNTIME = "\\Souther\\Runtime\\";

    /**
     * The version of what generated code calls of the runtime package that this writes against:
     * {@code Binding::PROTOCOL} in {@code bindings/php/runtime}, which a test holds to this.
     */
    static final int RUNTIME_PROTOCOL = 8;

    private final Manifest manifest;
    private final String root;
    private final Path into;
    private final List<Path> written = new ArrayList<>();

    /** What each declared type is, by {@code module.Name}. */
    private final Map<String, Declared> declared = new LinkedHashMap<>();

    /**
     * Each module, by its name: what a value crosses in a function of one module's is asked of that
     * module, whose own functions a list is built and read through. The library defines them under
     * the module, so a class of one module calling another's would reach across what one object
     * offers.
     */
    private final Map<String, Manifest.Module> modules = new LinkedHashMap<>();

    /** The class each behavior is written as, by {@code module.name}, where it has one. */
    private final Map<String, BehaviorClass> behaviorClasses = new LinkedHashMap<>();

    /** What a host constructs each behavior out of, by {@code module.name}. */
    private final Map<String, Manifest.Construction> constructions = new LinkedHashMap<>();

    /**
     * Each function value PHP hands the library, as PHP holds one, by the slot the binding keeps
     * for it, in the order they were first written: what the library calls a closure of PHP's
     * through. One for each function type as PHP holds it and not for each shape, since what a
     * closure is handed is made into the classes its type names, and two types crossing in one
     * shape are two sets of classes.
     */
    private final Map<String, Callable> hosting = new LinkedHashMap<>();

    private PhpBindings(Manifest manifest, String root, Path into) {
        this.manifest = manifest;
        this.root = root;
        this.into = into;
        for (Manifest.Module module : manifest.modules()) {
            for (Manifest.Construction construction : module.constructions()) {
                constructions.put(module.name() + "." + construction.name(), construction);
            }
        }
    }

    /** What constructing {@code key} requires injected, in order, and nothing where it requires nothing. */
    private List<Manifest.Required> requiresOf(String key) {
        Manifest.Construction construction = constructions.get(key);
        return construction == null ? List.of() : construction.requires();
    }

    /** What a host makes a capability of {@code key} through, as PHP writes the name, or {@code null}. */
    private String bindOf(String key) {
        Manifest.Construction construction = constructions.get(key);
        return construction == null || construction.bind() == null ? "null"
                : "'" + quotedInSingle(construction.bind().name()) + "'";
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
        Output.replaceable(into, MARK);
    }

    /**
     * Writes the binding of what {@code manifest} describes into {@code into}, under the namespace
     * {@code namespace}, beside a copy of {@code declarations}, the C declarations the build wrote
     * for an FFI to read, which the binding loads the library through.
     *
     * <p>The namespace is the binding's own, and not read off the model, so that two libraries
     * publishing a module of the same name can stand in one application.
     *
     * <p>{@code into} is then that binding and nothing else: it is written beside it and put in
     * place whole ({@link Output}), so a class the model no longer declares does not survive a
     * generation, and a refused one leaves what was there as it was.
     *
     * @throws NotBindable where a name in the model is not one PHP takes
     */
    public static Generated generate(Path manifest, Path declarations, Path into, String namespace)
            throws IOException {
        Manifest read = Manifest.read(manifest);
        String root = PhpNames.rootNamespace(namespace);
        Output output = Output.replacing(into, MARK);
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

    /**
     * The class a behavior is written as: abstract where a host implements the behavior, and
     * otherwise final, bound to what the behavior requires.
     */
    private record BehaviorClass(String module, String name, String namespace, String className,
                                 boolean injected) {

        String fqcn() {
            return "\\" + namespace + "\\" + className;
        }

        String key() {
            return module + "." + name;
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
            modules.put(module.name(), module);
        }
        classes();
        for (Manifest.Module module : manifest.modules()) {
            module(module);
        }
        binding();
        autoload();
    }

    /**
     * Which behaviors are written as a class, of every module: each a host implements and can be
     * handed across to, and each published behavior a host can call whose every requirement has a
     * class too, since binding it hands an instance of each over.
     *
     * <p>A class is what this generator adds beside what the model publishes, and its name is this
     * generator's: the behavior's, made capital. So a name PHP will not take for it, or one that is
     * one class with another the module's binding writes, leaves the behavior with no class rather
     * than refusing the binding. The behavior is still a function on {@code Behaviors}, and what
     * requires it has no class either. What the model itself names is refused where PHP will not
     * take it, as before; this never is.
     */
    private void classes() {
        Map<String, BehaviorClass> candidates = new LinkedHashMap<>();
        Map<String, List<Manifest.Required>> requires = new LinkedHashMap<>();
        for (Manifest.Module module : manifest.modules()) {
            String namespace = PhpNames.moduleNamespace(root, module.name());
            for (Manifest.Injection injection : module.injections()) {
                if (adapter(module, injection) != null) {
                    BehaviorClass it = new BehaviorClass(module.name(), injection.name(), namespace,
                            PhpNames.capitalized(injection.name()), true);
                    candidates.put(it.key(), it);
                    requires.put(it.key(), List.of());
                }
            }
            Crossings crossings = in(module.name());
            for (Manifest.Behavior behavior : module.behaviors()) {
                String key = module.name() + "." + behavior.name();
                Manifest.Call call = behavior.call().available();
                if (call != null
                        && crossings.givens(behavior.parameters().types(), call.signature().takes())
                        != null
                        && crossings.received(behavior.answers(), call.signature().answers(), key)
                        != null) {
                    candidates.put(key, new BehaviorClass(module.name(), behavior.name(), namespace,
                            PhpNames.capitalized(behavior.name()), false));
                    requires.put(key, requiresOf(key));
                }
            }
        }

        // One class to PHP or to a file system is one file, whichever two claim it.
        Map<String, Set<String>> written = new LinkedHashMap<>();
        for (Manifest.Module module : manifest.modules()) {
            written.put(module.name(), coreClasses(module).stream()
                    .map(it -> PhpNames.asAFile(it.getKey())).collect(Collectors.toSet()));
        }
        Map<String, Long> spelt = candidates.values().stream().collect(Collectors.groupingBy(
                it -> it.module() + "\n" + PhpNames.asAFile(it.className()), Collectors.counting()));
        candidates.values().removeIf(it -> !PhpNames.takesAsClass(it.className())
                || written.get(it.module()).contains(PhpNames.asAFile(it.className()))
                || spelt.get(it.module() + "\n" + PhpNames.asAFile(it.className())) > 1);

        // A behavior requiring one with no class has none either, and so on up what requires it.
        boolean dropped = true;
        while (dropped) {
            dropped = candidates.values().removeIf(it -> requires.get(it.key()).stream()
                    .anyMatch(required -> !candidates.containsKey(required.key())));
        }
        behaviorClasses.putAll(candidates);
    }

    /**
     * Every class the binding writes for {@code module} whatever else it writes, each with what it
     * is: the three it always may, and each type's. Two of them that are one name are refused, since
     * each is what the model or the binding cannot go without.
     */
    private List<Map.Entry<String, String>> coreClasses(Manifest.Module module) {
        // A list and not a map: two of these under one name is what claiming them refuses, and a
        // map would keep one of the two.
        List<Map.Entry<String, String>> classes = new ArrayList<>();
        classes.add(Map.entry("Behaviors", "the generated `Behaviors`"));
        classes.add(Map.entry("Values", "the generated `Values`"));
        classes.add(Map.entry("Injections", "the generated `Injections`"));
        for (Declaration declaration : module.declarations()) {
            Declared it = declared.get(module.name() + "." + declaration.name());
            classes.add(Map.entry(it.name(), "type `" + it.key() + "`"));
            if (declaration instanceof Declaration.Sum sum) {
                classes.add(Map.entry(it.name() + "Codec", "the codec of `" + it.key() + "`"));
                if (opaque(sum)) {
                    classes.add(Map.entry(it.name() + "Value",
                            "a value of `" + it.key() + "` no class names"));
                }
            }
        }
        return classes;
    }

    // ---------------------------------------------------------------------------------------------
    // What a model type crosses as.

    /** How a value crosses in a function of {@code module}'s. */
    private Crossings in(String module) {
        return new Crossings(modules.get(module));
    }

    /**
     * How a value crosses in the functions one module hands it across in, as PHP holds it: the
     * shape the manifest says it crosses in, and whether PHP has a way to hold what crosses in that
     * shape. Every way this generator asks how a type crosses goes through one of these, so none of
     * them can be asked without saying whose function the value crosses in, which is whose lists
     * and function values it is built and called through.
     *
     * <p>What this adds to the shape is what PHP cannot hold: a value of a declared type this
     * binding has no class for, and a union no declaration names anywhere but where PHP hands one
     * over whole, as the class of one of its members, or is told its case by a behavior.
     */
    private final class Crossings {

        private final Manifest.Module module;

        Crossings(Manifest.Module module) {
            this.module = module;
        }

        /** How PHP hands the library a value of {@code type} in {@code shape}, or null where it has no way to. */
        @Nullable Given given(Type type, Shape shape) {
            if (shape instanceof Shape.Leaf && type instanceof Type.Union union) {
                List<Member> members = members(union);
                return members == null ? null : new OneOf(union, members);
            }
            return both(type, shape);
        }

        /** How the library hands PHP a value of {@code type} in {@code shape}, or null where it has no way to. */
        @Nullable Received received(Type type, Shape shape) {
            return both(type, shape);
        }

        /**
         * How the library hands PHP what a behavior answers, or null where it has no way to: a
         * union no declaration names as the class of the case the library says a value is, and
         * anything else as a value of its type is handed.
         *
         * <p>Each case is made as its own class where it has one, and otherwise as a value of the
         * member it is a case of, whose codec decides again: a case the model keeps has no class,
         * and is still a value of the sum the union names. A case neither way is a union PHP cannot
         * be handed.
         */
        @Nullable Received received(Manifest.Answer answer, Shape shape, String what) {
            if (!(answer.type() instanceof Type.Union union)) {
                return received(answer.type(), shape);
            }
            Manifest.UnionAnswer told = Objects.requireNonNull(answer.union());
            List<Member> members = members(union);
            if (members == null || told.which() == null) {
                return null;
            }
            List<Member> made = new ArrayList<>();
            for (Case of : told.cases()) {
                Member it = switch (of) {
                    case Case.Declared d -> {
                        Whole whole = caseClass(of);
                        if (whole == null) {
                            whole = memberHolding(union, of);
                        }
                        yield whole == null ? null : new Member(whole, null);
                    }
                    case Case.Primitive p -> carried(p);
                    case Case.Language l -> null;
                };
                if (it == null) {
                    return null;
                }
                made.add(it);
            }
            return new Told(told.which(), members.stream().map(it -> it.whole().phpType())
                    .collect(Collectors.joining("|")), made, quotedInSingle("`" + what + "`"));
        }

        /** How PHP hands over each of {@code types} in its shape, or null where any of them has no way. */
        @Nullable List<Given> givens(List<Type> types, List<Shape> shapes) {
            List<Given> crossings = new ArrayList<>();
            for (int at = 0; at < types.size(); at++) {
                Given crossing = given(types.get(at), shapes.get(at));
                if (crossing == null) {
                    return null;
                }
                crossings.add(crossing);
            }
            return crossings;
        }

        /** How PHP is handed each of {@code types} in its shape, or null where any of them has no way. */
        @Nullable List<Received> receiveds(List<Type> types, List<Shape> shapes) {
            List<Received> crossings = new ArrayList<>();
            for (int at = 0; at < types.size(); at++) {
                Received crossing = received(types.get(at), shapes.get(at));
                if (crossing == null) {
                    return null;
                }
                crossings.add(crossing);
            }
            return crossings;
        }

        /**
         * How PHP holds a value of {@code type} crossing both ways in {@code shape}, or null where
         * it has no way. What a list holds, what a tuple is made of, what an optional holds and
         * what a function value takes and answers are each held both ways, even where the value
         * crosses one way, since what they are made of crosses either way inside it: a union no
         * declaration names is refused inside any of them, having no way to say which case a
         * value read out of one is.
         */
        private @Nullable Both both(Type type, Shape shape) {
            return switch (shape) {
                case Shape.Leaf leaf -> switch (type) {
                    case Type.Primitive it -> Whole.primitive(leaf.word());
                    case Type.Declared it -> whole(it.module(), it.name());
                    default -> null;
                };
                case Shape.Option option -> {
                    Both of = both(((Type.Option) type).of(), option.of());
                    yield of == null ? null : new Optional(of);
                }
                case Shape.Product product -> {
                    List<Type> types = ((Type.Tuple) type).of();
                    List<Both> members = new ArrayList<>();
                    for (int at = 0; at < types.size(); at++) {
                        Both member = both(types.get(at), product.of().get(at));
                        if (member == null) {
                            yield null;
                        }
                        members.add(member);
                    }
                    yield new Tuple(members);
                }
                case Shape.ListOf list -> {
                    Both element = both(((Type.ListOf) type).of(), list.element());
                    yield element == null ? null : new Listed(element, module.lists().stream()
                            .filter(it -> it.element().equals(list.element())).findFirst()
                            .orElseThrow());
                }
                case Shape.FunctionOf function -> {
                    Type.Function fn = (Type.Function) type;
                    List<Both> takes = new ArrayList<>();
                    for (int at = 0; at < fn.takes().size(); at++) {
                        Both taken = both(fn.takes().get(at), function.signature().takes().get(at));
                        if (taken == null) {
                            yield null;
                        }
                        takes.add(taken);
                    }
                    Both answers = both(fn.answers(), function.signature().answers());
                    yield answers == null ? null : hosted(new Callable(takes, answers,
                            module.functions().stream()
                                    .filter(it -> it.signature().equals(function.signature()))
                                    .findFirst().orElseThrow(), bindingClass()));
                }
            };
        }
    }

    /**
     * {@code callable}, with the slot the binding keeps for a closure of its type, which it is
     * written the first time it is asked for.
     */
    private Callable hosted(Callable callable) {
        hosting.putIfAbsent(callable.slot(), callable);
        return callable;
    }

    /**
     * How PHP holds a value that is one word of the library's, of the declared type {@code
     * module.name}, as the class generated for it, or null where there is none.
     */
    private @Nullable Whole whole(String module, String name) {
        Declared it = declared.get(module + "." + name);
        if (it == null) {
            return null;
        }
        return it.declaration() instanceof Declaration.Sum
                ? Whole.sum(it.fqcn(), it.codec()) : Whole.product(it.fqcn());
    }

    /**
     * What each member of {@code union} crosses as, or null where PHP has no way to hold one: a
     * declared type as its class, a primitive as PHP's own type carried into the union, and a case
     * the language gives no way, since no class of this binding's is one.
     */
    private @Nullable List<Member> members(Type.Union union) {
        List<Member> members = new ArrayList<>();
        for (Case member : union.cases()) {
            Member it = switch (member) {
                case Case.Declared d -> whole(d.module(), d.name()) instanceof Whole w
                        ? new Member(w, null) : null;
                case Case.Primitive p -> carried(p);
                case Case.Language l -> null;
            };
            if (it == null) {
                return null;
            }
            members.add(it);
        }
        return members;
    }

    /**
     * A primitive case as PHP's own type for it, made and read through what the manifest names
     * for it, or null where PHP has none. Said for every primitive of a union that crosses, which
     * the manifest was refused for leaving out, so a missing one is never read here as PHP having
     * no way to hold it.
     */
    private @Nullable Member carried(Case.Primitive of) {
        Manifest.CaseCrossing crossing = manifest.crossing(of);
        Word held = crossing.holds();
        Whole whole = held == null ? null : Whole.primitive(held);
        return whole == null ? null : new Member(whole, crossing);
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


    // ---------------------------------------------------------------------------------------------
    // A module.

    private void module(Manifest.Module module) throws IOException {
        String namespace = PhpNames.moduleNamespace(root, module.name());
        PhpNames.Claimed classes = PhpNames.Claimed.classes("namespace " + namespace);
        coreClasses(module).forEach(it -> classes.claim(it.getKey(), it.getValue()));
        // Each left with a name no other class here is ({@link #classes}), so these never refuse.
        for (BehaviorClass it : behaviorClasses.values()) {
            if (it.module().equals(module.name())) {
                classes.claim(it.className(), "the class of behavior `" + it.key() + "`");
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
        for (Manifest.Injection injection : module.injections()) {
            BehaviorClass it = behaviorClasses.get(module.name() + "." + injection.name());
            if (it != null) {
                injectedClass(module, injection, it);
            }
        }
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
            case Case.Primitive p -> "primitive:" + p.name();
            case Case.Language l -> "language:" + l.name();
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
        if (it.declaration() instanceof Declaration.Sum) {
            throw new IllegalStateException("a sum has no class");
        }
        List<Manifest.Field> fields = it.declaration().fields();
        Manifest.@Nullable Construct construct = Declaration.built(it.declaration());

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
        decode(php, it, it.declaration().decode(), it.declaration().decodeHost(), it.fqcn(),
                "new " + it.fqcn() + "($session->held($value))");
        for (Manifest.Field field : fields) {
            getter(php, it, field);
        }
        Function encode = it.declaration().encode();
        if (encode != null) {
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

    /**
     * What a function of the binding finds the session it is called in with: the innermost run
     * going of its library, which the generated {@code Binding} answers.
     */
    private String innermost() {
        return bindingClass() + "::session()";
    }

    /** The binding this generator writes, as PHP names the class. */
    private String bindingClass() {
        return "\\" + root + "\\Binding";
    }

    /** The static constructor: the value, or the invariant it does not hold as an issue. */
    private void of(StringBuilder php, Declared it, List<Manifest.Field> fields,
                    Manifest.Construct construct) {
        List<Given> crossings = in(it.module())
                .givens(fields.stream().map(Manifest.Field::type).toList(), construct.takes());
        if (crossings == null) {
            return;
        }
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
        // An optional takes null where nothing after it has to be named: PHP reads a default
        // before a parameter that has none as a mistake.
        int defaulted = crossings.size();
        while (defaulted > 0 && crossings.get(defaulted - 1).nullable()) {
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
                        $%s = %s;
                        $%s = $%s->call();
                        $%s = $%s->new('%s');
                        $%s = $%s->%s(%s);
                        return $%s->constructed($%s,
                            static fn (): %s => new %s($%s->held($%s)));
                    }
                """.formatted(it.key(), described, it.fqcn(), String.join(", ", parameters),
                session, innermost(), ffi, session, made, ffi,
                Crossing.storage(Word.VALUE), status, ffi, construct.function().name(),
                String.join(", ", given), session, status, it.fqcn(), it.fqcn(), session, made));
    }

    /**
     * Reading a value of the type: out of text in its external form ({@code decode}), and out of a
     * PHP value ({@code decoder}).
     *
     * <p>Two functions of the library's and not one. Text says of every container whether it is an
     * object or an array; a PHP array does not, since a list is the array keyed by its indices and
     * the empty list is the empty array. So a PHP value is handed over through the library's reading
     * of a host's value ({@code decodeHost}), which takes a map keyed by its indices as an array
     * where the declaration holds one, and never through text it would have to be guessed into.
     */
    private void decode(StringBuilder php, Declared it, @Nullable Function decode,
                        @Nullable Function decodeHost, String answers, String made) {
        if (decode == null && decodeHost == null) {
            return;
        }
        if (decode == null || decodeHost == null) {
            throw new IllegalStateException("the manifest says `" + it.key() + "` is read out of"
                    + (decode == null ? " a host's value and not out of text"
                    : " text and not out of a host's value") + ", and the two are emitted together");
        }
        php.append("""

                    /**
                     * A value of `%s` read out of its external form, or the issues found in it.
                     *
                     * @return \\Raoh\\Result<%s>
                     */
                    public static function decode(string $json): \\Raoh\\Result
                    {
                %s
                    }

                    /**
                     * A raoh-php decoder of `%s`, to compose with a host's own: what it is handed is a
                     * PHP value, read as a value of the type, and what is wrong in it is an issue at
                     * the path the decoder is reached at. An array is read as whatever the position
                     * holds, an object or a list, as PHP makes no difference between the two where
                     * the array is empty or keyed by its indices.
                     *
                     * @return \\Raoh\\Decoder<mixed, %s>
                     */
                    public static function decoder(): \\Raoh\\Decoder
                    {
                        return %s::decoder(static function (string $json): \\Raoh\\Result {
                %s
                        });
                    }
                """.formatted(it.key(), answers, reading(decode, "        ", answers, made), it.key(),
                answers, RUNTIME + "Session", reading(decodeHost, "            ", answers, made)));
    }

    /**
     * The body that reads `$json` through {@code function}, one of a type's two readings, and
     * answers what the reading came to: written once for both, each line indented by {@code indent}.
     */
    private String reading(Function function, String indent, String answers, String made) {
        return """
                $session = %s;
                $ffi = $session->call();
                $reading = $ffi->new('%s');
                $status = $ffi->%s($session->bytes($json), \\strlen($json), \\FFI::addr($reading));
                return $session->decoded($status, $reading,
                    static fn (\\FFI\\CData $value): %s => %s);""".formatted(innermost(),
                Crossing.storage(function.takes().getLast().word()), function.name(), answers, made)
                .lines().map(line -> indent + line)
                .collect(Collectors.joining("\n"));
    }

    private void getter(StringBuilder php, Declared it, Manifest.Field field) {
        // Whether there is a reader first: a field with none is one no function hands across, and
        // nothing is asked of how it would cross.
        Manifest.Read read = field.read().available();
        if (read == null) {
            return;
        }
        Received crossing = in(it.module()).received(field.type(), read.answers());
        if (crossing == null) {
            return;
        }
        List<Word> words = crossing.words();
        StringBuilder body = new StringBuilder();
        List<String> rooms = new ArrayList<>();
        List<String> handed = new ArrayList<>(List.of("$value"));
        for (int at = 0; at < words.size(); at++) {
            String room = "$room" + (words.size() == 1 ? "" : at);
            rooms.add(room);
            handed.add("\\FFI::addr(" + room + ")");
            body.append(room).append(" = $ffi->new('").append(Crossing.storage(words.get(at)))
                    .append("');\n        ");
        }
        body.append("$ffi->").append(read.function().name()).append("(")
                .append(String.join(", ", handed)).append(");\n        return ")
                .append(crossing.of(crossing.fromRooms(rooms), "$session")).append(";");
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
        decode(codec, it, sum.decode(), sum.decodeHost(), it.fqcn(),
                it.codec() + "::wrap($session, $value)");
        Function encode = sum.encode();
        if (encode != null) {
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
            Manifest.Call call = behavior.call().available();
            if (call == null) {
                continue;
            }
            Crossings crossings = in(module.name());
            List<Given> takes = crossings.givens(behavior.parameters().types(),
                    call.signature().takes());
            Received answers = crossings.received(behavior.answers(), call.signature().answers(),
                    module.name() + "." + behavior.name());
            if (takes == null || answers == null) {
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
            String session = PhpNames.freeOf("session", names);
            String requirements = requiresOf(module.name() + "." + behavior.name()).isEmpty() ? "null"
                    : bindingClass() + "::in($" + session + "->library())->requirementsOf($" + session
                    + ", '" + quotedInSingle(module.name() + "." + behavior.name()) + "')";
            functions.append(call(what, "public static function " + behavior.name(), names, takes,
                    answers, call.function(), requirements));
            BehaviorClass it = behaviorClasses.get(module.name() + "." + behavior.name());
            if (it != null) {
                behaviorClass(it, behavior, names, takes, answers);
            }
        }
        if (functions.isEmpty()) {
            return;
        }
        StringBuilder php = header(namespace);
        php.append(doc("", "The behaviors `" + module.name() + "` publishes, each called in the"
                + " innermost run going and answering its value, or throwing where the computation"
                + " ends without one."));
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
            Manifest.Call read = value.read().available();
            if (read == null) {
                continue;
            }
            Received answers = in(module.name()).received(value.type(),
                    read.signature().answers());
            if (answers == null) {
                continue;
            }
            String what = "value `" + module.name() + "." + value.name() + "`";
            members.claim(PhpNames.memberName(value.name(), what), what);
            functions.append(call(what, "public static function " + value.name(), List.of(),
                    List.of(), answers, read.function(), null));
        }
        if (functions.isEmpty()) {
            return;
        }
        StringBuilder php = header(namespace);
        php.append(doc("", "The values `" + module.name() + "` publishes, each read in the innermost"
                + " run going."));
        php.append("final class Values\n{\n");
        php.append("""
                    private function __construct()
                    {
                    }
                """);
        php.append(functions).append("}\n");
        file(namespace, "Values", php);
    }

    /**
     * A function, declared as {@code declared}, calling {@code function} in the innermost run going
     * and answering what it wrote. {@code requirements} is what a behavior is called with first, as
     * PHP works it out in that run, and null for a value, which is called with nothing more.
     */
    private String call(String what, String declared, List<String> names, List<Given> takes,
                        Received answers, Function function, @Nullable String requirements) {
        List<Word> rooms = answers.words();
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
        List<String> given = new ArrayList<>();
        if (requirements != null) {
            given.add(requirements);
        }
        for (int at = 0; at < takes.size(); at++) {
            parameters.add(takes.get(at).phpType() + " $" + names.get(at));
            given.addAll(takes.get(at).given("$" + names.get(at), "$" + session));
        }
        StringBuilder body = new StringBuilder();
        body.append("        $").append(session).append(" = ").append(innermost()).append(";\n");
        body.append("        $").append(ffi).append(" = $").append(session).append("->call();\n");
        for (int at = 0; at < rooms.size(); at++) {
            body.append("        $").append(roomNames.get(at)).append(" = $").append(ffi)
                    .append("->new('").append(Crossing.storage(rooms.get(at))).append("');\n");
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
                    %s(%s): %s
                    {
                %s    }
                """.formatted(declared, String.join(", ", parameters), answers.phpType(), body);
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
            Crossings crossings = in(module.name());
            List<String> types = Objects.requireNonNull(crossings.receiveds(
                            injection.parameters().stream().map(Manifest.NamedParameter::type)
                                    .toList(), injection.signature().takes())).stream()
                    .map(Crossing::phpDocType).toList();
            described.add("     * @param (callable(" + String.join(", ", types) + "): "
                    + Objects.requireNonNull(crossings.given(injection.answers(),
                            injection.signature().answers())).phpDocType() + ")|null $" + name);
        }
        if (parameters.isEmpty()) {
            return;
        }
        StringBuilder php = header(namespace);
        php.append(doc("", "Implementations of the behaviors `" + module.name() + "` asks a host to"
                + " implement, handed to a run. Each is called in the innermost run going with what"
                + " the behavior takes; an exception it throws comes back out of the call into the"
                + " library that reached it."));
        php.append("final class Injections extends \\Souther\\Runtime\\Injections\n{\n");
        php.append("""
                    /**
                %s
                     */
                    public static function of(%s): self
                    {
                        $given = array_filter([%s], static fn (?callable $it): bool => $it !== null);
                        return new self(%s::class,
                            array_map(static fn (callable $it): \\Closure => \\Closure::fromCallable($it), $given));
                    }
                }
                """.formatted(String.join("\n", described), String.join(", ", parameters),
                String.join(", ", entries), bindingClass()));
        file(namespace, "Injections", php);
    }

    /**
     * What the runtime hands what C passes an implementation of {@code injection} through: PHP
     * values made of each word, the implementation called with them, and its answer written through
     * the room C handed over. Null where a host could not be handed what it takes or hand back
     * what it answers.
     */
    private @Nullable String adapter(Manifest.Module module, Manifest.Injection injection) {
        Crossings crossings = in(module.name());
        List<Received> takes = crossings.receiveds(injection.parameters().stream()
                .map(Manifest.NamedParameter::type).toList(), injection.signature().takes());
        Given answers = crossings.given(injection.answers(), injection.signature().answers());
        if (takes == null || answers == null) {
            return null;
        }
        return adapting(takes, answers, module.name() + "." + injection.name());
    }

    /**
     * What the runtime hands what C passes a function a host wrote through, the function taking
     * {@code takes} and answering {@code answers}: PHP values made of each word C handed over, the
     * implementation called with them, and what it answered written through the room C handed over,
     * word for word. What it was handed where it was laid out comes first, and the runtime takes it
     * off before this is handed the rest. {@code what} is what the function answers for, for the
     * error an answer of another type is.
     */
    private static String adapting(List<Received> takes, Given answers, String what) {
        List<String> arguments = new ArrayList<>();
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
                quotedInSingle(what), quotedInSingle(answers.phpType()), written);
    }

    // ---------------------------------------------------------------------------------------------
    // A behavior as an application holds one.

    /**
     * The class an application extends to implement {@code injection}: an abstract {@code apply}
     * typed as the model says, which the library calls in the innermost run going, the way it calls
     * a closure handed to {@code Injections}.
     */
    private void injectedClass(Manifest.Module module, Manifest.Injection injection,
                               BehaviorClass it) throws IOException {
        Crossings crossings = in(module.name());
        // Named as the model names them where PHP takes that: nothing else publishes these names,
        // and an override is not held to them.
        List<String> names = PhpNames.ownParameters(injection.parameters().stream()
                .map(Manifest.NamedParameter::name).toList(), "input");
        List<Received> takes = Objects.requireNonNull(crossings.receiveds(injection.parameters()
                .stream().map(Manifest.NamedParameter::type).toList(),
                injection.signature().takes()));
        Given answers = Objects.requireNonNull(crossings.given(injection.answers(),
                injection.signature().answers()));
        List<String> parameters = new ArrayList<>();
        List<String> described = new ArrayList<>();
        for (int at = 0; at < takes.size(); at++) {
            parameters.add(takes.get(at).phpType() + " $" + names.get(at));
            described.addAll(paramTag(takes.get(at), names.get(at)));
        }
        StringBuilder php = header(it.namespace());
        php.append(doc("", "What implements `" + it.key() + "`, which the library asks a host to"
                + " implement. An instance is handed to what is bound to it, and the library calls"
                + " its `apply` wherever what was bound to it reaches the behavior."));
        php.append("abstract class ").append(it.className()).append("\n{\n");
        php.append(docLines(List.of("Answers `" + it.key() + "`, in the innermost run going. An"
                + " exception thrown here comes back out of the call into the library that reached"
                + " it."), described, answers));
        php.append("    abstract public function apply(").append(String.join(", ", parameters))
                .append("): ").append(answers.phpType()).append(";\n}\n");
        file(it.namespace(), it.className(), php);
    }

    /**
     * The class an application binds {@code behavior} through and calls it on: {@code bind}, taking
     * an implementation of each behavior it requires, or {@code of} where it requires none, and
     * {@code apply}, calling it with the capabilities of what it was bound to.
     *
     * <p>{@code apply} is called in the caller's run, as every function a binding writes is, and
     * does not open a run of its own: what it answers is a value of the caller's run, and a run it
     * opened would have ended by the time the caller held it. The class is callable too, so an
     * application holding one calls it as it calls any PHP function.
     */
    private void behaviorClass(BehaviorClass it, Manifest.Behavior behavior, List<String> names,
                               List<Given> takes, Received answers) throws IOException {
        String session = PhpNames.freeOf("session", names);
        List<String> parameters = new ArrayList<>();
        List<String> arguments = new ArrayList<>();
        List<String> described = new ArrayList<>();
        for (int at = 0; at < takes.size(); at++) {
            parameters.add(takes.get(at).phpType() + " $" + names.get(at));
            arguments.add("$" + names.get(at));
            described.addAll(paramTag(takes.get(at), names.get(at)));
        }

        StringBuilder php = header(it.namespace());
        php.append(doc("", "`" + it.key() + "` as an application holds it: bound to an"
                + " implementation of each behavior it requires, which each call is made with."));
        php.append("final class ").append(it.className()).append("\n{\n");
        php.append("""
                    private function __construct(private readonly \\Souther\\Runtime\\Bound $bound)
                    {
                    }
                """);
        php.append(construction(it, behavior));
        php.append("""

                    /** @internal What this was bound to, for what requires it. */
                    public function bound(): \\Souther\\Runtime\\Bound
                    {
                        return $this->bound;
                    }
                """);
        php.append(call("`" + it.key() + "` with what this was bound to", "public function apply",
                names, takes, answers, Objects.requireNonNull(behavior.call().available()).function(),
                "$this->bound->requirements($" + session + ")"));
        php.append("\n");
        php.append(docLines(List.of("`apply`, for calling this as a function."), described, answers));
        php.append("""
                    public function __invoke(%s): %s
                    {
                        return $this->apply(%s);
                    }
                }
                """.formatted(String.join(", ", parameters), answers.phpType(),
                String.join(", ", arguments)));
        file(it.namespace(), it.className(), php);
    }

    /**
     * How an application makes {@code it}: {@code bind}, taking one implementation of each behavior
     * {@code behavior} requires, in order, or {@code of}, where it requires nothing. A behavior a host
     * implements stands as the instance handed over, and one constructed in turn as what it was
     * bound to.
     */
    private String construction(BehaviorClass it, Manifest.Behavior behavior) {
        List<Manifest.Required> requires = requiresOf(it.key());
        String bind = bindOf(it.key());
        if (requires.isEmpty()) {
            return """

                        /** `%s`, which requires nothing. */
                        public static function of(): self
                        {
                            return new self(\\Souther\\Runtime\\Bound::of(%s));
                        }
                    """.formatted(it.key(), bind);
        }
        // A requirement is its module and its name, and two of one name from two modules are two
        // requirements (a composition over `a.load` and `b.load`), so a parameter is named after
        // the name only where no other is, and after its place otherwise.
        List<String> names = PhpNames.ownParameters(
                requires.stream().map(Manifest.Required::name).toList(), "dependency");
        List<String> parameters = new ArrayList<>();
        List<String> handed = new ArrayList<>();
        handed.add(bind);
        for (int at = 0; at < requires.size(); at++) {
            Manifest.Required required = requires.get(at);
            BehaviorClass of = behaviorClasses.get(required.key());
            String name = names.get(at);
            parameters.add(of.fqcn() + " $" + name);
            handed.add(of.injected()
                    ? "\\Souther\\Runtime\\Implemented::by(" + bindingClass() + "::class, '"
                            + quotedInSingle(required.key()) + "', $" + name + "->apply(...))"
                    : "$" + name + "->bound()");
        }
        return """

                    /** `%s`, bound to an implementation of each behavior it requires. */
                    public static function bind(%s): self
                    {
                        return new self(\\Souther\\Runtime\\Bound::of(%s));
                    }
                """.formatted(it.key(), String.join(", ", parameters), String.join(", ", handed));
    }

    /**
     * The {@code @param} tag of a docblock for {@code crossing} under {@code name}, where the
     * docblock says more of it than its PHP type does, and none where it does not.
     */
    private static List<String> paramTag(Crossing crossing, String name) {
        return crossing.phpDocType().equals(crossing.phpType()) ? List.of()
                : List.of("@param " + crossing.phpDocType() + " $" + name);
    }

    // ---------------------------------------------------------------------------------------------
    // The binding, and loading it.

    private void binding() throws IOException {
        StringBuilder slots = new StringBuilder();
        StringBuilder constructions = new StringBuilder();
        List<String> injected = new ArrayList<>();
        for (Manifest.Module module : manifest.modules()) {
            for (Manifest.Injection injection : module.injections()) {
                injected.add("'" + quotedInSingle(module.name() + "." + injection.name()) + "'");
                String adapter = adapter(module, injection);
                if (adapter == null) {
                    continue;
                }
                slots.append("            '").append(module.name()).append('.')
                        .append(injection.name()).append("' => new \\Souther\\Runtime\\InjectionSlot(")
                        .append("$library, '").append(injection.implementation().type()).append("', '")
                        .append(injection.implement()).append("',\n                ").append(adapter)
                        .append("),\n");
            }
            // Every behavior a host constructs, whether or not it calls it by name: a behavior the
            // module keeps that a published one depends on has no function and no class here, and
            // is built all the same where the published one is called.
            for (Manifest.Construction construction : module.constructions()) {
                String key = module.name() + "." + construction.name();
                String requires = construction.requires().stream()
                        .map(it -> "'" + quotedInSingle(it.key()) + "'")
                        .collect(Collectors.joining(", ", "[", "]"));
                constructions.append("        '").append(quotedInSingle(key)).append("' => [")
                        .append(bindOf(key)).append(", ").append(requires).append("],\n");
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
                     * What makes a capability of each behavior, where something may require it, and
                     * the behaviors it requires, in order: what a run constructs what it calls from.
                     */
                    private const CONSTRUCTIONS = [
                %s    ];

                    /**
                     * Every behavior a host implements, as the library says, whether or not this binding
                     * adapts an implementation of it.
                     */
                    private const INJECTED = [%s];
                """.formatted(constructions, String.join(", ", injected)));
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

                    /** @internal This binding, as it was loaded for `$library`. */
                    public static function in(\\Souther\\Runtime\\NativeLibrary $library): static
                    {
                        return self::$bindings[spl_object_id($library)]
                            ?? throw new \\LogicException('this binding was not loaded for that library');
                    }

                    /**
                     * @internal The session every function of this binding is called in: the
                     * innermost run going on this fiber of a library it was loaded for.
                     */
                    public static function session(): \\Souther\\Runtime\\Session
                    {
                        return self::innermostOf(self::$bindings);
                    }

                    /**
                     * One binding for each library, made the first time it is asked for: what each
                     * implementation is called through is made into a C entry here, and PHP keeps
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
                %s        ], [
                %s        ], self::CONSTRUCTIONS, self::INJECTED);
                    }
                }
                """.formatted(DECLARATIONS, DECLARATIONS, RUNTIME_PROTOCOL, RUNTIME_PROTOCOL, slots,
                functionSlots()));
        file(root, "Binding", php);
    }

    /**
     * What the library calls each closure PHP hands it as a function value through, one slot for
     * each function type as PHP holds it, as the members of a PHP array.
     */
    private String functionSlots() {
        StringBuilder slots = new StringBuilder();
        for (Callable callable : hosting.values()) {
            slots.append("            '").append(quotedInSingle(callable.slot()))
                    .append("' => new \\Souther\\Runtime\\FunctionSlot($library, '")
                    .append(callable.crossing().implementation().type()).append("', '")
                    .append(callable.crossing().implement()).append("',\n                ")
                    .append(adapting(List.copyOf(callable.takes()), callable.answers(),
                            "a function value of " + callable.phpDocType()))
                    .append("),\n");
        }
        return slots.toString();
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
