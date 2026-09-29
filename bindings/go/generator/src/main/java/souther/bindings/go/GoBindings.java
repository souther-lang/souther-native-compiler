package souther.bindings.go;

import org.jspecify.annotations.Nullable;
import souther.bindings.BindingInput;
import souther.bindings.Generated;
import souther.bindings.Manifest;
import souther.bindings.Manifest.Case;
import souther.bindings.Manifest.Declaration;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.Word;
import souther.bindings.NotBindable;
import souther.bindings.Output;
import souther.bindings.RuntimeFunctions;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.Objects;
import java.util.Set;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;

/**
 * Writes the Go packages a host calls a library through, from the manifest the build wrote beside
 * it.
 *
 * <p>Reads the manifest and nothing else, as the PHP and the Rust generators do. What it writes is
 * the manifest's surface as Go: a package for each module of the model, a handle for each published
 * type and a function for each behavior a host can call, over the runtime module in
 * {@code bindings/go/runtime}, which is where a run's thread, loading the library and what each
 * status means are kept. The library is loaded by path at run time and every function is called
 * through the address its file has for it, so nothing is linked: every Souther library exports the
 * same runtime functions, and two libraries in one program each keep their own.
 *
 * <p>What a host has no way to reach is not written: a behavior the manifest says is unavailable, a
 * field with no {@code read}, a type Go has no representation for here. How a value crosses is read
 * off the manifest's shape for it, and what is decided here is only how Go holds it
 * ({@link Crossing}).
 */
public final class GoBindings {

    /** What says a directory is a Go binding this wrote, and may be replaced whole. */
    static final String MARK = ".souther-go-binding";

    /** The version of the runtime module what this writes requires. */
    static final String RUNTIME_VERSION = "v0.1.0";

    static final String RUNTIME_MODULE_PATH = "github.com/souther-lang/souther-native-compiler/bindings/go/runtime";

    private static final String RUNTIME_MODULE = RUNTIME_MODULE_PATH;

    private final Manifest manifest;
    private final BindingInput input;
    private final String importPath;
    private final Path into;
    private final List<Path> written = new ArrayList<>();

    /** What each declared type is, by {@code module.Name}. */
    private final Map<String, Declared> declared = new LinkedHashMap<>();

    /** Each module of the model, by its name. */
    private final Map<String, GoModule> modules = new LinkedHashMap<>();

    /** Every function of the library the binding calls, by its symbol. */
    private final TreeSet<String> symbols = new TreeSet<>();

    private GoBindings(BindingInput input, String importPath, Path into) {
        this.input = input;
        this.manifest = input.manifest();
        this.importPath = importPath;
        this.into = into;
    }

    /**
     * Refuses what a generation into {@code into} as {@code importPath} would refuse whatever the
     * manifest said: a path Go will not take, and a directory holding what no generation wrote.
     *
     * @throws NotBindable where the path or the directory would be refused
     */
    public static void refuseAhead(Path into, String importPath) throws IOException {
        GoNames.importPath(importPath);
        Output.replaceable(into, MARK);
    }

    /**
     * Writes the binding of what the input's manifest describes into {@code into}, as the package
     * {@code importPath}, which a host depends on by path.
     *
     * <p>{@code into} is then that package and nothing else: it is written beside it and put in
     * place whole ({@link Output}), so a module the model no longer declares does not survive a
     * generation, and a refused one leaves what was there as it was.
     *
     * @throws NotBindable where a name in the model is not one Go takes
     */
    public static Generated generate(BindingInput input, Path into, String importPath)
            throws IOException {
        String path = GoNames.importPath(importPath);
        Output output = Output.replacing(into, MARK);
        GoBindings binding = new GoBindings(input, path, output.staging());
        try {
            binding.write();
            output.commit();
        } catch (IOException | RuntimeException e) {
            output.abandon();
            throw e;
        }
        return new Generated(output.placed(output.staging()),
                binding.written.stream().map(output::placed).toList());
    }

    /** One module of the model: the package it is written as, and the Go written in it. */
    private final class GoModule {
        final Manifest.Module module;
        final List<String> path;
        final String importPath;
        final Body.Imports imports;
        final GoNames.Claimed names;
        final StringBuilder items = new StringBuilder();
        final Map<String, Function> shims = new LinkedHashMap<>();

        GoModule(Manifest.Module module, List<String> path) {
            this.module = module;
            this.path = path;
            this.importPath = GoBindings.this.importPath + "/" + String.join("/", path);
            this.imports = new Body.Imports(GoBindings.this.importPath, importPath);
            this.names = new GoNames.Claimed("the package of module `" + module.name() + "`");
        }

        /** The name of the C function that calls {@code function} through its address. */
        String shim(Function function) {
            shims.putIfAbsent(function.name(), function);
            symbols.add(function.name());
            return "C.call_" + function.name();
        }
    }

    private void write() throws IOException {
        RuntimeFunctions.check(manifest, "Go");
        Map<List<String>, String> directories = new LinkedHashMap<>();
        for (Manifest.Module module : manifest.modules()) {
            List<String> path = GoNames.modulePath(module.name());
            String before = directories.putIfAbsent(path, module.name());
            if (before != null) {
                throw new NotBindable("modules `" + before + "` and `" + module.name()
                        + "` are both the package " + String.join("/", path));
            }
            GoModule at = new GoModule(module, path);
            modules.put(module.name(), at);
            for (Declaration declaration : module.declarations()) {
                String what = "type `" + module.name() + "." + declaration.name() + "`";
                String name = at.names.claim(GoNames.exported(declaration.name(), what), what);
                Declared it = new Declared(module.name(), declaration, at.importPath, name);
                declared.put(it.key(), it);
            }
        }
        for (GoModule at : modules.values()) {
            module(at);
        }
        for (GoModule at : modules.values()) {
            moduleFile(at);
        }
        library();
        binding();
        goMod();
    }

    // ---------------------------------------------------------------------------------------------
    // What a model type crosses as.

    /**
     * How a value of {@code type} crosses in {@code shape}, the way {@code way} says, in a function
     * of {@code module}'s, or null where this binding has no way to hold it: a pair of a type and a
     * shape it knows no way to hold, a declared type it has no handle for, a union no declaration
     * names where nothing says which case it is, and a function value.
     */
    private @Nullable Crossing crossing(Manifest.Module module, Type type, Shape shape,
                                        Manifest.Way way) {
        Crossing made = switch (shape) {
            case Shape.Leaf leaf -> switch (type) {
                case Type.Primitive it -> Crossing.Whole.primitive(it.name(), leaf.word());
                case Type.Declared it -> leaf.word() == Word.VALUE ? handle(it.module(), it.name())
                        : null;
                // Handed to Go only as a behavior's answer, which says which case it is
                // (`answered`): anywhere else Go would be handed a value of it told nothing.
                case Type.Union union -> leaf.word() == Word.VALUE && way == Manifest.Way.GIVEN
                        ? oneOf(module, union, null) : null;
                default -> null;
            };
            case Shape.Option option -> type instanceof Type.Option it
                    && crossing(module, it.of(), option.of(), way) instanceof Crossing of
                    ? new Crossing.Optional(of) : null;
            case Shape.Product product -> {
                if (!(type instanceof Type.Tuple it) || it.of().size() != product.of().size()
                        || product.of().size() > Crossing.Tuple.MOST) {
                    yield null;
                }
                List<Crossing> members = crossings(module, it.of(), product.of(), way);
                yield members == null ? null : new Crossing.Tuple(members);
            }
            case Shape.ListOf list -> {
                if (!(Type.listed(type) instanceof Type of)
                        || !(crossing(module, of, list.element(), way) instanceof Crossing element)) {
                    yield null;
                }
                Manifest.ListCrossing listed = module.lists().stream()
                        .filter(l -> l.element().equals(list.element())).findFirst().orElseThrow();
                if (way == Manifest.Way.GIVEN ? listed.construct() == null : listed.read() == null) {
                    yield null;
                }
                yield new Crossing.Listed(element, listed.construct(), listed.read());
            }
            case Shape.FunctionOf function -> null;
        };
        if (made != null && !made.shape().equals(shape)) {
            throw new IllegalStateException("this binding holds a value crossing as " + shape
                    + " as what crosses as " + made.shape());
        }
        return made;
    }

    /**
     * How each of {@code types} crosses in its shape, or null where any of them has no way, or the
     * two say different counts.
     */
    private @Nullable List<Crossing> crossings(Manifest.Module module, List<Type> types,
                                               List<Shape> shapes, Manifest.Way way) {
        if (types.size() != shapes.size()) {
            return null;
        }
        List<Crossing> made = new ArrayList<>();
        for (int at = 0; at < types.size(); at++) {
            Crossing it = crossing(module, types.get(at), shapes.get(at), way);
            if (it == null) {
                return null;
            }
            made.add(it);
        }
        return made;
    }

    /**
     * How the library hands Go what {@code behavior} answers, or null where it has no way to: a
     * union no declaration names as the type of the member the case the library says it is belongs
     * to, and anything else as a value of its type is handed.
     */
    private @Nullable Crossing answered(Manifest.Module module, Manifest.Answer answer, Shape shape) {
        if (!(answer.type() instanceof Type.Union union)) {
            return crossing(module, answer.type(), shape, Manifest.Way.HANDED);
        }
        Manifest.UnionAnswer told = Objects.requireNonNull(answer.union());
        if (!(shape instanceof Shape.Leaf leaf) || leaf.word() != Word.VALUE || told.which() == null) {
            return null;
        }
        return oneOf(module, union, told);
    }

    /** The interface each union no declaration names is written as, by its members, once it is asked for. */
    private final Map<List<Case>, @Nullable UnionType> unions = new LinkedHashMap<>();

    /** A union's interface: where it stands, and its members. */
    private record UnionType(String importPath, String name, List<Crossing.OneOf.Member> members) {
    }

    /**
     * {@code union} as the interface generated for it, written in {@code module}'s package the
     * first time it is asked for, and told its case as {@code told} says where it is handed to Go.
     * Null where a member has no way to be held: a declared type with no handle, a primitive Go
     * holds no way, a case the language gives, or two members that would be one type or an
     * interface whose name another name of the package already is.
     */
    private Crossing.@Nullable OneOf oneOf(Manifest.Module module, Type.Union union,
                                           Manifest.@Nullable UnionAnswer told) {
        UnionType made = unions.containsKey(union.cases()) ? unions.get(union.cases())
                : unionType(modules.get(module.name()), union);
        unions.put(union.cases(), made);
        if (made == null) {
            return null;
        }
        if (told == null) {
            return new Crossing.OneOf(made.importPath(), made.name(), made.members(), null);
        }
        List<Crossing.OneOf.Arm> arms = new ArrayList<>();
        for (Case of : told.cases()) {
            Crossing.OneOf.Member member = memberOf(union, made, of);
            if (member == null) {
                return null;
            }
            arms.add(new Crossing.OneOf.Arm(member));
        }
        return new Crossing.OneOf(made.importPath(), made.name(), made.members(),
                new Crossing.OneOf.Told(Objects.requireNonNull(told.which()), arms));
    }

    /**
     * The member a value the library says is the case {@code of} is made as: the member it is or
     * the member sum it is a case of; null where it is neither, a case the model keeps.
     */
    private Crossing.OneOf.@Nullable Member memberOf(Type.Union union, UnionType made, Case of) {
        for (int at = 0; at < union.cases().size(); at++) {
            if (union.cases().get(at).equals(of)) {
                return made.members().get(at);
            }
        }
        if (of instanceof Case.Declared leaf) {
            for (int at = 0; at < union.cases().size(); at++) {
                if (union.cases().get(at) instanceof Case.Declared d
                        && declared.get(d.module() + "." + d.name()) instanceof Declared it
                        && it.declaration() instanceof Declaration.Sum sum
                        && cases(sum).contains(leaf.module() + "." + leaf.name())) {
                    return made.members().get(at);
                }
            }
        }
        return null;
    }

    private @Nullable UnionType unionType(GoModule at, Type.Union union) {
        List<Crossing.OneOf.Member> members = new ArrayList<>();
        Set<String> variants = new java.util.HashSet<>();
        for (Case each : union.cases()) {
            Crossing.OneOf.Member member = switch (each) {
                case Case.Declared d -> {
                    Declared it = declared.get(d.module() + "." + d.name());
                    yield it == null ? null
                            : new Crossing.OneOf.Member(it.name(), Crossing.Whole.handle(it), null, null);
                }
                case Case.Primitive p -> {
                    Manifest.CaseCrossing crossing = manifest.crossing(p);
                    Word held = crossing.holds();
                    Crossing.Whole whole = held == null ? null : Crossing.Whole.primitive(p.name(), held);
                    if (whole == null) {
                        yield null;
                    }
                    yield new Crossing.OneOf.Member(p.name(), whole, crossing.make(),
                            Objects.requireNonNull(crossing.read()));
                }
                case Case.Language l -> null;
            };
            if (member == null || !variants.add(member.variant())) {
                return null;
            }
            members.add(member);
        }
        String name = members.stream().map(Crossing.OneOf.Member::variant)
                .collect(java.util.stream.Collectors.joining("Or"));
        if (at.names.has(name)) {
            return null;
        }
        at.names.claim(name, "the interface of a union");
                String marker = "is" + name;
        StringBuilder out = new StringBuilder();
        String what = union.cases().stream().map(it -> switch (it) {
            case Case.Declared d -> d.module() + "." + d.name();
            case Case.Primitive p -> p.name();
            case Case.Language l -> l.name();
        }).collect(java.util.stream.Collectors.joining(" | "));
        out.append("\n// ").append(name).append(" is a value of `").append(what)
                .append("`: one of its members. A type switch tells them apart.\n")
                .append("type ").append(name).append(" interface {\n\t").append(marker).append("()\n}\n");
        for (Crossing.OneOf.Member member : members) {
            String variant = at.names.claim(name + member.variant(), "the member `" + member.variant()
                    + "` of the union `" + what + "`");
            out.append("\n// ").append(variant).append(" is the member ").append(member.variant())
                    .append(" of `").append(what).append("`.\n").append("type ").append(variant)
                    .append(" struct {\n\tValue ").append(member.whole().type(at.imports))
                    .append("\n}\n\nfunc (").append(variant).append(") ").append(marker)
                    .append("() {}\n");
        }
        at.items.append(out);
        return new UnionType(at.importPath, name, members);
    }

    private Crossing.@Nullable Whole handle(String module, String name) {
        Declared it = declared.get(module + "." + name);
        return it == null ? null : Crossing.Whole.handle(it);
    }

    // ---------------------------------------------------------------------------------------------
    // A module.

    private void module(GoModule at) {
        for (Declaration declaration : at.module.declarations()) {
            handleType(at, declared.get(at.module.name() + "." + declaration.name()));
        }
        behaviors(at);
        values(at);
    }

    /** The struct a value of {@code it} is held as, with what reads it and makes it. */
    private void handleType(GoModule at, Declared it) {
        Declaration declaration = it.declaration();
        GoNames.Claimed methods = new GoNames.Claimed("the methods of `" + it.key() + "`");
        methods.claim("Encode", "the generated `Encode`");
        List<String> readers = new ArrayList<>();
        for (Manifest.Field field : declaration.fields()) {
            String what = "field `" + it.key() + "." + field.name() + "`";
            readers.add(methods.claim(GoNames.exported(field.name(), what), what));
        }
        at.items.append("\n// ").append(it.name()).append(" is a value of `").append(it.key())
                .append("`, held where the library made it. Its zero value holds none.\n")
                .append("type ").append(it.name()).append(" struct {\n")
                .append("\t// Ref__ is the value, and the run it was made in. It is what the binding\n")
                .append("\t// hands from one package to another and is nothing to read.\n")
                .append("\tRef__ ").append(at.imports.lib()).append(".Ref\n}\n");
        Manifest.Construct construct = Declaration.built(declaration);
        if (construct != null) {
            construct(at, it, construct);
        }
        decode(at, it, declaration.decode());
        encode(at, it, declaration.encode());
        for (int field = 0; field < declaration.fields().size(); field++) {
            reader(at, it, declaration.fields().get(field), readers.get(field));
        }
        if (declaration instanceof Declaration.Sum sum) {
            sum(at, it, sum);
        }
    }

    /**
     * One case of a sum as {@code Case} answers it: its variant, what it holds, and which case of
     * the model it is.
     *
     * @param holds the Go type of the value of the case, or null where it holds none
     * @param each  the case, or null for the one every case the model keeps is
     */
    private record Arm(String variant, @Nullable String holds, @Nullable Case each) {
    }

    /** The variant every case of a sum the model keeps is, holding the value as the sum. */
    private static final String KEPT = "Kept";

    /**
     * Each case of {@code sum} in the order its {@code which} counts them, or null where a case has
     * no variant this binding can write: where nothing says which case a value is, a primitive Go
     * holds no way, or two cases that would be one variant. A case the model keeps, a declared type
     * it does not publish, is {@value #KEPT}, holding the value as the sum.
     */
    private @Nullable List<Arm> arms(GoModule at, Declared of, Declaration.Sum sum) {
        if (sum.which() == null) {
            return null;
        }
        List<Arm> arms = new ArrayList<>();
        Set<String> variants = new java.util.HashSet<>();
        Arm kept = new Arm(KEPT, of.name(), null);
        for (Case each : sum.cases()) {
            Arm arm = switch (each) {
                case Case.Declared d -> {
                    Declared it = declared.get(d.module() + "." + d.name());
                    yield it == null ? kept : new Arm(it.name(),
                            at.imports.module(it.importPath()) + it.name(), each);
                }
                case Case.Primitive p -> {
                    Word held = manifest.crossing(p).holds();
                    Crossing.Whole whole = held == null ? null : Crossing.Whole.primitive(p.name(), held);
                    yield whole == null ? null : new Arm(p.name(), whole.type(at.imports), each);
                }
                case Case.Language l -> new Arm(GoNames.exported(l.name(), "case `" + l.name() + "`"),
                        null, each);
            };
            if (arm == null || !variants.add(arm.variant()) && arm != kept) {
                return null;
            }
            arms.add(arm);
        }
        if (arms.contains(kept)
                && arms.stream().anyMatch(it -> it != kept && it.variant().equals(KEPT))) {
            return null;
        }
        return arms;
    }

    private static Set<String> cases(Declaration.Sum sum) {
        return sum.cases().stream().map(it -> switch (it) {
            case Case.Declared d -> d.module() + "." + d.name();
            case Case.Primitive p -> "primitive:" + p.name();
            case Case.Language l -> "language:" + l.name();
        }).collect(java.util.stream.Collectors.toCollection(LinkedHashSet::new));
    }

    /**
     * {@code Case}: which case a value of the sum is, as the value of that case, of a type of its
     * own for each, and what makes a value of a case or of a narrower sum a value of this one.
     */
    private void sum(GoModule at, Declared it, Declaration.Sum sum) {
        List<Arm> arms = arms(at, it, sum);
        if (arms == null) {
            return;
        }
        String souther = at.imports.souther();
        String caseType = at.names.claim(it.name() + "Case", "the cases of `" + it.key() + "`");
        String marker = "is" + caseType;
        StringBuilder out = new StringBuilder();
        out.append("\n// ").append(caseType).append(" is the case a value of `").append(it.key())
                .append("` is, holding the value of that case. A type switch tells them apart.\n")
                .append("type ").append(caseType).append(" interface {\n\t").append(marker)
                .append("()\n}\n");
        for (Arm arm : new LinkedHashSet<>(arms)) {
            String variant = it.name() + arm.variant();
            at.names.claim(variant, "the case `" + arm.variant() + "` of `" + it.key() + "`");
            out.append("\n// ").append(variant).append(" is the case ").append(arm.variant())
                    .append(" of `").append(it.key()).append("`.\n").append("type ").append(variant)
                    .append(" struct");
            out.append(arm.holds() == null ? "{}\n" : " {\n\tValue " + arm.holds() + "\n}\n");
            out.append("\nfunc (").append(variant).append(") ").append(marker).append("() {}\n");
        }
        Body body = new Body(at.imports, at::shim, "run", "return", 1);
        body.line("value := v.Ref__.Read()");
        body.line("run := v.Ref__.Run()");
        body.line("switch " + at.shim(Objects.requireNonNull(sum.which())) + "(run.Library().Symbol(\""
                + sum.which().name() + "\"), value) {");
        for (int place = 0; place < arms.size(); place++) {
            Arm arm = arms.get(place);
            String variant = it.name() + arm.variant();
            body.line("case " + place + ":");
            String held = souther + ".NewRef(run, value)";
            if (arm.holds() == null) {
                body.line("\treturn " + variant + "{}");
            } else if (arm.each() instanceof Case.Primitive p) {
                Manifest.CaseCrossing crossing = manifest.crossing(p);
                Crossing.Whole whole = Crossing.Whole.primitive(p.name(), crossing.holds());
                Body inner = new Body(at.imports, at::shim, "run", "return", 2);
                String word = inner.temp("held");
                inner.line(word + " := " + at.shim(crossing.read()) + "(run.Library().Symbol(\""
                        + crossing.read().name() + "\"), value)");
                String made = whole.of(inner, List.of(word));
                inner.line("return " + variant + "{Value: " + made + "}");
                body.raw(inner.toString());
            } else {
                body.line("\treturn " + variant + "{Value: " + arm.holds() + "{Ref__: " + held + "}}");
            }
        }
        body.line("}");
        body.line("panic(\"the library answered a case `" + it.key() + "` does not have\")");
        out.append("\n// Case is which case this value is, as the value of that case.\n")
                .append("func (v ").append(it.name()).append(") Case() ").append(caseType)
                .append(" {\n").append(body).append("}\n");
        // A value of a case, or of a narrower sum, is a value of the sum as it is.
        Set<String> mine = cases(sum);
        for (Declared other : declared.values()) {
            boolean within = switch (other.declaration()) {
                case Declaration.Sum narrower -> other != it && mine.containsAll(cases(narrower))
                        && !cases(narrower).equals(mine);
                default -> mine.contains(other.key());
            };
            if (within) {
                String name = at.names.claim(it.name() + "From" + other.name(),
                        "the conversion of `" + other.key() + "` to `" + it.key() + "`");
                out.append("\n// ").append(name).append(" is a value of `").append(other.key())
                        .append("` as one of `").append(it.key()).append("`.\n")
                        .append("func ").append(name).append("(v ")
                        .append(at.imports.module(other.importPath())).append(other.name())
                        .append(") ").append(it.name()).append(" {\n\treturn ").append(it.name())
                        .append("{Ref__: v.Ref__}\n}\n");
            }
        }
        at.items.append(out);
    }

    /** {@code New<Type>}: the value, or the invariant it does not hold as an issue. */
    private void construct(GoModule at, Declared it, Manifest.Construct construct) {
        List<Manifest.Field> fields = it.declaration().fields();
        List<Crossing> takes = crossings(at.module,
                fields.stream().map(Manifest.Field::type).toList(), construct.takes(),
                Manifest.Way.GIVEN);
        if (takes == null) {
            return;
        }
        String what = "the constructor of `" + it.key() + "`";
        String name = at.names.claim("New" + it.name(), what);
        GoNames.Claimed claimed = new GoNames.Claimed("the parameters of " + what);
        List<String> names = new ArrayList<>();
        for (Manifest.Field field : fields) {
            names.add(claimed.claim(GoNames.local(field.name(),
                    "field `" + it.key() + "." + field.name() + "`"), "field `" + field.name() + "`"));
        }
        Crossing made = Crossing.Whole.handle(it);
        at.items.append(function(at, "// " + name + " is a value of `" + it.key() + "`, or an"
                + " invariant_violation issue where what is handed over does not hold what the type"
                + " states.", "func " + name, names, takes, made, construct.function(), false, true));
    }

    /** {@code Decode<Type>}: a value of the type read out of its external form, or the issues found in it. */
    private void decode(GoModule at, Declared it, @Nullable Function decode) {
        if (decode == null) {
            return;
        }
        String name = at.names.claim("Decode" + it.name(), "the reader of `" + it.key() + "`");
        String souther = at.imports.souther();
        String unsafe = at.imports.unsafe();
        Body body = new Body(at.imports, at::shim, "r", "return " + it.name() + "{}, err", 1);
        body.line("fn := r.Library().Symbol(\"" + decode.name() + "\")");
        body.line("var reading " + unsafe + ".Pointer");
        body.line("failed := " + souther + ".Call(r, func() " + souther + ".Status {");
        body.line("\treturn " + souther + ".Status(" + at.shim(decode) + "(fn, " + souther
                + ".Bytes(json), C.int64_t(len(json)), &reading))");
        body.line("})");
        body.open("if failed != nil").line("return " + it.name() + "{}, failed").close();
        body.line("value, err := " + souther + ".Reading(r, reading)").checked();
        body.line("return " + it.name() + "{Ref__: " + souther + ".NewRef(r, value)}, nil");
        at.items.append("\n// ").append(name).append(" is a value of `").append(it.key())
                .append("` read out of its external form, or the issues found in it as a\n")
                .append("// *raoh.Issues, or an invalid_format issue where the text is not JSON.\n")
                .append("func ").append(name).append("(r *").append(at.imports.lib())
                .append(".Run, json []byte) (").append(it.name()).append(", error) {\n").append(body)
                .append("}\n");
    }

    /** {@code Encode}: the value in its external form. */
    private void encode(GoModule at, Declared it, @Nullable Function encode) {
        if (encode == null) {
            return;
        }
        String souther = at.imports.souther();
        at.items.append("\n// Encode is this value in the external form of `").append(it.key())
                .append("`.\n")
                .append("func (v ").append(it.name()).append(") Encode() string {\n")
                .append("\tvalue := v.Ref__.Read()\n")
                .append("\trun := v.Ref__.Run()\n")
                .append("\ttext := ").append(at.shim(encode)).append("(run.Library().Symbol(\"")
                .append(encode.name()).append("\"), value)\n")
                .append("\treturn ").append(souther).append(".Text(run, text)\n}\n");
    }

    private void reader(GoModule at, Declared it, Manifest.Field field, String name) {
        Manifest.Read read = field.read().available();
        if (read == null) {
            return;
        }
        Crossing crossing = crossing(at.module, field.type(), read.answers(), Manifest.Way.HANDED);
        if (crossing == null) {
            return;
        }
        Body body = new Body(at.imports, at::shim, "run", "return", 1);
        body.line("value := v.Ref__.Read()");
        body.line("run := v.Ref__.Run()");
        List<String> rooms = Crossing.declare(body, "a", crossing.words());
        List<String> handed = new ArrayList<>(List.of("run.Library().Symbol(\""
                + read.function().name() + "\")", "value"));
        rooms.forEach(room -> handed.add("&" + room));
        body.line(at.shim(read.function()) + "(" + String.join(", ", handed) + ")");
        body.line("return " + crossing.of(body, rooms));
        at.items.append("\n// ").append(name).append(" is the `").append(field.name())
                .append("` of this value.\n")
                .append("func (v ").append(it.name()).append(") ").append(name).append("() ")
                .append(crossing.type(at.imports)).append(" {\n").append(body).append("}\n");
    }

    // ---------------------------------------------------------------------------------------------
    // Behaviors and values.

    private void behaviors(GoModule at) {
        for (Manifest.Behavior behavior : at.module.behaviors()) {
            Manifest.Call call = behavior.call().available();
            if (call == null || !requiresOf(at.module.name() + "." + behavior.name()).isEmpty()) {
                continue;
            }
            List<Crossing> takes = crossings(at.module, behavior.parameters().types(),
                    call.signature().takes(), Manifest.Way.GIVEN);
            Crossing answers = answered(at.module, behavior.answers(), call.signature().answers());
            if (takes == null || answers == null) {
                continue;
            }
            String what = "behavior `" + at.module.name() + "." + behavior.name() + "`";
            String name = at.names.claim(GoNames.exported(behavior.name(), what), what);
            GoNames.Claimed claimed = new GoNames.Claimed("the parameters of " + what);
            List<String> names = switch (behavior.parameters()) {
                case Manifest.Parameters.Named named -> named.parameters().stream()
                        .map(it -> claimed.claim(GoNames.local(it.name(),
                                "parameter `" + it.name() + "` of " + what),
                                "parameter `" + it.name() + "`"))
                        .toList();
                case Manifest.Parameters.Positional positional -> java.util.stream.IntStream
                        .range(0, positional.types().size()).mapToObj(it -> "input" + it).toList();
            };
            at.items.append(function(at, "// " + name + " calls " + what + ".", "func " + name,
                    names, takes, answers, call.function(), true, false));
        }
    }

    private void values(GoModule at) {
        for (Manifest.PublishedValue value : at.module.values()) {
            Manifest.Call read = value.read().available();
            if (read == null) {
                continue;
            }
            Crossing answers = crossing(at.module, value.type(), read.signature().answers(),
                    Manifest.Way.HANDED);
            if (answers == null) {
                continue;
            }
            String what = "value `" + at.module.name() + "." + value.name() + "`";
            String name = at.names.claim(GoNames.exported(value.name(), what), what);
            at.items.append(function(at, "// " + name + " reads " + what + ".", "func " + name,
                    List.of(), List.of(), answers, read.function(), false, false));
        }
    }

    /** What constructing {@code key} requires injected, in order, and nothing where it requires nothing. */
    private List<Manifest.Required> requiresOf(String key) {
        for (Manifest.Module module : manifest.modules()) {
            for (Manifest.Construction construction : module.constructions()) {
                if (key.equals(module.name() + "." + construction.name())) {
                    return construction.requires();
                }
            }
        }
        return List.of();
    }

    /**
     * A function declared as {@code declared}, calling {@code function} in the run it is handed and
     * answering what it wrote.
     *
     * @param requirements whether the function is called with what a behavior requires, which is
     *                     nothing here
     * @param constructed  whether a call that says the invariant was not held comes to an issue
     */
    private String function(GoModule at, String doc, String declared, List<String> names,
                            List<Crossing> takes, Crossing answers, Function function,
                            boolean requirements, boolean constructed) {
        Body.Imports imports = at.imports;
        String souther = imports.souther();
        Body body = new Body(imports, at::shim, "r", "return " + answers.zero(imports) + ", err", 1);
        List<String> parameters = new ArrayList<>(List.of("r *" + imports.lib() + ".Run"));
        for (int at2 = 0; at2 < takes.size(); at2++) {
            parameters.add(names.get(at2) + " " + takes.get(at2).type(imports));
        }
        body.line("fn := r.Library().Symbol(\"" + function.name() + "\")");
        List<String> handed = new ArrayList<>(List.of("fn"));
        if (requirements) {
            handed.add("nil");
        }
        for (int given = 0; given < takes.size(); given++) {
            List<String> words = Crossing.declare(body, "g", takes.get(given).words());
            takes.get(given).give(body, names.get(given), words);
            handed.addAll(words);
        }
        List<String> rooms = Crossing.declare(body, "a", answers.words());
        rooms.forEach(room -> handed.add("&" + room));
        body.line("failed := " + souther + ".Call(r, func() " + souther + ".Status {");
        body.line("\treturn " + souther + ".Status(" + at.shim(function) + "("
                + String.join(", ", handed) + "))");
        body.line("})");
        body.open("if failed != nil").line("return " + answers.zero(imports) + ", "
                + (constructed ? souther + ".Constructed(failed)" : "failed")).close();
        body.line("return " + answers.of(body, rooms) + ", nil");
        // A list or a union built here is made before the call is, so the run is asked first.
        String making = body.makes() ? "\t" + souther + ".Making(r)\n" : "";
        return "\n" + doc + "\n" + declared + "(" + String.join(", ", parameters) + ") ("
                + answers.type(imports) + ", error) {\n" + making + body + "}\n";
    }

    // ---------------------------------------------------------------------------------------------
    // The files.

    private static String header() {
        return "// Code generated by souther-native-compiler from souther.json. DO NOT EDIT.\n\n";
    }

    private void moduleFile(GoModule at) throws IOException {
        String pkg = at.path.getLast();
        StringBuilder go = new StringBuilder(header());
        go.append("// Package ").append(pkg).append(" is module `").append(at.module.name())
                .append("` of the model.\n").append("package ").append(pkg).append("\n\n");
        if (!at.shims.isEmpty()) {
            go.append("/*\n#include <stdint.h>\n#include \"souther.ffi.h\"\n\n");
            at.shims.values().forEach(it -> go.append(shim(it)));
            go.append("*/\nimport \"C\"\n\n");
        }
        String imports = at.imports.written();
        if (!imports.isEmpty()) {
            go.append(imports);
        }
        go.append(at.items);
        List<String> file = new ArrayList<>(at.path);
        file.add("module.go");
        file(file, go.toString());
        if (!at.shims.isEmpty()) {
            List<String> header = new ArrayList<>(at.path);
            header.add("souther.ffi.h");
            declarations(header);
        }
    }

    private void declarations(List<String> parts) throws IOException {
        Path at = into;
        for (String part : parts) {
            at = at.resolve(part);
        }
        Files.createDirectories(at.getParent());
        input.declarations().copyTo(at);
        written.add(at);
    }

    // ---------------------------------------------------------------------------------------------
    // What a call to a function through its address is in C.

    /** What C calls the type of a word that is a number. */
    private static @Nullable String scalar(Word word) {
        return switch (word) {
            case STATUS, CASE -> "uint32_t";
            case INT, COUNT, MARK -> "int64_t";
            case BOOL -> "uint8_t";
            case OUTCOME -> "int32_t";
            default -> null;
        };
    }

    /** What the declarations call the type of a word that is an address of the library's. */
    private static @Nullable String opaque(Word word) {
        return switch (word) {
            case VALUE -> "souther_value";
            case STRING -> "souther_string";
            case DECIMAL -> "souther_decimal";
            case DATE -> "souther_date";
            case TIME -> "souther_time";
            case DATETIME -> "souther_datetime";
            case INSTANT -> "souther_instant";
            case DECODED -> "souther_decoded";
            case ISSUE -> "souther_issue";
            case LIST -> "souther_list";
            case FUNCTION -> "souther_function";
            default -> null;
        };
    }

    /**
     * The C function that calls {@code function} through the address a library has for it, since
     * cgo cannot call one. Its type is the one the declarations say the symbol has, so a call that
     * does not fit them does not compile; an address of the library's crosses as a plain pointer
     * and is given the declarations' type here.
     */
    private static String shim(Function function) {
        List<String> parameters = new ArrayList<>(List.of("void *fn"));
        List<String> arguments = new ArrayList<>();
        int place = 0;
        for (Parameter it : function.takes()) {
            String name = "a" + place++;
            String number = scalar(it.word());
            String address = opaque(it.word());
            switch (it.mode()) {
                case GIVEN -> {
                    if (number != null) {
                        parameters.add(number + " " + name);
                        arguments.add(name);
                    } else if (address != null) {
                        parameters.add("void *" + name);
                        arguments.add("(" + address + ")" + name);
                    } else {
                        parameters.add("void *" + name);
                        arguments.add(switch (it.word()) {
                            case BYTES -> "(const uint8_t *)" + name;
                            case REQUIREMENTS -> "(const souther_capability *const *)" + name;
                            default -> name;
                        });
                    }
                }
                case ROOM -> {
                    if (number != null) {
                        parameters.add(number + " *" + name);
                        arguments.add(name);
                    } else {
                        parameters.add("void **" + name);
                        arguments.add("(" + address + " *)" + name);
                    }
                }
                case SLICE -> {
                    parameters.add("const void *" + name);
                    arguments.add(number != null ? "(const " + number + " *)" + name
                            : "(const " + address + " *)" + name);
                }
            }
        }
        Word answers = function.answers();
        String returns = answers == null ? "void" : scalar(answers) != null ? scalar(answers)
                : "void *";
        String call = "f(" + String.join(", ", arguments) + ")";
        return "static inline " + returns + " call_" + function.name() + "("
                + String.join(", ", parameters) + ") {\n\t__typeof__(&" + function.name()
                + ") f = fn;\n\t" + (answers == null ? "" : "return ")
                + (answers != null && scalar(answers) == null ? "(void *)" : "") + call + ";\n}\n";
    }

    // ---------------------------------------------------------------------------------------------
    // The root package, what it is loaded with, and the module.

    private void library() throws IOException {
        String pkg = GoNames.packageName(importPath);
        String go = header() + "// Package " + pkg + " is the Go binding of a Souther library built by"
                + " souther-native-compiler: a package\n// for each module of the model, and `Load`,"
                + " which loads the library and gives the runs its functions are called in.\n"
                + "package " + pkg + "\n\n"
                + "import (\n\tsouther \"" + RUNTIME_MODULE + "\"\n\n\t\"" + importPath
                + "/internal/binding\"\n)\n\n"
                + "// Library is the library this binding was generated for, loaded from a path.\n"
                + "type Library = souther.Library[binding.Tag]\n\n"
                + "// Run is a run of the library, as every function of the binding takes one. It"
                + " belongs to the\n// goroutine that opened it, and a run of another binding is"
                + " another type.\n"
                + "type Run = souther.Run[binding.Tag]\n\n"
                + "// Ref is a value of the library and the run it was made in.\n"
                + "type Ref = souther.Ref[binding.Tag]\n\n"
                + "// Load opens the library file at path.\n//\n"
                + "// It checks that every function this binding calls is there, which a library of another ABI"
                + " generation\n// has none of. That path is the library this binding was generated from is"
                + " the caller's to\n// hold: a library built from another program may have a function"
                + " of the same name that is\n// something else, and Load cannot tell.\n"
                + "func Load(path string) (*Library, error) {\n"
                + "\treturn souther.Load[binding.Tag](path, binding.Spec)\n}\n";
        file(List.of("library.go"), go);
    }

    private void binding() throws IOException {
        StringBuilder go = new StringBuilder(header());
        go.append("// Package binding is what the binding of this library is called by: its tag, which"
                + " makes\n// its runs and values types of their own, and what it asks of a library.\n"
                + "package binding\n\n"
                + "import souther \"" + RUNTIME_MODULE + "\"\n\n"
                + "// Tag is this binding's: a run of another generated binding is not a run of this"
                + " one.\ntype Tag struct{}\n\n"
                + "// Spec is what this binding needs of a library.\n"
                + "var Spec = souther.Spec{\n"
                + "\tStatuses: map[string]souther.Status{\n");
        aligned(go, manifest.statuses());
        go.append("\t},\n\tOutcomes: map[string]int32{\n");
        aligned(go, manifest.outcomes());
        go.append("\t},\n\tSymbols: []string{\n");
        TreeSet<String> all = new TreeSet<>(symbols);
        all.addAll(RuntimeFunctions.CALLED.keySet());
        all.forEach(name -> go.append("\t\t\"").append(name).append("\",\n"));
        go.append("\t},\n}\n");
        file(List.of("internal", "binding", "binding.go"), go.toString());
    }

    /** The entries of a Go map literal, their values in one column, as gofmt writes them. */
    private static void aligned(StringBuilder go, Map<String, Integer> numbers) {
        TreeMap<String, Integer> sorted = new TreeMap<>(numbers);
        int width = sorted.keySet().stream().mapToInt(String::length).max().orElse(0);
        sorted.forEach((name, number) -> go.append("\t\t\"").append(name).append("\":")
                .append(" ".repeat(width - name.length() + 1)).append(number).append(",\n"));
    }

    private void goMod() throws IOException {
        file(List.of("go.mod"), "// Generated by souther-native-compiler from souther.json. Written"
                + " again on every build.\nmodule " + importPath + "\n\ngo 1.27\n\nrequire "
                + RUNTIME_MODULE + " " + RUNTIME_VERSION + "\n");
    }

    private void file(List<String> parts, String content) throws IOException {
        Path at = into;
        for (String part : parts) {
            at = at.resolve(part);
        }
        Files.createDirectories(at.getParent());
        Files.writeString(at, content, StandardCharsets.UTF_8);
        written.add(at);
    }
}
