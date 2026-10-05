package souther.bindings.go;

import org.jspecify.annotations.Nullable;
import souther.bindings.Claimed;
import souther.bindings.BindingInput;
import souther.bindings.Manifest;
import souther.bindings.Manifest.Case;
import souther.bindings.Manifest.Declaration;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.Word;
import souther.bindings.NotBindable;
import souther.bindings.ValueCrossing;

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

    /**
     * The protocol of the runtime module this writes for: what its public surface is, as recorded
     * under {@code bindings/go/runtime/protocol}. A test holds it to the runtime's own.
     */
    static final int RUNTIME_PROTOCOL = 5;

    /**
     * The version of Raoh the runtime module asks for, which a package that imports it asks for as
     * well; held to the runtime's own go.mod by a test.
     */
    static final String RAOH_VERSION = "v0.9.0";

    /** The runtime module every import of it names, as the module says its own path. */
    private static final String RUNTIME_MODULE = RuntimeModule.THE.path();

    private final Manifest manifest;
    private final BindingInput input;
    private final String importPath;
    private final Path into;

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
     * Refuses what a generation as {@code importPath} would refuse whatever the manifest said: a
     * path Go will not take.
     *
     * @throws NotBindable where the path would be refused
     */
    public static void refuseAhead(String importPath) {
        GoNames.importPath(importPath);
    }

    /**
     * Writes the binding of what the input's manifest describes into {@code into}, as the package
     * {@code importPath}, which a host depends on by path; and answers {@code into}.
     *
     * @throws NotBindable where a name in the model is not one Go takes
     */
    public static Path generate(BindingInput input, Path into, String importPath)
            throws IOException {
        String path = GoNames.importPath(importPath);
        Files.createDirectories(into);
        new GoBindings(input, path, into).write();
        return into;
    }

    /** One module of the model: the package it is written as, and the Go written in it. */
    private final class GoModule {
        final Manifest.Module module;
        final List<String> path;
        final String importPath;
        final Body.Imports imports;
        final Claimed names;
        final StringBuilder items = new StringBuilder();
        final Map<String, Function> shims = new LinkedHashMap<>();
        /** What a host implements that the library calls back: each an exported function and what it needs declared. */
        final List<Callback> callbacks = new ArrayList<>();
        /** The C declarations and functions written beside the shims. */
        final StringBuilder c = new StringBuilder();

        GoModule(Manifest.Module module, List<String> path) {
            this.module = module;
            this.path = path;
            this.importPath = GoBindings.this.importPath + "/" + String.join("/", path);
            this.imports = new Body.Imports(GoBindings.this.importPath, importPath);
            this.names = new Claimed("the package of module `" + module.name() + "`");
        }

        /** The name of the C function that calls {@code function} through its address. */
        String shim(Function function) {
            shims.putIfAbsent(function.name(), function);
            symbols.add(function.name());
            return "C.call_" + function.name();
        }
    }

    /**
     * A function of the library's a host implements: the exported function the library calls, which
     * only hands what it is given to the function of the package that does the work.
     *
     * @param exported the name the library calls it by, which no other of the program has
     * @param takes    what it is handed, one Go type for each C parameter, as the declarations
     *                 spell it
     * @param type     the type the declarations say a function that implements it is of
     */
    private record Callback(String exported, List<String> takes, String forwards, String type) {
    }

    private void write() throws IOException {
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
        behaviorTypes();
        for (GoModule at : modules.values()) {
            module(at);
        }
        assertImportsAreReferences();
        assertAcyclic();
        for (GoModule at : modules.values()) {
            moduleFile(at);
        }
        library();
        binding();
        goMod();
    }

    // ---------------------------------------------------------------------------------------------
    // What a model type crosses as.

    /** How Go holds {@code value}, crossing the way {@code way} says, or null where it has no way. */
    private @Nullable Crossing held(Manifest.Module module, ValueCrossing value, Manifest.Way way) {
        Crossing made = switch (value) {
            case ValueCrossing.Primitive it -> Crossing.Whole.primitive(it.type().primitive(), it.word());
            case ValueCrossing.Handle it -> it.word() == Word.VALUE
                    ? handle(it.type().module(), it.type().name()) : null;
            // Handed to Go only as a behavior's answer, which says which case it is
            // (`answered`): anywhere else Go would be handed a value of it told nothing.
            case ValueCrossing.Union it -> it.word() == Word.VALUE && way == Manifest.Way.GIVEN
                    ? oneOf(module, it.type(), null) : null;
            case ValueCrossing.Optional it -> held(module, it.of(), way) instanceof Crossing of
                    ? new Crossing.Optional(of) : null;
            case ValueCrossing.Tuple it -> {
                if (it.members().size() > Crossing.Tuple.MOST) {
                    yield null;
                }
                List<Crossing> members = helds(module, it.members(), way);
                yield members == null ? null : new Crossing.Tuple(members);
            }
            case ValueCrossing.Listed it -> it.crosses(way)
                    && held(module, it.element(), way) instanceof Crossing element
                    ? new Crossing.Listed(element, it.crossing().construct(), it.crossing().read())
                    : null;
            case ValueCrossing.FunctionValue it -> {
                FunctionType written = functionType(module, it);
                // Handed to Go, a function value is one the library made, which is called through
                // the library; handed over, it may be one of the host's, which the library calls.
                yield written == null
                        || !(way == Manifest.Way.HANDED ? written.called() : written.hosted())
                        ? null : new Crossing.FunctionValue(written.importPath(), written.name(), it.shape());
            }
        };
        if (made != null && !made.shape().equals(value.shape())) {
            throw new IllegalStateException("this binding holds a value crossing as " + value.shape()
                    + " as what crosses as " + made.shape());
        }
        return made;
    }

    /** How Go holds each of {@code values}, or null where it has no way to hold any of them. */
    private @Nullable List<Crossing> helds(Manifest.Module module, List<ValueCrossing> values,
                                           Manifest.Way way) {
        List<Crossing> made = new ArrayList<>();
        for (ValueCrossing value : values) {
            Crossing it = held(module, value, way);
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
    private @Nullable Crossing answered(Manifest.Module module, Manifest.Answer answer,
                                        ValueCrossing value) {
        if (!(answer.type() instanceof Type.Union union)) {
            return held(module, value, Manifest.Way.HANDED);
        }
        Manifest.UnionAnswer told = Objects.requireNonNull(answer.union());
        if (!(value instanceof ValueCrossing.Union it) || it.word() != Word.VALUE || told.which() == null) {
            return null;
        }
        return oneOf(module, union, told);
    }

    /**
     * The type a function type is written as: where it stands, and whether a value the library made
     * can be called ({@code called}) and a host's own function handed over ({@code hosted}).
     */
    private record FunctionType(String importPath, String name, boolean called, boolean hosted) {
    }

    /** Each function type's Go type, by its module, the type and the shape it crosses in, once it is asked for. */
    private final Map<List<Object>, @Nullable FunctionType> functions = new LinkedHashMap<>();

    /**
     * {@code type} crossing as {@code shape} as the type generated for it, written in {@code
     * module}'s package the first time it is asked for; or null where a value of it can be neither
     * called nor made: what it takes or answers has no way to cross either way it would, or its name
     * is one another name of the package already is.
     *
     * <p>Calling one the library made hands over what it takes and is handed what it answers; a
     * host's own is handed what it takes and hands back what it answers. The two are asked apart,
     * since a union is handed over and not handed to Go, and each is written where the manifest
     * says how ({@code call}, {@code make}).
     */
    private @Nullable FunctionType functionType(Manifest.Module module,
                                                ValueCrossing.FunctionValue value) {
        // Written by the module that says it, and by no other, as a union is.
        List<Object> key = List.of(module.name(), value.type(), value.shape());
        if (functions.containsKey(key)) {
            return functions.get(key);
        }
        functions.put(key, null);
        Manifest.FunctionCrossing crossing = value.crossing();
        List<Crossing> handedOver = crossing.call() == null ? null
                : helds(module, value.takes(), Manifest.Way.GIVEN);
        Crossing answered = crossing.call() == null ? null
                : held(module, value.answers(), Manifest.Way.HANDED);
        boolean called = handedOver != null && answered != null;
        List<Crossing> handed = crossing.make() == null ? null
                : helds(module, value.takes(), Manifest.Way.HANDED);
        Crossing answering = crossing.make() == null ? null
                : held(module, value.answers(), Manifest.Way.GIVEN);
        boolean hosted = handed != null && answering != null;
        if (!called && !hosted) {
            return null;
        }
        List<Crossing> takes = called ? handedOver : handed;
        Crossing answers = called ? answered : answering;
        GoModule at = modules.get(module.name());
        String name = "Fn" + takes.stream().map(Crossing::label)
                .collect(java.util.stream.Collectors.joining("And")) + "To" + answers.label();
        List<String> generated = List.of(name, name + "Host", "Host" + name, name + "Word__",
                "hosted" + name, "dispatch" + name);
        if (generated.stream().anyMatch(at.names::has)) {
            return null;
        }
        generated.forEach(it -> at.names.claim(it, "what is written for the function type " + name));
        FunctionType made = new FunctionType(at.importPath, name, called, hosted);
        functions.put(key, made);

        Body.Imports imports = at.imports;
        String lib = imports.lib();
        String souther = imports.souther();
        String unsafe = imports.unsafe();
        List<String> inputs = java.util.stream.IntStream.range(0, takes.size())
                .mapToObj(it -> "input" + it).toList();
        List<String> parameters = new ArrayList<>(List.of("r *" + lib + ".Run"));
        for (int place = 0; place < takes.size(); place++) {
            parameters.add(inputs.get(place) + " " + takes.get(place).type(imports));
        }
        String signed = "func(" + String.join(", ", parameters) + ") (" + answers.type(imports)
                + ", error)";
        String what = "a function value of " + (takes.isEmpty() ? "nothing" : takes.stream()
                .map(it -> it.type(imports)).collect(java.util.stream.Collectors.joining(", ")))
                + " to " + answers.type(imports);
        String host = name + "Host";
        StringBuilder out = new StringBuilder();
        out.append("\n// ").append(name).append(" is ").append(what)
                .append(": one the library made, or a function of the host's own.\n")
                .append("type ").append(name).append(" struct {\n")
                .append("\t// Ref__ is the function the library made, and the run it was made in.\n")
                .append("\tRef__ ").append(lib).append(".Ref\n")
                .append("\t// Host__ is a function of the host's own.\n")
                .append("\tHost__ *").append(host).append("\n}\n");
        out.append("\n// ").append(host).append(" is a function of the host's own that is ").append(what)
                .append(".\ntype ").append(host).append(" struct {\n\tFn ").append(signed).append("\n}\n");
        out.append("\n// Host").append(name).append(" is f as ").append(what).append(".\n")
                .append("func Host").append(name).append("(f ").append(signed).append(") ").append(name)
                .append(" {\n\treturn ").append(name).append("{Host__: &").append(host)
                .append("{f}}\n}\n");

        // Call: a function of the host's is called as it is, and one the library made through the
        // library. Both ask the run first, as every function that takes one does.
        Before hostBranch = frame -> {
            Body body = frame.body();
            String self = Objects.requireNonNull(frame.receiver());
            String answer = body.names.fixed("answer");
            body.open("if " + self + ".Host__ != nil");
            body.line(answer + ", " + body.err() + " := " + self + ".Host__.Fn(" + String.join(", ",
                    java.util.stream.Stream.concat(java.util.stream.Stream.of(frame.run()), inputs.stream())
                            .toList()) + ")");
            body.open("if " + body.err() + " != nil").line("return " + answers.zero(imports) + ", &"
                    + souther + ".HostError{Err: " + body.err() + "}").close();
            body.line("return " + answer + ", nil").close();
            if (!called) {
                body.line("panic(\"the library hands over no " + what + " it offers no way to call\")");
                return null;
            }
            String function = body.names.fixed("function");
            body.line(function + ", " + body.err() + " := " + self + ".Ref__.In(" + frame.run() + ")").checked();
            return function;
        };
        Receiver receiver = new Receiver("f", name);
        if (called) {
            out.append(function(at, "// Call calls it in r, with what it takes.", "Call", receiver, inputs,
                    takes, answers, crossing.call(), hostBranch, false));
        } else {
            out.append(callOfWhatCannotBeCalled(at, receiver, inputs, takes, answers, hostBranch));
        }

        // Word__: the value as the library of r is handed it.
        out.append("\n// ").append(name).append("Word__ is the function value as the library of r is handed"
                + " it: one another library made is\n// refused, and a host's function is made into one"
                + " there the first time it is handed over.\n")
                .append("func ").append(name).append("Word__(r *").append(lib).append(".Run, f ")
                .append(name).append(") (").append(unsafe).append(".Pointer, error) {\n")
                .append("\tif f.Host__ == nil {\n\t\treturn f.Ref__.In(r)\n\t}\n");
        if (hosted) {
            Manifest.FunctionMaking making = Objects.requireNonNull(crossing.make());
            symbols.add(making.implement());
            String dispatch = "hosted" + name;
            out.append("\tfn := r.Library().Symbol(\"").append(making.implement()).append("\")\n")
                    .append("\treturn ").append(souther).append(".HostFunction(r, f.Host__, func() any {\n")
                    .append("\t\treturn &").append(dispatch).append("{r, f.Host__.Fn}\n\t}, func(room, userdata ")
                    .append(unsafe).append(".Pointer) ").append(unsafe).append(".Pointer {\n")
                    .append("\t\treturn C.implement_").append(making.implement())
                    .append("(fn, room, userdata)\n\t}), nil\n}\n");
            out.append("\n// ").append(dispatch).append(" is what the library hands back when it calls a"
                    + " function of the host's: the\n// function, and the run it was handed over in.\n")
                    .append("type ").append(dispatch).append(" struct {\n\torigin *").append(lib)
                    .append(".Run\n\tfn     ").append(signed).append("\n}\n");
            at.items.append(out);
            hostCallback(at, GoNames.hostSymbol('f', importPath, module.name(), name),
                    "dispatch" + name,
                    dispatch, "fn", making.implementation(), making.implement(), true,
                    handed, answering, inputs);
        } else {
            out.append("\tpanic(\"the library offers no way to make ").append(what)
                    .append(" of a host's own function\")\n}\n");
            at.items.append(out);
        }
        return made;
    }

    /**
     * {@code Call} of a function type whose values the library made cannot be called through the
     * library: a host's own function is called, and asking a value of the library's is a program
     * that could not be told the way to call one. It asks the run first, as the rest do.
     */
    private String callOfWhatCannotBeCalled(GoModule at, Receiver receiver, List<String> inputs,
                                            List<Crossing> takes, Crossing answers, Before host) {
        Body.Imports imports = at.imports;
        Names scope = new Names(inputs);
        String self = scope.fixed(receiver.name());
        Body body = new Body(imports, at::shim, scope, GoNames.RUN,
                "return " + answers.zero(imports) + ", " + scope.fixed("err"), 1);
        body.line(imports.souther() + ".Making(" + GoNames.RUN + ")");
        host.write(new Frame(body, GoNames.RUN, self));
        List<String> parameters = new ArrayList<>(List.of(GoNames.RUN + " *" + imports.lib() + ".Run"));
        for (int at2 = 0; at2 < takes.size(); at2++) {
            parameters.add(inputs.get(at2) + " " + takes.get(at2).type(imports));
        }
        return "\n// Call calls it in r, with what it takes.\nfunc (" + self + " " + receiver.type() + ") Call("
                + String.join(", ", parameters) + ") (" + answers.type(imports) + ", error) {\n" + body + "}\n";
    }

    /** The interface each union no declaration names is written as, by its members, once it is asked for. */
    private final Map<UnionKey, @Nullable UnionType> unions = new LinkedHashMap<>();

    /**
     * A union is written by the module that says it, and by no other: a package that reused the one
     * another module wrote would import it, and an import that is no dependency of the model is one
     * Go may find a cycle in, where the model has none.
     */
    private record UnionKey(String module, List<Case> cases) {
    }

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
        UnionKey key = new UnionKey(module.name(), union.cases());
        UnionType made = unions.containsKey(key) ? unions.get(key)
                : unionType(modules.get(module.name()), union);
        unions.put(key, made);
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
                            : new Crossing.OneOf.Member(it.name(), Crossing.Whole.handle(it), null, null,
                            it.importPath().equals(at.importPath));
                }
                case Case.Primitive p -> {
                    Manifest.CaseCrossing crossing = manifest.crossing(p);
                    Word held = crossing.holds();
                    Crossing.Whole whole = held == null ? null : Crossing.Whole.primitive(p.primitive(), held);
                    if (whole == null) {
                        yield null;
                    }
                    yield new Crossing.OneOf.Member(p.primitive().spelt(), whole, crossing.make(),
                            Objects.requireNonNull(crossing.read()), false);
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
            case Case.Primitive p -> p.primitive().spelt();
            case Case.Language l -> l.name();
        }).collect(java.util.stream.Collectors.joining(" | "));
        out.append("\n// ").append(name).append(" is a value of `").append(what)
                .append("`: one of its members. A type switch tells them apart: a member declared in\n")
                .append("// this package is a value of it as it is, and any other is held by a type of its own.\n")
                .append(SUM_TYPE)
                .append("type ").append(name).append(" interface {\n\t").append(marker).append("()\n}\n");
        for (Crossing.OneOf.Member member : members) {
            if (member.itself()) {
                String type = member.whole().type(at.imports);
                out.append("\n// ").append(marker).append(" makes a value of ").append(type)
                        .append(" one of `").append(what).append("`.\n").append(noBody(type, marker));
                continue;
            }
            String variant = at.names.claim(name + member.variant(), "the member `" + member.variant()
                    + "` of the union `" + what + "`");
            out.append("\n// ").append(variant).append(" is the member ").append(member.variant())
                    .append(" of `").append(what).append("`.\n").append("type ").append(variant)
                    .append(" struct {\n\tValue ").append(member.whole().type(at.imports))
                    .append("\n}\n\n").append(noBody(variant, marker));
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
        for (Manifest.Injection injection : at.module.injections()) {
            BehaviorType it = behaviorTypes.get(at.module.name() + "." + injection.name());
            if (it != null) {
                injected(at, injection, it);
            }
        }
        for (Manifest.Behavior behavior : at.module.behaviors()) {
            BehaviorType it = behaviorTypes.get(at.module.name() + "." + behavior.name());
            if (it != null && !it.injected()) {
                bound(at, behavior, it);
            }
        }
    }

    /** The struct a value of {@code it} is held as, with what reads it and makes it. */
    private void handleType(GoModule at, Declared it) {
        Declaration declaration = it.declaration();
        Claimed methods = new Claimed("the methods of `" + it.key() + "`");
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
     * @param holds  the Go type of the value of the case, or null where it holds none
     * @param each   the case, or null for the one every case the model keeps is
     * @param itself whether the case's own type is a value of the sum's cases: a type declared in
     *               the sum's package, which the method of the cases is written on. Any other case
     *               is a type of the sum's, holding its value as {@code Value} where it has one.
     */
    private record Arm(String variant, String holds, boolean itself) {
    }

    /**
     * What the doc comment of every interface a union or a sum's cases is ends with: the directive
     * that declares it a sum type to go-check-sumtype (and golangci-lint's gochecksumtype), which
     * fails a type switch over it that leaves a case out. Go does not check a type switch for the
     * types it leaves out, and the model's cases are closed, so a host that runs the check learns of
     * a case added to the model at every switch that does not answer it, when it builds.
     */
    static final String SUM_TYPE = "//\n//sumtype:decl\n";

    /** The variant every case of a sum the model keeps is, holding the value as the sum. */
    private static final String KEPT = "Kept";

    /**
     * Each case of {@code sum} in the order its {@code which} counts them, or null where a case has
     * no variant this binding can write: where nothing says which case a value is, or two cases
     * that would be one variant. A case the model keeps, a declared type
     * it does not publish, is {@value #KEPT}, holding the value as the sum.
     */
    private @Nullable List<Arm> arms(GoModule at, Declared of, Declaration.Sum sum) {
        if (sum.which() == null) {
            return null;
        }
        List<Arm> arms = new ArrayList<>();
        Set<String> variants = new java.util.HashSet<>();
        Arm kept = new Arm(KEPT, of.name(), false);
        for (Case.Declared each : sum.cases()) {
            Declared it = declared.get(each.module() + "." + each.name());
            Arm arm = it == null ? kept : new Arm(it.name(),
                    at.imports.module(it.importPath()) + it.name(),
                    it.importPath().equals(at.importPath));
            if (!variants.add(arm.variant()) && arm != kept) {
                return null;
            }
            arms.add(arm);
        }
        if (arms.contains(kept)
                && arms.stream().anyMatch(it -> it != kept && !it.itself() && it.variant().equals(KEPT))) {
            return null;
        }
        return arms;
    }

    private static Set<String> cases(Declaration.Sum sum) {
        return sum.cases().stream().map(it -> it.module() + "." + it.name())
                .collect(java.util.stream.Collectors.toCollection(LinkedHashSet::new));
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
                .append("` is, as the value of that case. A type switch tells them apart: a case declared\n")
                .append("// in this package is its own type, and any other is a type of its own.\n")
                .append(SUM_TYPE)
                .append("type ").append(caseType).append(" interface {\n\t").append(marker)
                .append("()\n}\n");
        for (Arm arm : new LinkedHashSet<>(arms)) {
            if (arm.itself()) {
                out.append("\n// ").append(marker).append(" makes a value of ").append(arm.holds())
                        .append(" a case of `").append(it.key()).append("`.\n")
                        .append(noBody(arm.holds(), marker));
                continue;
            }
            String variant = it.name() + arm.variant();
            at.names.claim(variant, "the case `" + arm.variant() + "` of `" + it.key() + "`");
            out.append("\n// ").append(variant).append(" is the case ").append(arm.variant())
                    .append(" of `").append(it.key()).append("`.\n").append("type ").append(variant)
                    .append(" struct");
            out.append(" {\n\tValue " + arm.holds() + "\n}\n");
            out.append("\n").append(noBody(variant, marker));
        }
        Body body = new Body(at.imports, at::shim, new Names(List.of()), "run", "return", 1);
        body.line("value := v.Ref__.Read()");
        body.line("run := v.Ref__.Run()");
        body.line("switch " + at.shim(Objects.requireNonNull(sum.which())) + "(run.Library().Symbol(\""
                + sum.which().name() + "\"), value) {");
        for (int place = 0; place < arms.size(); place++) {
            Arm arm = arms.get(place);
            String variant = it.name() + arm.variant();
            body.line("case " + place + ":");
            String held = souther + ".NewRef(run, value)";
            if (arm.itself()) {
                body.line("\treturn " + arm.holds() + "{Ref__: " + held + "}");
            } else {
                body.line("\treturn " + variant + "{Value: " + arm.holds() + "{Ref__: " + held + "}}");
            }
        }
        body.line("}");
        body.line("panic(\"the library answered a case `" + it.key() + "` does not have\")");
        out.append("\n// Case is which case this value is, as the value of that case.\n")
                .append("func (v ").append(it.name()).append(") Case() ").append(caseType)
                .append(" {\n").append(body).append("}\n");
        // A value of a case, or of a narrower sum, is a value of the sum as it is. What the
        // conversion names has to be something this package may name: a case is one of the sum's own
        // and its module is one the sum's depends on, and so is a narrower sum of this same
        // package. A narrower sum of another module is not: nothing says that module is one this
        // depends on, and the sum it is a part of is not one that module depends on, so a helper
        // that named it would import the other way about. It is converted where its two sums are
        // written, with Ref__, which both are held by.
        Set<String> mine = cases(sum);
        for (Declared other : declared.values()) {
            boolean within = switch (other.declaration()) {
                case Declaration.Sum narrower -> other != it && mine.containsAll(cases(narrower))
                        && !cases(narrower).equals(mine) && other.importPath().equals(it.importPath());
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
        List<Crossing> takes = helds(at.module, construct.takes(), Manifest.Way.GIVEN);
        if (takes == null) {
            return;
        }
        String what = "the constructor of `" + it.key() + "`";
        String name = at.names.claim("New" + it.name(), what);
        Claimed claimed = new Claimed("the parameters of " + what);
        List<String> names = new ArrayList<>();
        for (Manifest.Field field : fields) {
            names.add(claimed.claim(GoNames.local(field.name(),
                    "field `" + it.key() + "." + field.name() + "`"), "field `" + field.name() + "`"));
        }
        Crossing made = Crossing.Whole.handle(it);
        at.items.append(function(at, "// " + name + " is a value of `" + it.key() + "`, or an"
                + " invariant_violation issue where what is handed over does not hold what the type"
                + " states.", name, null, names, takes, made, construct.function(), null, true));
    }

    /** {@code Decode<Type>}: a value of the type read out of its external form, or the issues found in it. */
    private void decode(GoModule at, Declared it, @Nullable Function decode) {
        if (decode == null) {
            return;
        }
        String name = at.names.claim("Decode" + it.name(), "the reader of `" + it.key() + "`");
        String souther = at.imports.souther();
        String unsafe = at.imports.unsafe();
        Names scope = new Names(List.of("json"));
        String err = scope.fixed("err");
        Body body = new Body(at.imports, at::shim, scope, GoNames.RUN, "return " + it.name() + "{}, " + err, 1);
        String fn = scope.fixed("fn");
        String reading = scope.fixed("reading");
        String failed = scope.fixed("failed");
        String value = scope.fixed("value");
        body.line(souther + ".Making(r)");
        body.line(fn + " := r.Library().Symbol(\"" + decode.name() + "\")");
        body.line("var " + reading + " " + unsafe + ".Pointer");
        body.line(failed + " := " + souther + ".Called(r, func() " + souther + ".Status {");
        body.line("\treturn " + souther + ".Status(" + at.shim(decode) + "(" + fn + ", " + souther
                + ".Addr(json), C.int64_t(len(json)), &" + reading + "))");
        body.line("})");
        body.open("if " + failed + " != nil").line("return " + it.name() + "{}, " + failed).close();
        body.line(value + ", " + err + " := " + souther + ".Reading(r, " + reading + ")").checked();
        body.line("return " + it.name() + "{Ref__: " + souther + ".NewRef(r, " + value + ")}, nil");
        at.items.append("\n// ").append(name).append(" is a value of `").append(it.key())
                .append("` read out of its external form, or the issues found in it as a\n")
                .append("// *raoh.Issues, or an invalid_format issue where the text is not JSON.\n")
                .append("func ").append(name).append("(r *").append(at.imports.lib())
                .append(".Run, json []byte) (").append(it.name()).append(", error) {\n").append(body)
                .append("}\n");
        String decoder = at.names.claim(it.name() + "Decoder", "the raoh decoder of `" + it.key() + "`");
        at.items.append("\n// ").append(decoder).append(" is ").append(name)
                .append(" as a raoh decoder of what a host decoded, reading in r:\n")
                .append("// composed with a host's own decoders, its issues come back with theirs, at the path it is reached at.\n")
                .append("// It holds r, and is good only in r's run on the goroutine that opened it: made for a decode\n")
                .append("// inside the run, not kept past it or shared, which raoh's own decoders can be.\n")
                .append("func ").append(decoder).append("(r *").append(at.imports.lib()).append(".Run) ")
                .append(at.imports.raoh()).append(".Decoder[any, ").append(it.name()).append("] {\n")
                .append("\treturn ").append(souther).append(".Decoder(r, ").append(name).append(")\n}\n");
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
        Crossing crossing = held(at.module, read.answers(), Manifest.Way.HANDED);
        if (crossing == null) {
            return;
        }
        Body body = new Body(at.imports, at::shim, new Names(List.of()), "run", "return", 1);
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
            List<Crossing> takes = helds(at.module, call.crossings().takes(), Manifest.Way.GIVEN);
            Crossing answers = answered(at.module, behavior.answers(), call.crossings().answers());
            if (takes == null || answers == null) {
                continue;
            }
            String what = "behavior `" + at.module.name() + "." + behavior.name() + "`";
            String name = at.names.claim(GoNames.exported(behavior.name(), what), what);
            Claimed claimed = new Claimed("the parameters of " + what);
            List<String> names = switch (behavior.parameters()) {
                case Manifest.Parameters.Named named -> named.parameters().stream()
                        .map(it -> claimed.claim(GoNames.local(it.name(),
                                "parameter `" + it.name() + "` of " + what),
                                "parameter `" + it.name() + "`"))
                        .toList();
                case Manifest.Parameters.Positional positional -> java.util.stream.IntStream
                        .range(0, positional.types().size()).mapToObj(it -> "input" + it).toList();
            };
            at.items.append(function(at, "// " + name + " calls " + what + ".", name, null,
                    names, takes, answers, call.function(), frame -> "nil", false));
        }
    }

    private void values(GoModule at) {
        for (Manifest.PublishedValue value : at.module.values()) {
            Manifest.Call read = value.read().available();
            if (read == null) {
                continue;
            }
            Crossing answers = held(at.module, read.crossings().answers(), Manifest.Way.HANDED);
            if (answers == null) {
                continue;
            }
            String what = "value `" + at.module.name() + "." + value.name() + "`";
            String name = at.names.claim(GoNames.exported(value.name(), what), what);
            at.items.append(function(at, "// " + name + " reads " + what + ".", name, null,
                    List.of(), List.of(), answers, read.function(), null, false));
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

    /** What a function is written around: its run, its receiver where it is a method, and its body. */
    private record Frame(Body body, String run, @Nullable String receiver) {
    }

    /**
     * What a function writes before it calls the library, which may be a good deal (a host's own
     * function is called and answered here) or nothing. It answers what the call is handed first,
     * or null where it is handed nothing first.
     */
    private interface Before {
        @Nullable String write(Frame frame);
    }

    /** A method's receiver: the name a generated signature gives it, and its type. */
    private record Receiver(String name, String type) {
    }

    /**
     * A function called {@code name}, calling {@code function} in the run it is handed and
     * answering what it wrote.
     *
     * <p>Whatever else it does, it asks {@code Making(r)} of its run first: the check is a thing every
     * function that takes a run does, in one place, and is never something a function has because of
     * what it happens to write. The call is then made with {@code Called}, which does not ask
     * again.
     *
     * @param receiver    what a method is a method of, or null for a function
     * @param names       what the model names each parameter
     * @param before      what is written before the call, or null for nothing
     * @param constructed whether a call that says the invariant was not held comes to an issue
     */
    private String function(GoModule at, String doc, String name, @Nullable Receiver receiver,
                            List<String> names, List<Crossing> takes, Crossing answers,
                            Function function, @Nullable Before before, boolean constructed) {
        Body.Imports imports = at.imports;
        String souther = imports.souther();
        // The model's names first, so that whatever the function writes of its own yields to them.
        Names scope = new Names(names);
        String run = GoNames.RUN;
        String self = receiver == null ? null : scope.fixed(receiver.name());
        String err = scope.fixed("err");
        Body body = new Body(imports, at::shim, scope, run, "return " + answers.zero(imports) + ", " + err, 1);
        List<String> parameters = new ArrayList<>(List.of(run + " *" + imports.lib() + ".Run"));
        for (int at2 = 0; at2 < takes.size(); at2++) {
            parameters.add(names.get(at2) + " " + takes.get(at2).type(imports));
        }
        body.line(souther + ".Making(" + run + ")");
        String requirements = before == null ? null : before.write(new Frame(body, run, self));
        String fn = scope.fixed("fn");
        String failed = scope.fixed("failed");
        body.line(fn + " := " + run + ".Library().Symbol(\"" + function.name() + "\")");
        List<String> handed = new ArrayList<>(List.of(fn));
        if (requirements != null) {
            handed.add(requirements);
        }
        for (int given = 0; given < takes.size(); given++) {
            List<String> words = Crossing.declare(body, "g", takes.get(given).words());
            takes.get(given).give(body, names.get(given), words);
            handed.addAll(words);
        }
        List<String> rooms = Crossing.declare(body, "a", answers.words());
        rooms.forEach(room -> handed.add("&" + room));
        body.line(failed + " := " + souther + ".Called(" + run + ", func() " + souther + ".Status {");
        body.line("\treturn " + souther + ".Status(" + at.shim(function) + "("
                + String.join(", ", handed) + "))");
        body.line("})");
        body.open("if " + failed + " != nil").line("return " + answers.zero(imports) + ", "
                + (constructed ? souther + ".Constructed(" + failed + ")" : failed)).close();
        body.line("return " + answers.of(body, rooms) + ", nil");
        String declared = receiver == null ? "func " + name
                : "func (" + self + " " + receiver.type() + ") " + name;
        return "\n" + doc + "\n" + declared + "(" + String.join(", ", parameters) + ") ("
                + answers.type(imports) + ", error) {\n" + body + "}\n";
    }

    // ---------------------------------------------------------------------------------------------
    // A behavior as an application holds one.

    /**
     * The types a behavior is written as: an interface a host implements and the type an
     * implementation is made into, where the library asks a host to implement the behavior; and
     * otherwise a type bound to what the behavior requires.
     *
     * @param type       the interface of an injected behavior, and the name a bound one is written
     *                   under without its suffix
     * @param capability what stands for the behavior where another requires it
     */
    private record BehaviorType(String module, String name, GoModule at, String type,
                                String capability, boolean injected) {

        String key() {
            return module + "." + name;
        }

        /** What stands for the behavior, as the file being written names it. */
        String requirement(Body.Imports imports) {
            return imports.module(at.importPath) + capability;
        }
    }

    /** The type of each behavior that has one, by {@code module.name}. */
    private final Map<String, BehaviorType> behaviorTypes = new LinkedHashMap<>();

    /**
     * Which behaviors are written as types, of every module: each a host implements and can be
     * handed across to, and each published behavior a host can call that requires something or is
     * required, whose every requirement has a type too, since binding it hands one of each over.
     *
     * <p>The names are this generator's, the behavior's with a suffix, so a name another of the
     * package already is leaves the behavior with no type rather than refusing the binding; what
     * requires it has none either. What the model itself names is refused where Go will not take it.
     */
    private void behaviorTypes() {
        Map<String, BehaviorType> candidates = new LinkedHashMap<>();
        Map<String, List<Manifest.Required>> requires = new LinkedHashMap<>();
        java.util.Set<String> required = new java.util.HashSet<>();
        for (Manifest.Module module : manifest.modules()) {
            for (Manifest.Construction construction : module.constructions()) {
                construction.requires().forEach(it -> required.add(it.key()));
            }
        }
        for (Manifest.Module module : manifest.modules()) {
            GoModule at = modules.get(module.name());
            for (Manifest.Injection injection : module.injections()) {
                if (implementable(module, injection)) {
                    String name = GoNames.exported(injection.name(), "behavior `" + module.name()
                            + "." + injection.name() + "`");
                    BehaviorType it = new BehaviorType(module.name(), injection.name(), at, name,
                            name + "Implementation", true);
                    candidates.put(it.key(), it);
                    requires.put(it.key(), List.of());
                }
            }
            for (Manifest.Behavior behavior : module.behaviors()) {
                String key = module.name() + "." + behavior.name();
                if (callable(module, behavior)
                        && (!requiresOf(key).isEmpty() || required.contains(key))) {
                    String name = GoNames.exported(behavior.name(), "behavior `" + key + "`");
                    BehaviorType it = new BehaviorType(module.name(), behavior.name(), at, name,
                            name + "Bound", false);
                    candidates.put(it.key(), it);
                    requires.put(it.key(), requiresOf(key));
                }
            }
        }
        // Each written name is claimed by one behavior only.
        Map<String, Long> spelt = new LinkedHashMap<>();
        for (BehaviorType it : candidates.values()) {
            for (String name : generated(it)) {
                spelt.merge(it.at().importPath + " " + name, 1L, Long::sum);
            }
        }
        candidates.values().removeIf(it -> generated(it).stream().anyMatch(name ->
                it.at().names.has(name) || spelt.get(it.at().importPath + " " + name) > 1));
        boolean dropped = true;
        while (dropped) {
            dropped = candidates.values().removeIf(it -> requires.get(it.key()).stream()
                    .anyMatch(each -> !candidates.containsKey(each.key())));
        }
        for (BehaviorType it : candidates.values()) {
            for (String name : generated(it)) {
                it.at().names.claim(name, "what is written for behavior `" + it.key() + "`");
            }
        }
        behaviorTypes.putAll(candidates);
    }

    /** The names {@code it} takes in its package. */
    private static List<String> generated(BehaviorType it) {
        return it.injected()
                ? List.of(it.type(), it.capability(), "Implement" + it.type(), "hosted" + it.type(),
                        "dispatch" + it.type())
                : List.of(it.capability(), "Bind" + it.type());
    }

    /** Whether a host can be handed what {@code injection} takes and hand back what it answers. */
    private boolean implementable(Manifest.Module module, Manifest.Injection injection) {
        return helds(module, injection.crossings().takes(), Manifest.Way.HANDED) != null
                && held(module, injection.crossings().answers(), Manifest.Way.GIVEN) != null;
    }

    /** Whether a host can call {@code behavior}, handing over what it takes and handed what it answers. */
    private boolean callable(Manifest.Module module, Manifest.Behavior behavior) {
        Manifest.Call call = behavior.call().available();
        return call != null
                && helds(module, call.crossings().takes(), Manifest.Way.GIVEN) != null
                && answered(module, behavior.answers(), call.crossings().answers()) != null;
    }

    /** The names a behavior's parameters are written under. */
    private static List<String> parameterNames(Manifest.Parameters parameters, String what) {
        Claimed claimed = new Claimed("the parameters of " + what);
        return switch (parameters) {
            case Manifest.Parameters.Named named -> named.parameters().stream()
                    .map(it -> claimed.claim(GoNames.local(it.name(),
                            "parameter `" + it.name() + "` of " + what),
                            "parameter `" + it.name() + "`"))
                    .toList();
            case Manifest.Parameters.Positional positional -> java.util.stream.IntStream
                    .range(0, positional.types().size()).mapToObj(it -> "input" + it).toList();
        };
    }

    /**
     * The interface a host implements {@code injection} as, the type an implementation is made
     * into that the library calls it through, and what the library calls ({@link #hostCallback}).
     */
    private void injected(GoModule at, Manifest.Injection injection, BehaviorType it) {
        Manifest.Module module = at.module;
        List<Crossing> takes = Objects.requireNonNull(helds(module, injection.crossings().takes(), Manifest.Way.HANDED));
        Crossing answers = Objects.requireNonNull(held(module, injection.crossings().answers(), Manifest.Way.GIVEN));
        String what = "behavior `" + it.key() + "`";
        List<String> names = parameterNames(new Manifest.Parameters.Named(injection.parameters()),
                what);
        Body.Imports imports = at.imports;
        String souther = imports.souther();
        String lib = imports.lib();
        String unsafe = imports.unsafe();
        String trait = it.type();
        String hosted = "hosted" + trait;
        symbols.add(injection.implement());

        List<String> parameters = new ArrayList<>(List.of("r *" + lib + ".Run"));
        for (int place = 0; place < takes.size(); place++) {
            parameters.add(names.get(place) + " " + takes.get(place).type(imports));
        }
        at.items.append("\n// ").append(trait).append(" is what implements `").append(it.key())
                .append("`, which the library asks a host to implement. Made into a\n// ")
                .append(it.capability()).append(" it is handed to what requires the behavior, and the"
                        + " library calls Apply\n// wherever what was bound to it reaches the behavior.\n")
                .append("type ").append(trait).append(" interface {\n")
                .append("\t// Apply answers `").append(it.key()).append("` in r, the run of the call that reached\n")
                .append("\t// it. An error returned here comes back out of that call, as a panic here does.\n")
                .append("\tApply(").append(String.join(", ", parameters)).append(") (")
                .append(answers.type(imports)).append(", error)\n}\n");
        at.items.append("\n// ").append(it.capability()).append(" is an implementation of `")
                .append(it.key()).append("`, made into a capability the library calls it through.\n")
                .append("type ").append(it.capability()).append(" struct {\n")
                .append("\t// Cap__ is what the binding hands to what requires the behavior.\n")
                .append("\tCap__ ").append(lib).append(".Capability\n}\n");
        at.items.append("\n// ").append(hosted).append(" is what the library hands back when it calls the"
                + " implementation: the\n// implementation, and the run its capability was made in.\n")
                .append("type ").append(hosted).append(" struct {\n\torigin *").append(lib)
                .append(".Run\n\timpl   ").append(trait).append("\n}\n");
        at.items.append("\n// Implement").append(trait).append(" makes a capability of impl as `")
                .append(it.key()).append("`, held until r ends.\n")
                .append("func Implement").append(trait).append("(r *").append(lib).append(".Run, impl ")
                .append(trait).append(") ").append(it.capability()).append(" {\n")
                .append("\tfn := r.Library().Symbol(\"").append(injection.implement()).append("\")\n")
                .append("\treturn ").append(it.capability()).append("{Cap__: ").append(souther)
                .append(".Implemented(r, &").append(hosted).append("{r, impl}, func(capability, hosted, userdata ")
                .append(unsafe).append(".Pointer) {\n")
                .append("\t\tC.implement_").append(injection.implement())
                .append("(fn, capability, hosted, userdata)\n\t})}\n}\n");
        hostCallback(at, GoNames.hostSymbol('i', importPath, module.name(), injection.name()),
                "dispatch" + trait, hosted, "impl.Apply", injection.implementation(),
                injection.implement(), false, takes, answers, names);
    }

    /**
     * What the library calls for a function a host implements, as the manifest says the type of
     * that function is: it makes Go values of what the library handed over, calls the host's
     * function in the run of the call that reached it, and writes what it answered through the room
     * the library handed over.
     *
     * <p>The function the library calls is exported from a file of its own, and only hands what it
     * is given to {@code dispatch}, which does the work in the file that has the C it calls
     * ({@code Callback}). What the library is given to call is made by a C function of this file's,
     * {@code implement_<symbol>}, which names the exported one.
     *
     * @param exported what the function is exported as, which no other of the program is ({@link GoNames#hostSymbol})
     * @param hosted   the type of what the library hands back first, holding the run its function
     *                 was made in as {@code origin}
     * @param call     what calls the host's function, as a member of the value the library hands back
     *                 first, taking the run and what the library handed over
     * @param function whether what is implemented is a function value and not a behavior, which
     *                 answers the value the library made
     */
    private void hostCallback(GoModule at, String exported, String dispatch, String hosted,
                              String call, Manifest.Implementation implementation, String implement,
                              boolean function, List<Crossing> takes, Crossing answers,
                              List<String> names) {
        Body.Imports imports = at.imports;
        String souther = imports.souther();
        String lib = imports.lib();
        String unsafe = imports.unsafe();
        // The model's names for what the host's function takes are locals of the function that does
        // the work, beside what it is handed: they are claimed first, and whatever else is written
        // there yields to them.
        Names scope = new Names(names);
        // What the library calls is declared with the declarations' own types, which the C compiler
        // holds it to (abi.c); what does the work is handed the same words as Go's own.
        List<String> exportedParameters = new ArrayList<>();
        List<String> goParameters = new ArrayList<>();
        List<String> handedWords = new ArrayList<>();
        List<String> rooms = new ArrayList<>();
        List<String> forwarded = new ArrayList<>();
        String userdata = null;
        for (Parameter parameter : implementation.takes()) {
            Word word = parameter.word();
            String scalar = CTypes.scalar(word);
            String address = CTypes.opaque(word);
            String name;
            String exportedType;
            String goType;
            String forward;
            if (word == Word.USERDATA && parameter.mode() == Parameter.Mode.GIVEN
                    && goParameters.isEmpty()) {
                name = scope.fixed("userdata");
                userdata = name;
                exportedType = goType = unsafe + ".Pointer";
                forward = name;
            } else if (parameter.mode() == Parameter.Mode.GIVEN) {
                name = scope.temp("handed");
                handedWords.add(name);
                goType = Crossing.local(word, imports);
                exportedType = scalar != null ? "C." + scalar : "C." + Objects.requireNonNull(address);
                forward = scalar != null ? name : unsafe + ".Pointer(" + name + ")";
            } else if (parameter.mode() == Parameter.Mode.ROOM) {
                name = scope.temp("answer");
                rooms.add(name);
                goType = "*" + Crossing.local(word, imports);
                exportedType = "*C." + (scalar != null ? scalar : Objects.requireNonNull(address));
                forward = scalar != null ? name : "(*" + unsafe + ".Pointer)(" + unsafe + ".Pointer("
                        + name + "))";
            } else {
                throw new IllegalArgumentException("a callback is handed no slice");
            }
            goParameters.add(name + " " + goType);
            exportedParameters.add(name + " " + exportedType);
            forwarded.add(forward);
        }
        // The exported function is declared as the declarations say the implementation is, and the C
        // compiler is asked whether what Go exports is that (abi.c): no cast says it is.
        at.c.append("extern __typeof__(*(").append(implementation.type()).append(")0) ")
                .append(exported).append(";\n");
        if (function) {
            at.c.append("_Static_assert(__builtin_types_compatible_p(__typeof__(&").append(implement)
                    .append("), souther_function (*)(souther_hosted_function *, ")
                    .append(implementation.type()).append(", void *)),\n\t\"").append(implement)
                    .append(" is not the function the manifest says it is\");\n")
                    .append("static inline void *implement_").append(implement)
                    .append("(void *fn, void *a0, void *a1) {\n")
                    .append("\t__typeof__(&").append(implement).append(") f = fn;\n")
                    .append("\treturn (void *)f((souther_hosted_function *)a0, ").append(exported)
                    .append(", a1);\n}\n");
        } else {
            at.c.append("_Static_assert(__builtin_types_compatible_p(__typeof__(&").append(implement)
                    .append("), void (*)(souther_capability *, souther_hosted *, ")
                    .append(implementation.type()).append(", void *)),\n\t\"").append(implement)
                    .append(" is not the function the manifest says it is\");\n")
                    .append("static inline void implement_").append(implement)
                    .append("(void *fn, void *a0, void *a1, void *a2) {\n")
                    .append("\t__typeof__(&").append(implement).append(") f = fn;\n")
                    .append("\tf((souther_capability *)a0, (souther_hosted *)a1, ").append(exported)
                    .append(", a2);\n}\n");
        }
        String run = scope.fixed("run");
        String err = scope.fixed("err");
        String answer = scope.fixed("answer");
        String held = scope.fixed("hosted");
        Objects.requireNonNull(userdata, "what the library hands an implementation first is its userdata");
        Body body = new Body(imports, at::shim, scope, run, "return " + souther + ".Crossing(" + err + ")", 2);
        int word = 0;
        List<String> arguments = new ArrayList<>(List.of(run));
        for (int place = 0; place < takes.size(); place++) {
            int wide = takes.get(place).words().size();
            body.line(names.get(place) + " := " + takes.get(place).of(body,
                    handedWords.subList(word, word + wide)));
            word += wide;
            arguments.add(names.get(place));
        }
        body.line(answer + ", " + err + " := " + held + "." + call + "("
                + String.join(", ", arguments) + ")");
        body.open("if " + err + " != nil").line("return " + err).close();
        List<String> given = Crossing.declare(body, "g", answers.words());
        answers.give(body, answer, given);
        for (int place = 0; place < given.size(); place++) {
            body.line("*" + rooms.get(place) + " = " + given.get(place));
        }
        body.line("return nil");
        at.items.append("\n// ").append(dispatch).append(" makes Go values of what the library handed over and"
                + " calls the host's function in the run of\n// the call that reached it.\n")
                .append("func ").append(dispatch).append("(").append(String.join(", ", goParameters))
                .append(") C.uint32_t {\n\t").append(held).append(" := ").append(souther)
                .append(".UserdataValue(").append(userdata).append(").(*").append(hosted).append(")\n")
                .append("\treturn C.uint32_t(").append(souther).append(".Host(").append(held)
                .append(".origin, func(").append(run).append(" *").append(lib).append(".Run) error {\n")
                .append(body).append("\t}))\n}\n");
        at.callbacks.add(new Callback(exported, exportedParameters, dispatch + "("
                + String.join(", ", forwarded) + ")", implementation.type()));
    }

    /**
     * The type an application binds {@code behavior} through and calls it on: a function binding it
     * to what stands for each behavior it requires, and {@code Call}, calling it with the
     * capabilities of what it was bound to, in the run it is handed.
     */
    private void bound(GoModule at, Manifest.Behavior behavior, BehaviorType it) {
        Manifest.Module module = at.module;
        Manifest.Call call = Objects.requireNonNull(behavior.call().available());
        List<Crossing> takes = Objects.requireNonNull(helds(module, call.crossings().takes(), Manifest.Way.GIVEN));
        Crossing answers = Objects.requireNonNull(answered(module, behavior.answers(), call.crossings().answers()));
        String what = "behavior `" + it.key() + "`";
        List<String> names = parameterNames(behavior.parameters(), what);
        List<Manifest.Required> requires = requiresOf(it.key());
        Manifest.Construction construction = null;
        for (Manifest.Module each : manifest.modules()) {
            for (Manifest.Construction c : each.constructions()) {
                if ((each.name() + "." + c.name()).equals(it.key())) {
                    construction = c;
                }
            }
        }
        Body.Imports imports = at.imports;
        String lib = imports.lib();
        String souther = imports.souther();
        String unsafe = imports.unsafe();
        // A requirement is named after the behavior it is where no other is of that name, and
        // after its place otherwise. Its name, the run's and what the function writes of its own are
        // one scope: the names of the requirements are claimed first, and the rest yield.
        Map<String, Long> counted = requires.stream().collect(java.util.stream.Collectors.groupingBy(
                Manifest.Required::name, java.util.stream.Collectors.counting()));
        Names scope = new Names(List.of());
        scope.fixed(GoNames.RUN);
        List<String> parameters = new ArrayList<>(List.of(GoNames.RUN + " *" + lib + ".Run"));
        List<String> capabilities = new ArrayList<>();
        for (int place = 0; place < requires.size(); place++) {
            Manifest.Required each = requires.get(place);
            String preferred = counted.get(each.name()) == 1 ? GoNames.local(each.name(),
                    "requirement `" + each.name() + "`") : "dependency" + place;
            String name = scope.fixed(preferred.endsWith("_") ? "dependency" + place : preferred);
            parameters.add(name + " " + behaviorTypes.get(each.key()).requirement(imports));
            capabilities.add(name + ".Cap__");
        }
        String bindClosure = "nil";
        StringBuilder bind = new StringBuilder();
        if (construction != null && construction.bind() != null) {
            String fn = scope.fixed("fn");
            bind.append("\t").append(fn).append(" := r.Library().Symbol(\"")
                    .append(construction.bind().name()).append("\")\n");
            bindClosure = "func(capability, requirements " + unsafe + ".Pointer) {\n\t\t"
                    + at.shim(construction.bind()) + "(" + fn + ", capability, requirements)\n\t}";
        }
        at.items.append("\n// ").append(it.capability()).append(" is `").append(it.key())
                .append("` as an application holds it: bound to what stands for each behavior it\n")
                .append("// requires, which each call is made with.\n")
                .append("type ").append(it.capability()).append(" struct {\n")
                .append("\t// Cap__ is what the binding hands to what requires the behavior.\n")
                .append("\tCap__ ").append(lib).append(".Capability\n}\n");
        at.items.append("\n// Bind").append(it.type()).append(" is `").append(it.key())
                .append("` bound to what stands for each behavior it requires, held until r ends.\n")
                .append("func Bind").append(it.type()).append("(").append(String.join(", ", parameters))
                .append(") ").append(it.capability()).append(" {\n").append(bind)
                .append("\treturn ").append(it.capability()).append("{Cap__: ").append(souther)
                .append(".Bound(r, []").append(lib).append(".Capability{").append(String.join(", ", capabilities))
                .append("}, ").append(bindClosure).append(")}\n}\n");
        // What is bound is asked what the call is handed first, after the run is asked.
        Before requirementsOf = frame -> {
            Body body = frame.body();
            String requirements = body.names.fixed("requirements");
            body.line(requirements + ", " + body.err() + " := " + frame.receiver() + ".Cap__.Requirements("
                    + frame.run() + ")").checked();
            return requirements;
        };
        at.items.append(function(at, "// Call calls `" + it.key() + "` with what this was bound to.",
                "Call", new Receiver("b", it.capability()), names, takes, answers, call.function(),
                requirementsOf, false));
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
        if (!at.shims.isEmpty() || at.c.length() > 0) {
            go.append("/*\n#include <stdint.h>\n#include \"souther.ffi.h\"\n\n");
            go.append(at.c);
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
        if (!at.callbacks.isEmpty()) {
            StringBuilder exports = new StringBuilder(header());
            exports.append("package ").append(pkg).append("\n\n")
                    .append("/*\n#include <stdint.h>\n#include \"souther.ffi.h\"\n*/\nimport \"C\"\n\n")
                    .append("import \"unsafe\"\n\n")
                    .append("// A file that exports a function has a preamble of declarations only, and the\n")
                    .append("// function is only what the library calls: the work is done in module.go.\n");
            for (Callback it : at.callbacks) {
                exports.append("\n//export ").append(it.exported()).append("\nfunc ").append(it.exported())
                        .append("(").append(String.join(", ", it.takes())).append(") C.uint32_t {\n")
                        .append("\treturn ").append(it.forwards()).append("\n}\n");
            }
            exports.append("\nvar _ = unsafe.Pointer(nil)\n");
            List<String> exported = new ArrayList<>(at.path);
            exported.add("callbacks.go");
            file(exported, exports.toString());
            // What Go exports is what the declarations say an implementation is: a C file of the
            // package sees both, and the compiler says whether they are one type.
            StringBuilder abi = new StringBuilder("/* Generated by souther-native-compiler from souther.json."
                    + " Written again on every build. */\n\n"
                    + "/* What callbacks.go includes is in this, and the declarations have no guard. */\n"
                    + "#include \"_cgo_export.h\"\n\n");
            for (Callback it : at.callbacks) {
                abi.append("_Static_assert(__builtin_types_compatible_p(__typeof__(&").append(it.exported())
                        .append("), ").append(it.type()).append("),\n\t\"").append(it.exported())
                        .append(" is not what the library declares an implementation as\");\n");
            }
            List<String> check = new ArrayList<>(at.path);
            check.add("abi.c");
            file(check, abi.toString());
        }
        if (!at.shims.isEmpty() || at.c.length() > 0) {
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
    }

    /**
     * Holds what is written to what the model says. A package imports another only for a type the
     * module it is written from refers to (a field, a parameter, an answer, a case, a union's member,
     * a behavior it requires), and the model's modules do not depend on one another in a cycle, so
     * the packages do not either. The first is what makes the second true, so it is asked for and
     * not the second alone: a helper written for two types that relate as types and not as
     * dependencies is an import the model has no reason for, which a cycle check finds only where a
     * cycle happens to close, and then refuses a library that is right.
     */
    private void assertImportsAreReferences() {
        Map<String, String> moduleOf = new LinkedHashMap<>();
        modules.values().forEach(it -> moduleOf.put(it.importPath, it.module.name()));
        for (GoModule at : modules.values()) {
            Set<String> refers = referred(at.module);
            for (String imported : at.imports.modulePaths()) {
                if (!refers.contains(moduleOf.get(imported))) {
                    throw new IllegalStateException("package " + at.importPath + " imports " + imported
                            + ", which module " + at.module.name() + " does not refer to");
                }
            }
        }
    }

    /** The modules whose types the manifest of {@code module} names anywhere. */
    private static Set<String> referred(Manifest.Module module) {
        Set<String> refers = new java.util.HashSet<>();
        for (Declaration declaration : module.declarations()) {
            declaration.fields().forEach(it -> refer(it.type(), refers));
            if (declaration instanceof Declaration.Sum sum) {
                sum.cases().forEach(it -> refer(it, refers));
            }
        }
        for (Manifest.Behavior behavior : module.behaviors()) {
            behavior.parameters().types().forEach(it -> refer(it, refers));
            refer(behavior.answers().type(), refers);
            if (behavior.answers().union() != null) {
                behavior.answers().union().cases().forEach(it -> refer(it, refers));
            }
        }
        for (Manifest.Injection injection : module.injections()) {
            injection.parameters().forEach(it -> refer(it.type(), refers));
            refer(injection.answers(), refers);
        }
        module.values().forEach(it -> refer(it.type(), refers));
        for (Manifest.Construction construction : module.constructions()) {
            construction.requires().forEach(it -> refers.add(it.module()));
        }
        return refers;
    }

    private static void refer(Case each, Set<String> refers) {
        if (each instanceof Case.Declared declared) {
            refers.add(declared.module());
        }
    }

    private static void refer(Type type, Set<String> refers) {
        switch (type) {
            case Type.Declared it -> refers.add(it.module());
            case Type.Union it -> it.cases().forEach(each -> refer(each, refers));
            case Type.Option it -> refer(it.of(), refers);
            case Type.ListOf it -> refer(it.of(), refers);
            case Type.SetOf it -> refer(it.of(), refers);
            case Type.MapOf it -> {
                refer(it.key(), refers);
                refer(it.value(), refers);
            }
            case Type.Tuple it -> it.of().forEach(each -> refer(each, refers));
            case Type.Function it -> {
                it.takes().forEach(each -> refer(each, refers));
                refer(it.answers(), refers);
            }
            case Type.Primitive it -> { }
            case Type.Nothing it -> { }
            case Type.Never it -> { }
        }
    }

    /**
     * The packages import one another without a cycle, as Go asks. Every import being one the model
     * refers to ({@link #assertImportsAreReferences}) makes this true; it is asked as well, so that
     * a cycle is a refusal here that says which and not a build that fails for a host.
     */
    private void assertAcyclic() {
        Map<String, GoModule> byPath = new LinkedHashMap<>();
        modules.values().forEach(it -> byPath.put(it.importPath, it));
        Set<String> done = new java.util.HashSet<>();
        for (GoModule start : modules.values()) {
            visit(start, byPath, done, new ArrayList<>());
        }
    }

    private void visit(GoModule at, Map<String, GoModule> byPath, Set<String> done, List<String> path) {
        if (done.contains(at.importPath)) {
            return;
        }
        if (path.contains(at.importPath)) {
            List<String> cycle = new ArrayList<>(path.subList(path.indexOf(at.importPath), path.size()));
            cycle.add(at.importPath);
            throw new IllegalStateException("the generated packages import one another in a cycle: "
                    + String.join(" -> ", cycle));
        }
        path.add(at.importPath);
        for (String imported : at.imports.modulePaths()) {
            visit(byPath.get(imported), byPath, done, path);
        }
        path.removeLast();
        done.add(at.importPath);
    }

    // ---------------------------------------------------------------------------------------------
    // What a call to a function through its address is in C.

    /**
     * The C that calls {@code function} through the address a library has for it, since cgo cannot
     * call one, and asserts that the function the declarations declare is of the type its words make
     * ({@link CTypes#asserted}). A number crosses as itself and an address as a plain pointer, given
     * the declarations' type here, where the assertion has held it to what they say.
     */
    private static String shim(Function function) {
        List<String> parameters = new ArrayList<>(List.of("void *fn"));
        List<String> arguments = new ArrayList<>();
        int place = 0;
        for (Parameter it : function.takes()) {
            String name = "a" + place++;
            String exact = CTypes.parameter(it);
            boolean number = it.mode() != Parameter.Mode.SLICE && it.word() != Word.CAPABILITY
                    && CTypes.scalar(it.word()) != null;
            if (number && it.mode() == Parameter.Mode.GIVEN) {
                parameters.add(exact + " " + name);
                arguments.add(name);
            } else if (number) {
                parameters.add(exact + name);
                arguments.add(name);
            } else if (it.mode() == Parameter.Mode.ROOM && CTypes.opaque(it.word()) != null) {
                parameters.add("void **" + name);
                arguments.add("(" + exact + ")" + name);
            } else if (it.mode() == Parameter.Mode.SLICE) {
                parameters.add("const void *" + name);
                arguments.add("(" + exact + ")" + name);
            } else {
                parameters.add("void *" + name);
                arguments.add("(" + exact + ")" + name);
            }
        }
        Word answers = function.answers();
        boolean answersNumber = answers != null && CTypes.scalar(answers) != null;
        String returns = answers == null ? "void" : answersNumber ? CTypes.scalar(answers) : "void *";
        String expected = "expected_" + function.name();
        String call = "f(" + String.join(", ", arguments) + ")";
        return CTypes.asserted(function, function.name())
                + "static inline " + returns + " call_" + function.name() + "("
                + String.join(", ", parameters) + ") {\n\t" + expected + " f = (" + expected
                + ")fn;\n\t" + (answers == null ? "" : "return ")
                + (answers != null && !answersNumber ? "(void *)" : "") + call + ";\n}\n";
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
                + "// Capability stands for a behavior another requires, made in a run.\n"
                + "type Capability = souther.Capability[binding.Tag]\n\n"
                + "// Load opens the library file at path.\n//\n"
                + "// It refuses a library of another ABI generation, and checks that every function this"
                + " binding\n// calls is there. That path is the library this binding was generated from is"
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
                + "// This binding is written for one protocol of the runtime, and does not compile against"
                + " another:\n// the surface of each is recorded, and a change to it is a new one.\n"
                + "var _ = [1]struct{}{}[souther.Protocol-" + RUNTIME_PROTOCOL + "]\n\n"
                + "// Tag is this binding's: a run of another generated binding is not a run of this"
                + " one.\ntype Tag struct{}\n\n"
                + "// Spec is what this binding needs of a library.\n"
                + "var Spec = souther.Spec{\n\tLayout: layout,\n"
                + "\tStatuses: map[string]souther.Status{\n");
        aligned(go, manifest.statuses());
        go.append("\t},\n\tOutcomes: map[string]int32{\n");
        aligned(go, manifest.outcomes());
        go.append("\t},\n\tSymbols: []string{\n");
        new TreeSet<>(symbols).forEach(name -> go.append("\t\t\"").append(name).append("\",\n"));
        go.append("\t},\n}\n");
        file(List.of("internal", "binding", "binding.go"), go.toString());
        abi();
        declarations(List.of("internal", "binding", "souther.ffi.h"));
    }

    /**
     * What the runtime module lays out room for, asked of the declarations and not worked out: each
     * size is the compiler's {@code sizeof} of what the declarations declare. The runtime's own
     * functions are the ABI generation's, which the runtime module asks a library for before it
     * calls any, and its calls are held to that generation's record by its own test.
     */
    private void abi() throws IOException {
        StringBuilder go = new StringBuilder(header());
        go.append("package binding\n\n/*\n#include <stdint.h>\n#include \"souther.ffi.h\"\n\n"
                + "typedef const souther_capability *souther_capability_ref;\n\n");
        go.append("*/\nimport \"C\"\n\nimport souther \"" + RUNTIME_MODULE + "\"\n\n"
                + "// layout is what the declarations say a host lays out room for.\n"
                + "var layout = souther.Layout{\n"
                + "\tPointer:        uintptr(C.sizeof_souther_capability_ref),\n"
                + "\tCapability:     uintptr(C.sizeof_souther_capability),\n"
                + "\tHosted:         uintptr(C.sizeof_souther_hosted),\n"
                + "\tHostedFunction: uintptr(C.sizeof_souther_hosted_function),\n}\n");
        file(List.of("internal", "binding", "abi.go"), go.toString());
    }

    /**
     * The method {@code marker} of {@code receiver} with no body, as gofmt writes it: the braces on
     * its line where the header up to them is shorter than 100 bytes, and on two lines otherwise.
     */
    static String noBody(String receiver, String marker) {
        String header = "func (" + receiver + ") " + marker + "()";
        return header + (header.getBytes(StandardCharsets.UTF_8).length < 100 ? " {}\n" : " {\n}\n");
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
                + " again on every build.\nmodule " + importPath + "\n\ngo " + RuntimeModule.THE.go()
                + "\n\nrequire (\n\t"
                + Body.Imports.RAOH + " " + RAOH_VERSION + "\n\t" + RUNTIME_MODULE + " "
                + RuntimeModule.THE.requirement() + "\n)\n");
    }

    private void file(List<String> parts, String content) throws IOException {
        Path at = into;
        for (String part : parts) {
            at = at.resolve(part);
        }
        Files.createDirectories(at.getParent());
        Files.writeString(at, content, StandardCharsets.UTF_8);
    }
}
