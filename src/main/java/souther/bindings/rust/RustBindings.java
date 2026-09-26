package souther.bindings.rust;

import org.jspecify.annotations.Nullable;
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

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.TreeMap;
import java.util.stream.Collectors;

/**
 * Writes the Rust crate a host calls a library through, from the manifest the build wrote beside
 * it.
 *
 * <p>Reads the manifest and nothing else, as the PHP generator does. What it writes is the
 * manifest's surface as Rust: a module for each module of the model, a handle for each published
 * type and a function for each behavior a host can call, over the runtime crate in
 * {@code bindings/rust/runtime}, which is where a run's lifetime, loading the library and what each
 * status means are kept. The functions are looked up by name through the library a host loads by
 * path, into a table of typed function pointers this writes from the manifest ({@code ffi.rs}); no
 * header is read and nothing is linked.
 *
 * <p>What a host has no way to reach is not written: a behavior the manifest says is unavailable, a
 * field with no {@code read}, a type Rust has no representation for here. How a value crosses is
 * read off the manifest's shape for it, and what is decided here is only how Rust holds it
 * ({@link Crossing}).
 */
public final class RustBindings {

    /** What a binding is written as: the directory its crate stands in, and every file in it. */
    public record Generated(Path root, List<Path> files) {

        public Generated {
            files = List.copyOf(files);
        }
    }

    /** What says a directory is a Rust binding this wrote, and may be replaced whole. */
    static final String MARK = ".souther-rust-binding";

    /** The version of the runtime crate what this writes calls. */
    static final String RUNTIME_VERSION = "0.1";

    /**
     * The runtime's functions the runtime crate calls, as the manifest has to say them: the crate
     * looks each up by this name and calls it as these words say, so a manifest saying another is
     * one this crate would call as something it is not.
     */
    private static final Map<String, Function> RUNTIME = runtime();

    private static Map<String, Function> runtime() {
        Map<String, Function> functions = new LinkedHashMap<>();
        java.util.function.BiConsumer<String, List<Object>> add = (name, words) -> {
            List<Parameter> takes = new ArrayList<>();
            for (Object word : words.subList(0, words.size() - 1)) {
                takes.add(Parameter.given((Word) word));
            }
            functions.put(name, new Function(name, takes, (Word) words.getLast()));
        };
        java.util.List<Object> none = new ArrayList<>();
        none.add(null);
        functions.put("souther_mark", new Function("souther_mark", List.of(), Word.MARK));
        functions.put("souther_reset",
                new Function("souther_reset", List.of(Parameter.given(Word.MARK)), null));
        add.accept("souther_string_of_utf8", List.of(Word.BYTES, Word.COUNT, Word.STRING));
        add.accept("souther_string_length", List.of(Word.STRING, Word.COUNT));
        add.accept("souther_string_bytes", List.of(Word.STRING, Word.BYTES));
        add.accept("souther_decimal_of_parts", List.of(Word.STRING, Word.INT, Word.DECIMAL));
        add.accept("souther_decimal_unscaled", List.of(Word.DECIMAL, Word.STRING));
        add.accept("souther_decimal_scale", List.of(Word.DECIMAL, Word.INT));
        add.accept("souther_decoded_outcome", List.of(Word.DECODED, Word.OUTCOME));
        add.accept("souther_decoded_value", List.of(Word.DECODED, Word.VALUE));
        add.accept("souther_decoded_malformed_at", List.of(Word.DECODED, Word.COUNT));
        add.accept("souther_decoded_issue_count", List.of(Word.DECODED, Word.COUNT));
        add.accept("souther_decoded_issue", List.of(Word.DECODED, Word.COUNT, Word.ISSUE));
        add.accept("souther_issue_code", List.of(Word.ISSUE, Word.STRING));
        add.accept("souther_issue_path", List.of(Word.ISSUE, Word.STRING));
        add.accept("souther_issue_meta_count", List.of(Word.ISSUE, Word.COUNT));
        add.accept("souther_issue_meta_key", List.of(Word.ISSUE, Word.COUNT, Word.STRING));
        add.accept("souther_issue_meta_value", List.of(Word.ISSUE, Word.COUNT, Word.STRING));
        return Map.copyOf(functions);
    }

    /** Every name the root of the crate declares, which no top module of the model may be. */
    private static final List<String> ROOT = List.of("Library", "Run", "Scope", "AlreadyRunning",
            "Construction", "Decimal", "Failure", "HostError", "LoadError", "NotADecimal",
            "Reading", "raoh", "__ffi");

    private final Manifest manifest;
    private final String crate;
    private final Path into;
    private final List<Path> written = new ArrayList<>();

    /** What each declared type is, by {@code module.Name}. */
    private final Map<String, Declared> declared = new LinkedHashMap<>();

    /** Each module, by its name. */
    private final Map<String, Manifest.Module> modules = new LinkedHashMap<>();

    /** Each Rust module the crate has, by its path, and what is written in it. */
    private final Map<List<String>, RustModule> tree = new LinkedHashMap<>();

    /** The type each behavior is written as, by {@code module.name}, where it has one. */
    private final Map<String, BehaviorType> behaviorTypes = new LinkedHashMap<>();

    /** The type of every function the generated code calls, by its symbol, in the order first asked for. */
    private final Map<String, String> symbols = new LinkedHashMap<>();

    private RustBindings(Manifest manifest, String crate, Path into) {
        this.manifest = manifest;
        this.crate = crate;
        this.into = into;
    }

    /**
     * Refuses what a generation into {@code into} as {@code crate} would refuse whatever the
     * manifest said: a name Cargo will not take, and a directory holding what no generation wrote.
     *
     * @throws NotBindable where the name or the directory would be refused
     */
    public static void refuseAhead(Path into, String crate) throws IOException {
        RustNames.crateName(crate);
        Output.replaceable(into, MARK);
    }

    /**
     * Writes the binding of what {@code manifest} describes into {@code into}, as the crate
     * {@code crate}, which a host depends on by path.
     *
     * <p>{@code into} is then that crate and nothing else: it is written beside it and put in place
     * whole ({@link Output}), so a module the model no longer declares does not survive a
     * generation, and a refused one leaves what was there as it was.
     *
     * @throws NotBindable where a name in the model is not one Rust takes
     */
    public static Generated generate(Path manifest, Path into, String crate) throws IOException {
        Manifest read = Manifest.read(manifest);
        String name = RustNames.crateName(crate);
        Output output = Output.replacing(into, MARK);
        RustBindings binding = new RustBindings(read, name, output.staging());
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

    /** A declared type, and where the crate writes it. */
    private record Declared(String module, Declaration declaration, List<String> path, String name) {

        /** The type as a path from the root of the crate, without its lifetime. */
        String type() {
            return "crate::" + String.join("::", path) + "::" + name;
        }

        String key() {
            return module + "." + declaration.name();
        }
    }

    /** One module of the crate: its children, and the Rust written in it. */
    private static final class RustModule {
        final Set<String> children = new LinkedHashSet<>();
        final StringBuilder items = new StringBuilder();
        final RustNames.Claimed types;
        final RustNames.Claimed values;

        RustModule(String where) {
            types = new RustNames.Claimed("the types of " + where);
            values = new RustNames.Claimed("the functions of " + where);
            types.claim("rt", "the runtime crate's alias");
        }
    }

    private RustModule moduleAt(List<String> path) {
        RustModule module = tree.get(path);
        if (module != null) {
            return module;
        }
        module = new RustModule(path.isEmpty() ? "the crate's root" : "module `"
                + String.join("::", path) + "`");
        tree.put(path, module);
        if (!path.isEmpty()) {
            List<String> parent = path.subList(0, path.size() - 1);
            RustModule above = moduleAt(parent);
            String child = path.getLast();
            if (above.children.add(child)) {
                above.types.claim(child, "module `" + String.join("::", path) + "`");
            }
        }
        return module;
    }

    private void write() throws IOException {
        checkRuntime();
        RustModule root = moduleAt(List.of());
        ROOT.forEach(it -> root.types.claim(it, "the generated `" + it + "`"));
        for (Manifest.Module module : manifest.modules()) {
            List<String> path = RustNames.modulePath(module.name());
            RustModule at = moduleAt(path);
            modules.put(module.name(), module);
            for (Declaration declaration : module.declarations()) {
                String what = "type `" + module.name() + "." + declaration.name() + "`";
                String name = at.types.claim(RustNames.identifier(declaration.name(), what), what);
                Declared it = new Declared(module.name(), declaration, path, name);
                declared.put(it.key(), it);
                if (declaration instanceof Declaration.Sum) {
                    at.types.claim(name + "Case", "the cases of " + what);
                }
            }
        }
        behaviorTypes();
        for (Manifest.Module module : manifest.modules()) {
            module(module);
        }
        library();
        ffi();
        for (Map.Entry<List<String>, RustModule> it : tree.entrySet()) {
            if (!it.getKey().isEmpty()) {
                moduleFile(it.getKey(), it.getValue());
            }
        }
        cargo();
    }

    /**
     * Refuses a manifest whose runtime functions the runtime crate would call as something they
     * are not: each it calls is there, taking and answering what it calls it with.
     */
    private void checkRuntime() {
        Map<String, Function> said = new LinkedHashMap<>();
        manifest.runtime().forEach(it -> said.put(it.name(), it));
        for (Function expected : RUNTIME.values()) {
            Function it = said.get(expected.name());
            if (it == null || !it.takes().equals(expected.takes())
                    || it.answers() != expected.answers()) {
                throw new IllegalArgumentException("the manifest says the runtime's "
                        + expected.name() + " is " + it + ", and the Rust runtime calls it as "
                        + expected);
            }
        }
    }

    // ---------------------------------------------------------------------------------------------
    // What a model type crosses as.

    /**
     * How a value of {@code type} crosses in {@code shape}, the way {@code way} says, in a function
     * of {@code module}'s, or null where this binding has no way to hold it: a pair of a type and a
     * shape it knows no way to hold, a declared type it has no handle for, a union no declaration
     * names, and a function value.
     */
    private @Nullable Crossing crossing(Manifest.Module module, Type type, Shape shape,
                                        Manifest.Way way) {
        Crossing made = switch (shape) {
            case Shape.Leaf leaf -> switch (type) {
                case Type.Primitive it -> Crossing.Whole.primitive(it.name(), leaf.word());
                case Type.Declared it -> leaf.word() == Word.VALUE ? handle(it.module(), it.name())
                        : null;
                default -> null;
            };
            case Shape.Option option -> type instanceof Type.Option it
                    && crossing(module, it.of(), option.of(), way) instanceof Crossing of
                    ? new Crossing.Optional(of) : null;
            case Shape.Product product -> {
                if (!(type instanceof Type.Tuple it) || it.of().size() != product.of().size()) {
                    yield null;
                }
                List<Crossing> members = crossings(module, it.of(), product.of(), way);
                yield members == null ? null : new Crossing.Tuple(members);
            }
            case Shape.ListOf list -> {
                if (!(type instanceof Type.ListOf it)
                        || !(crossing(module, it.of(), list.element(), way) instanceof Crossing element)) {
                    yield null;
                }
                Manifest.ListCrossing listed = module.lists().stream()
                        .filter(l -> l.element().equals(list.element())).findFirst().orElseThrow();
                Manifest.ListRead read = listed.read();
                if (way == Manifest.Way.GIVEN ? listed.construct() == null : read == null) {
                    yield null;
                }
                yield new Crossing.Listed(element, listed,
                        listed.construct() == null ? null : symbol(listed.construct()),
                        read == null ? null : symbol(read.length()),
                        read == null ? null : symbol(read.at()));
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

    /** The handle of the declared type {@code module.name}, or null where it has none. */
    private Crossing.@Nullable Whole handle(String module, String name) {
        Declared it = declared.get(module + "." + name);
        return it == null ? null : Crossing.Whole.handle(it.type());
    }

    /** The field of the symbol table {@code function} is called through, noted to be looked up. */
    private String symbol(Function function) {
        String type = functionType(function.takes(), function.answers());
        String before = symbols.putIfAbsent(function.name(), type);
        if (before != null && !before.equals(type)) {
            throw new IllegalArgumentException("the manifest says " + function.name()
                    + " is both " + before + " and " + type);
        }
        return function.name();
    }

    /** The type of a pointer to a C function taking {@code takes} and answering {@code answers}. */
    private static String functionType(List<Parameter> takes, @Nullable Word answers) {
        String parameters = takes.stream().map(it -> switch (it.mode()) {
            case GIVEN -> Crossing.word(it.word());
            case ROOM -> "*mut " + Crossing.word(it.word());
            case SLICE -> "*const " + Crossing.word(it.word());
        }).collect(Collectors.joining(", "));
        return "unsafe extern \"C\" fn(" + parameters + ")"
                + (answers == null ? "" : " -> " + Crossing.word(answers));
    }

    // ---------------------------------------------------------------------------------------------
    // A module.

    private void module(Manifest.Module module) {
        RustModule at = moduleAt(RustNames.modulePath(module.name()));
        for (Declaration declaration : module.declarations()) {
            Declared it = declared.get(module.name() + "." + declaration.name());
            switch (declaration) {
                case Declaration.Sum sum -> sum(at, module, it, sum);
                default -> handleType(at, module, it);
            }
        }
        behaviors(at, module);
        values(at, module);
        for (Manifest.Injection injection : module.injections()) {
            BehaviorType it = behaviorTypes.get(module.name() + "." + injection.name());
            if (it != null) {
                injected(at, module, injection, it);
            }
        }
        for (Manifest.Behavior behavior : module.behaviors()) {
            BehaviorType it = behaviorTypes.get(module.name() + "." + behavior.name());
            if (it != null) {
                bound(at, module, behavior, it);
            }
        }
    }

    // ---------------------------------------------------------------------------------------------
    // A handle: what every declared type is held as.

    /** The struct a value of {@code it} is held as, and what reads and writes it. */
    private void handleStruct(RustModule at, Declared it, String doc, RustNames.Claimed methods) {
        for (String fixed : List.of("__word", "__held")) {
            methods.claim(fixed, "the generated `" + fixed + "`");
        }
        at.items.append("""

                /// %s
                #[derive(Clone, Copy)]
                pub struct %s<'run> {
                    pub(crate) value: rt::Value<'run>,
                    pub(crate) library: &'run crate::Library,
                }

                impl<'run> %s<'run> {
                    /// The value at `at`, which the library answered and is good for `'run`.
                    pub(crate) unsafe fn __held(library: &'run crate::Library, at: rt::Word) -> Self {
                        let at = std::ptr::NonNull::new(at.cast_mut())
                            .expect("the library answers a value's address");
                        // SAFETY: what the caller says.
                        %s { value: unsafe { rt::Value::from_address(at) }, library }
                    }

                    /// Where the value stands, to hand to the library.
                    pub(crate) fn __word(self) -> rt::Word {
                        self.value.address().as_ptr().cast_const()
                    }
                """.formatted(doc, it.name(), it.name(), it.name()));
    }

    private void handleType(RustModule at, Manifest.Module module, Declared it) {
        Declaration declaration = it.declaration();
        RustNames.Claimed methods = new RustNames.Claimed("the methods of `" + it.key() + "`");
        for (String fixed : List.of("new", "decode", "encode")) {
            methods.claim(fixed, "the generated `" + fixed + "`");
        }
        List<Manifest.Field> fields = declaration.fields();
        List<String> readers = new ArrayList<>();
        for (Manifest.Field field : fields) {
            String what = "field `" + it.key() + "." + field.name() + "`";
            readers.add(methods.claim(RustNames.identifier(field.name(), what), what));
        }
        handleStruct(at, it, "A value of `" + it.key() + "`, held where the library made it.",
                methods);
        Manifest.Construct construct = Declaration.built(declaration);
        if (construct != null) {
            construct(at, module, it, fields, construct);
        }
        decode(at, it, declaration.decode());
        encode(at, it, declaration.encode());
        for (int field = 0; field < fields.size(); field++) {
            reader(at, module, it, fields.get(field), readers.get(field));
        }
        at.items.append("}\n");
    }

    /** {@code new}: the value, or the invariant it does not hold as an issue. */
    private void construct(RustModule at, Manifest.Module module, Declared it,
                           List<Manifest.Field> fields, Manifest.Construct construct) {
        List<Crossing> takes = crossings(module, fields.stream().map(Manifest.Field::type).toList(),
                construct.takes(), Manifest.Way.GIVEN);
        if (takes == null) {
            return;
        }
        RustNames.Claimed claimed = new RustNames.Claimed("the parameters of `" + it.key()
                + "::new`");
        claimed.claim("run", "the run it is made in");
        List<String> names = new ArrayList<>();
        for (Manifest.Field field : fields) {
            String what = "field `" + it.key() + "." + field.name() + "`";
            names.add(claimed.claim(RustNames.identifier(field.name(), what), what));
        }
        List<String> parameters = new ArrayList<>(List.of("run: &mut crate::Run<'run>"));
        for (int field = 0; field < takes.size(); field++) {
            parameters.add(names.get(field) + ": " + takes.get(field).view());
        }
        String function = symbol(construct.function());
        StringBuilder body = new StringBuilder("        let library = run.library();\n");
        List<String> handed = given(body, "        ", names, takes);
        body.append("        let mut made: rt::Word = std::ptr::null();\n");
        handed.add("&mut made");
        body.append("        let called = run.call(|| unsafe { (library.symbols.").append(function)
                .append(")(").append(String.join(", ", handed)).append(") });\n");
        body.append("        // SAFETY: the library answered the value in this run.\n");
        body.append("        crate::Construction::of(called, || unsafe { Self::__held(library,"
                + " made) })\n");
        at.items.append("""

                    /// A value of `%s`, or an `invariant_violation` where what is handed over does
                    /// not hold what the type states.
                    pub fn new(%s) -> Result<crate::Construction<Self>, crate::Failure> {
                %s    }
                """.formatted(it.key(), String.join(", ", parameters), body));
    }

    /** {@code decode}: a value of the type read out of its external form, or the issues found in it. */
    private void decode(RustModule at, Declared it, @Nullable Function decode) {
        if (decode == null) {
            return;
        }
        at.items.append("""

                    /// A value of `%s` read out of its external form, or the issues found in it.
                    pub fn decode(run: &mut crate::Run<'run>, json: &str)
                        -> Result<crate::Reading<Self>, crate::Failure> {
                        let library = run.library();
                        let length = i64::try_from(json.len()).expect("text's length is a 64-bit count");
                        let mut reading: rt::Word = std::ptr::null();
                        run.call(|| unsafe {
                            (library.symbols.%s)(json.as_ptr(), length, &mut reading)
                        })?;
                        // SAFETY: the library answered the reading, and the value it read, in this run.
                        Ok(unsafe {
                            library.words.reading(reading, |value| {
                                Self::__held(library, value.as_ptr().cast_const())
                            })
                        })
                    }
                """.formatted(it.key(), symbol(decode)));
    }

    /** {@code encode}: the value in its external form. */
    private void encode(RustModule at, Declared it, @Nullable Function encode) {
        if (encode == null) {
            return;
        }
        at.items.append("""

                    /// This value in the external form of `%s`.
                    pub fn encode(&self) -> String {
                        let library = self.library;
                        // SAFETY: the value is good for `'run`, and the text is read before anything
                        // else is made.
                        unsafe { library.words.text((library.symbols.%s)(self.__word())) }
                    }
                """.formatted(it.key(), symbol(encode)));
    }

    private void reader(RustModule at, Manifest.Module module, Declared it, Manifest.Field field,
                        String name) {
        Manifest.Read read = field.read().available();
        if (read == null) {
            return;
        }
        Crossing crossing = crossing(module, field.type(), read.answers(), Manifest.Way.HANDED);
        if (crossing == null) {
            return;
        }
        StringBuilder body = new StringBuilder("        let library = self.library;\n");
        List<String> rooms = rooms(body, "        ", crossing.words());
        List<String> handed = new ArrayList<>(List.of("self.__word()"));
        rooms.forEach(room -> handed.add("&mut " + room));
        body.append("        // SAFETY: the value is good for `'run`, and so is what it holds.\n");
        body.append("        unsafe { (library.symbols.").append(symbol(read.function())).append(")(")
                .append(String.join(", ", handed)).append(") };\n");
        body.append("        unsafe { ").append(crossing.of(rooms)).append(" }\n");
        at.items.append("""

                    /// The `%s` of this value.
                    pub fn %s(&self) -> %s {
                %s    }
                """.formatted(field.name(), name, crossing.owned(), body));
    }

    // ---------------------------------------------------------------------------------------------
    // A sum.

    private void sum(RustModule at, Manifest.Module module, Declared it, Declaration.Sum sum) {
        RustNames.Claimed methods = new RustNames.Claimed("the methods of `" + it.key() + "`");
        for (String fixed : List.of("case", "decode", "encode")) {
            methods.claim(fixed, "the generated `" + fixed + "`");
        }
        handleStruct(at, it, "A value of `" + it.key() + "`: one of its cases, which `case` says.",
                methods);
        decode(at, it, sum.decode());
        encode(at, it, sum.encode());
        List<CaseArm> arms = arms(it, sum);
        if (arms != null) {
            StringBuilder matched = new StringBuilder();
            for (int place = 0; place < arms.size(); place++) {
                CaseArm arm = arms.get(place);
                matched.append("            ").append(place).append(" => ").append(it.name())
                        .append("Case::").append(arm.variant()).append(arm.made().isEmpty() ? ""
                                : "(unsafe { " + arm.made() + " })").append(",\n");
            }
            at.items.append("""

                        /// Which case this value is, as the value of that case.
                        pub fn case(&self) -> %sCase%s {
                            let library = self.library;
                            let value = self.__word();
                            // SAFETY: the value is good for `'run`, and so is what it is of each case.
                            match unsafe { (library.symbols.%s)(value) } {
                    %s            _ => unreachable!("the library answered a case `%s` does not have"),
                            }
                        }
                    """.formatted(it.name(), lifetime(arms), symbol(Objects.requireNonNull(
                    sum.which())), matched, it.key()));
        }
        at.items.append("}\n");
        if (arms != null) {
            StringBuilder variants = new StringBuilder();
            for (CaseArm arm : new LinkedHashSet<>(arms)) {
                variants.append("    ").append(arm.variant())
                        .append(arm.holds().isEmpty() ? "" : "(" + arm.holds() + ")").append(",\n");
            }
            at.items.append("""

                    /// The case a value of `%s` is, holding the value of that case.
                    pub enum %sCase%s {
                    %s}
                    """.formatted(it.key(), it.name(), lifetime(arms), variants));
        }
        // A value of a case, or of a narrower sum, is a value of the sum as it is.
        Set<String> cases = cases(sum);
        for (Declared other : declared.values()) {
            boolean within = switch (other.declaration()) {
                case Declaration.Sum narrower -> other != it && cases.containsAll(cases(narrower))
                        && !cases(narrower).equals(cases);
                default -> cases.contains(other.key());
            };
            if (within) {
                at.items.append("""

                        impl<'run> From<%s<'run>> for %s<'run> {
                            fn from(it: %s<'run>) -> Self {
                                %s { value: it.value, library: it.library }
                            }
                        }
                        """.formatted(other.type(), it.name(), other.type(), it.name()));
            }
        }
    }

    /** {@code <'run>} where a case holds a value of the run, and nothing where none does. */
    private static String lifetime(List<CaseArm> arms) {
        return arms.stream().anyMatch(it -> it.holds().contains("'run")) ? "<'run>" : "";
    }

    /** One case of a sum as {@code case} answers it: its variant, what it holds, and how it is made. */
    private record CaseArm(String variant, String holds, String made) {
    }

    /** The variant every case of a sum the model keeps is, holding the value as the sum. */
    private static final String KEPT = "Kept";

    /**
     * Each case of {@code sum} in the order its {@code which} counts them, or null where a case has
     * no variant this binding can write: where nothing says which case a value is, a primitive
     * Rust holds no way, or two cases that would be one variant. A case the model keeps, a
     * declared type it does not publish, is {@value #KEPT}, holding the value as the sum, as the
     * PHP binding holds one as the sum's own class.
     */
    private @Nullable List<CaseArm> arms(Declared of, Declaration.Sum sum) {
        if (sum.which() == null) {
            return null;
        }
        List<CaseArm> arms = new ArrayList<>();
        Set<String> variants = new java.util.HashSet<>();
        CaseArm kept = new CaseArm(KEPT, of.type() + "<'run>", "*self");
        for (Case each : sum.cases()) {
            CaseArm arm = switch (each) {
                case Case.Declared d -> {
                    Declared it = declared.get(d.module() + "." + d.name());
                    yield it == null ? kept : new CaseArm(it.name(), it.type() + "<'run>",
                            it.type() + "::__held(library, value)");
                }
                case Case.Primitive p -> {
                    Manifest.CaseCrossing crossing = manifest.crossing(p);
                    Word held = crossing.holds();
                    Crossing.Whole whole = held == null ? null : Crossing.Whole.primitive(p.name(), held);
                    yield whole == null ? null : new CaseArm(p.name(), whole.owned(), whole.of(
                            List.of("(library.symbols." + symbol(Objects.requireNonNull(
                                    crossing.read())) + ")(value)")));
                }
                case Case.Language l -> RustNames.takes(RustNames.capitalized(l.name()))
                        ? new CaseArm(RustNames.capitalized(l.name()), "", "") : null;
            };
            if (arm == null || !RustNames.takes(arm.variant())
                    || !variants.add(arm.variant()) && arm != kept) {
                return null;
            }
            arms.add(arm);
        }
        boolean keeps = arms.contains(kept);
        if (keeps && arms.stream().anyMatch(it -> it != kept && it.variant().equals(KEPT))) {
            return null;
        }
        return arms;
    }

    private static Set<String> cases(Declaration.Sum sum) {
        return sum.cases().stream().map(it -> switch (it) {
            case Case.Declared d -> d.module() + "." + d.name();
            case Case.Primitive p -> "primitive:" + p.name();
            case Case.Language l -> "language:" + l.name();
        }).collect(Collectors.toCollection(LinkedHashSet::new));
    }

    // ---------------------------------------------------------------------------------------------
    // Behaviors and values.

    private void behaviors(RustModule at, Manifest.Module module) {
        for (Manifest.Behavior behavior : module.behaviors()) {
            Manifest.Call call = behavior.call().available();
            if (call == null || behavior.answers().type() instanceof Type.Union) {
                continue;
            }
            String key = module.name() + "." + behavior.name();
            if (!requiresOf(key).isEmpty()) {
                continue;
            }
            List<Crossing> takes = crossings(module, behavior.parameters().types(),
                    call.signature().takes(), Manifest.Way.GIVEN);
            Crossing answers = crossing(module, behavior.answers().type(),
                    call.signature().answers(), Manifest.Way.HANDED);
            if (takes == null || answers == null) {
                continue;
            }
            String what = "behavior `" + key + "`";
            String name = at.values.claim(RustNames.identifier(behavior.name(), what), what);
            RustNames.Claimed claimed = new RustNames.Claimed("the parameters of " + what);
            claimed.claim("run", "the run it is called in");
            List<String> names = switch (behavior.parameters()) {
                case Manifest.Parameters.Named named -> named.parameters().stream()
                        .map(it -> claimed.claim(RustNames.identifier(it.name(),
                                "parameter `" + it.name() + "` of " + what),
                                "parameter `" + it.name() + "`"))
                        .toList();
                case Manifest.Parameters.Positional positional -> java.util.stream.IntStream
                        .range(0, positional.types().size()).mapToObj(it -> "input" + it).toList();
            };
            at.items.append(call("Calls " + what + ".", name, names, takes, answers,
                    call.function(), "std::ptr::null()"));
        }
    }

    private void values(RustModule at, Manifest.Module module) {
        for (Manifest.PublishedValue value : module.values()) {
            Manifest.Call read = value.read().available();
            if (read == null) {
                continue;
            }
            Crossing answers = crossing(module, value.type(), read.signature().answers(),
                    Manifest.Way.HANDED);
            if (answers == null) {
                continue;
            }
            String what = "value `" + module.name() + "." + value.name() + "`";
            String name = at.values.claim(RustNames.identifier(value.name(), what), what);
            at.items.append(call("Reads " + what + ".", name, List.of(), List.of(), answers,
                    read.function(), null));
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
     * A function called {@code name}, calling {@code function} in the run it is handed and answering
     * what it wrote. {@code requirements} is what a behavior is called with first, and null for a
     * value, which is called with nothing more.
     */
    private String call(String doc, String name, List<String> names, List<Crossing> takes,
                        Crossing answers, Function function, @Nullable String requirements) {
        return call(doc, "", "pub fn " + name + "<'run>", "", "", names, takes, answers, function,
                requirements);
    }

    /**
     * A function declared as {@code declared}, indented by {@code indent}, calling {@code function}
     * in the run it is handed and answering what it wrote. {@code receiver} is what it takes ahead
     * of the run, {@code preamble} what it works out first, and {@code requirements} what a
     * behavior is called with first, or null for a value, which is called with nothing more.
     */
    private String call(String doc, String indent, String declared, String receiver,
                        String preamble, List<String> names, List<Crossing> takes, Crossing answers,
                        Function function, @Nullable String requirements) {
        List<String> parameters = new ArrayList<>(List.of(receiver + "run: &mut crate::Run<'run>"));
        for (int at = 0; at < takes.size(); at++) {
            parameters.add(names.get(at) + ": " + takes.get(at).view());
        }
        String inner = indent + "    ";
        StringBuilder body = new StringBuilder(inner + "let library = run.library();\n");
        body.append(preamble.lines().map(it -> inner + it + "\n").collect(Collectors.joining()));
        List<String> handed = new ArrayList<>();
        if (requirements != null) {
            handed.add(requirements);
        }
        handed.addAll(given(body, inner, names, takes));
        List<String> rooms = rooms(body, inner, answers.words());
        rooms.forEach(room -> handed.add("&mut " + room));
        body.append(inner).append("run.call(|| unsafe { (library.symbols.").append(symbol(function))
                .append(")(").append(String.join(", ", handed)).append(") })?;\n");
        body.append(inner).append("// SAFETY: the library answered what it wrote in this run.\n");
        body.append(inner).append("Ok(unsafe { ").append(answers.of(rooms)).append(" })\n");
        return "\n" + indent + "/// " + doc + "\n" + indent + declared + "("
                + String.join(", ", parameters) + ") -> Result<" + answers.owned()
                + ", crate::Failure> {\n" + body + indent + "}\n";
    }

    /**
     * Writes into {@code body} the words handing each of {@code names} over as its crossing says,
     * each into a local of its own before the call, and answers the locals in order.
     */
    private static List<String> given(StringBuilder body, String indent, List<String> names,
                                      List<Crossing> takes) {
        List<String> handed = new ArrayList<>();
        int word = 0;
        for (int at = 0; at < takes.size(); at++) {
            for (String expression : takes.get(at).given(names.get(at))) {
                String local = "given" + word++;
                body.append(indent).append("let ").append(local).append(" = ").append(expression)
                        .append(";\n");
                handed.add(local);
            }
        }
        return handed;
    }

    /** Writes into {@code body} room for each of {@code words}, and answers what each is called. */
    private static List<String> rooms(StringBuilder body, String indent, List<Word> words) {
        List<String> rooms = new ArrayList<>();
        for (int at = 0; at < words.size(); at++) {
            String room = "answer" + at;
            body.append(indent).append("let mut ").append(room).append(": ")
                    .append(Crossing.word(words.get(at))).append(" = ")
                    .append(Crossing.nothing(words.get(at))).append(";\n");
            rooms.add(room);
        }
        return rooms;
    }

    // ---------------------------------------------------------------------------------------------
    // A behavior as an application holds one.

    /**
     * The type a behavior is written as: a trait a host implements, with the type an implementation
     * is made into beside it, where the library asks a host to implement the behavior; and
     * otherwise a type bound to what the behavior requires.
     *
     * @param type the name of the trait or the type, in its module
     */
    private record BehaviorType(String module, String name, List<String> path, String type,
                                boolean injected) {

        String key() {
            return module + "." + name;
        }

        /** What stands for the behavior where another requires it, from the root of the crate. */
        String requirement() {
            return "crate::" + String.join("::", path) + "::" + type
                    + (injected ? IMPLEMENTATION : "");
        }
    }

    /** What the type an implementation of a behavior is made into is called, after the trait. */
    private static final String IMPLEMENTATION = "Implementation";

    /**
     * Which behaviors are written as a type, of every module: each a host implements and can be
     * handed across to, and each published behavior a host can call whose every requirement has a
     * type too, since binding it hands one of each over.
     *
     * <p>The type's name is this generator's, the behavior's made capital, as the PHP binding names
     * a behavior's class. So a name Rust will not take, or one another type of the module already
     * is, leaves the behavior with no type rather than refusing the binding; what requires it has
     * none either. What the model itself names is refused where Rust will not take it, as before.
     */
    private void behaviorTypes() {
        Map<String, BehaviorType> candidates = new LinkedHashMap<>();
        Map<String, List<Manifest.Required>> requires = new LinkedHashMap<>();
        for (Manifest.Module module : manifest.modules()) {
            List<String> path = RustNames.modulePath(module.name());
            for (Manifest.Injection injection : module.injections()) {
                if (implementable(module, injection)) {
                    BehaviorType it = new BehaviorType(module.name(), injection.name(), path,
                            RustNames.capitalized(injection.name()), true);
                    candidates.put(it.key(), it);
                    requires.put(it.key(), List.of());
                }
            }
            for (Manifest.Behavior behavior : module.behaviors()) {
                if (callable(module, behavior)) {
                    BehaviorType it = new BehaviorType(module.name(), behavior.name(), path,
                            RustNames.capitalized(behavior.name()), false);
                    candidates.put(it.key(), it);
                    requires.put(it.key(), requiresOf(it.key()));
                }
            }
        }
        Map<String, Long> spelt = new LinkedHashMap<>();
        for (BehaviorType it : candidates.values()) {
            for (String name : names(it)) {
                spelt.merge(String.join("::", it.path()) + "::" + name, 1L, Long::sum);
            }
        }
        candidates.values().removeIf(it -> names(it).stream().anyMatch(name ->
                !RustNames.takes(name) || RustNames.identifier(name, name).startsWith("r#")
                        || moduleAt(it.path()).types.has(name)
                        || spelt.get(String.join("::", it.path()) + "::" + name) > 1));
        boolean dropped = true;
        while (dropped) {
            dropped = candidates.values().removeIf(it -> requires.get(it.key()).stream()
                    .anyMatch(required -> !candidates.containsKey(required.key())));
        }
        for (BehaviorType it : candidates.values()) {
            for (String name : names(it)) {
                moduleAt(it.path()).types.claim(name, "the type of behavior `" + it.key() + "`");
            }
        }
        behaviorTypes.putAll(candidates);
    }

    /** The names the type of {@code it} takes in its module. */
    private static List<String> names(BehaviorType it) {
        return it.injected() ? List.of(it.type(), it.type() + IMPLEMENTATION) : List.of(it.type());
    }

    /** Whether a host can be handed what {@code injection} takes and hand back what it answers. */
    private boolean implementable(Manifest.Module module, Manifest.Injection injection) {
        return crossings(module, injection.parameters().stream().map(Manifest.NamedParameter::type)
                .toList(), injection.signature().takes(), Manifest.Way.HANDED) != null
                && !(injection.answers() instanceof Type.Union)
                && crossing(module, injection.answers(), injection.signature().answers(),
                Manifest.Way.GIVEN) != null;
    }

    /** Whether a host can call {@code behavior}, handing over what it takes and handed what it answers. */
    private boolean callable(Manifest.Module module, Manifest.Behavior behavior) {
        Manifest.Call call = behavior.call().available();
        return call != null && !(behavior.answers().type() instanceof Type.Union)
                && crossings(module, behavior.parameters().types(), call.signature().takes(),
                Manifest.Way.GIVEN) != null
                && crossing(module, behavior.answers().type(), call.signature().answers(),
                Manifest.Way.HANDED) != null;
    }

    /** The names a behavior's parameters are written under, the run's name taken already. */
    private static List<String> parameterNames(Manifest.Parameters parameters, String what,
                                               String... taken) {
        RustNames.Claimed claimed = new RustNames.Claimed("the parameters of " + what);
        for (String it : taken) {
            claimed.claim(it, "the generated `" + it + "`");
        }
        return switch (parameters) {
            case Manifest.Parameters.Named named -> named.parameters().stream()
                    .map(it -> claimed.claim(RustNames.identifier(it.name(),
                            "parameter `" + it.name() + "` of " + what),
                            "parameter `" + it.name() + "`"))
                    .toList();
            case Manifest.Parameters.Positional positional -> java.util.stream.IntStream
                    .range(0, positional.types().size()).mapToObj(it -> "input" + it).toList();
        };
    }

    /**
     * The trait a host implements {@code injection} as, the type an implementation is made into
     * that the library calls it through, and the function the library calls: it makes Rust values
     * of what the library handed over, calls the implementation in the run of the call that
     * reached it, and writes what it answered through the room the library handed over.
     */
    private void injected(RustModule at, Manifest.Module module, Manifest.Injection injection,
                          BehaviorType it) {
        List<Crossing> takes = Objects.requireNonNull(crossings(module, injection.parameters()
                .stream().map(Manifest.NamedParameter::type).toList(),
                injection.signature().takes(), Manifest.Way.HANDED));
        Crossing answers = Objects.requireNonNull(crossing(module, injection.answers(),
                injection.signature().answers(), Manifest.Way.GIVEN));
        String what = "behavior `" + it.key() + "`";
        List<String> names = parameterNames(new Manifest.Parameters.Named(injection.parameters()),
                what, "run", "self");
        String trait = it.type();
        String implementation = trait + IMPLEMENTATION;
        String dispatch = "__" + trait + "Dispatch";
        String entry = "__" + trait + "_implementation";
        symbols.putIfAbsent(injection.implement(), "rt::ImplementFn");

        List<String> parameters = new ArrayList<>();
        for (int place = 0; place < takes.size(); place++) {
            parameters.add(names.get(place) + ": " + takes.get(place).owned());
        }
        // What the library calls: what it was handed first, what the behavior takes, and room for
        // what it answers, as the manifest says the implementation's type is.
        List<String> cParameters = new ArrayList<>();
        List<String> handed = new ArrayList<>();
        List<String> rooms = new ArrayList<>();
        for (Parameter parameter : injection.implementation().takes()) {
            if (parameter.word() == Word.USERDATA && parameter.mode() == Parameter.Mode.GIVEN
                    && cParameters.isEmpty()) {
                cParameters.add("userdata: *mut std::ffi::c_void");
            } else if (parameter.mode() == Parameter.Mode.GIVEN) {
                String name = "handed" + handed.size();
                handed.add(name);
                cParameters.add(name + ": " + Crossing.word(parameter.word()));
            } else {
                String name = "answer" + rooms.size();
                rooms.add(name);
                cParameters.add(name + ": *mut " + Crossing.word(parameter.word()));
            }
        }
        StringBuilder made = new StringBuilder();
        int word = 0;
        List<String> arguments = new ArrayList<>();
        for (int place = 0; place < takes.size(); place++) {
            int wide = takes.get(place).words().size();
            made.append("        let ").append(names.get(place)).append(" = unsafe { ")
                    .append(takes.get(place).of(handed.subList(word, word + wide))).append(" };\n");
            word += wide;
            arguments.add(names.get(place));
        }
        StringBuilder written = new StringBuilder();
        List<String> given = answers.given("answer");
        for (int place = 0; place < given.size(); place++) {
            written.append("        let given").append(place).append(" = ").append(given.get(place))
                    .append(";\n");
        }
        for (int place = 0; place < given.size(); place++) {
            written.append("        unsafe { *").append(rooms.get(place)).append(" = given")
                    .append(place).append(" };\n");
        }
        at.items.append("""

                /// What implements `%s`, which the library asks a host to implement. Made into a
                /// [`%s`], it is handed to what requires the behavior, and the library calls
                /// `apply` wherever what was bound to it reaches the behavior.
                pub trait %s {
                    /// Answers `%s` in `run`, the run of the call that reached it. A failure answered
                    /// here comes back out of that call, as a panic here does.
                    fn apply<'run>(&self, run: &mut crate::Run<'run>%s) -> Result<%s, crate::HostError>;
                }

                struct %s<'a> {
                    library: &'a crate::Library,
                    implementation: Box<dyn %s + 'a>,
                }

                /// An implementation of `%s`, made into a capability the library calls it through.
                pub struct %s<'a> {
                    implemented: rt::Implemented<%s<'a>>,
                }

                impl<'a> %s<'a> {
                    /// `implementation`, as `library` calls it.
                    pub fn new(library: &'a crate::Library, implementation: impl %s + 'a) -> Self {
                        let dispatch = %s { library, implementation: Box::new(implementation) };
                        // SAFETY: the function is the library's making a capability of `%s`, and the
                        // entry below is of the type its implementation is, reading the dispatch.
                        let implemented = unsafe {
                            rt::Implemented::new(
                                rt::Loaded::runtime(library),
                                library.symbols.%s,
                                %s as *const std::ffi::c_void,
                                dispatch,
                            )
                        };
                        %s { implemented }
                    }
                }

                impl rt::Requirement for %s<'_> {
                    fn capability(&self) -> std::ptr::NonNull<rt::Capability> {
                        rt::Requirement::capability(&self.implemented)
                    }

                    fn made(&self) -> rt::Made {
                        rt::Requirement::made(&self.implemented)
                    }
                }

                unsafe extern "C" fn %s(%s) -> u32 {
                    // SAFETY: what the library hands first is what the capability was made with, the
                    // dispatch, which lives for as long as the capability may be called.
                    let dispatch = unsafe { &*userdata.cast::<%s<'_>>() };
                    let library = dispatch.library;
                    rt::implemented(library, |run| {
                        // SAFETY: the library handed these over in the run of the call reaching this.
                %s        let answer = dispatch.implementation.apply(run%s)?;
                        let answer = %s;
                %s        Ok(())
                    })
                }
                """.formatted(it.key(), implementation, trait, it.key(),
                parameters.stream().map(p -> ", " + p).collect(Collectors.joining()),
                answers.owned(), dispatch, trait, it.key(), implementation, dispatch,
                implementation, trait, dispatch, it.key(), injection.implement(), entry,
                implementation, implementation, entry, String.join(", ", cParameters), dispatch,
                made, arguments.stream().map(a -> ", " + a).collect(Collectors.joining()),
                answers.viewOf("(&answer)"), written));
    }

    /**
     * The type an application binds {@code behavior} through and calls it on: {@code bind}, taking
     * what stands for each behavior it requires, or {@code new} where it requires none, and
     * {@code call}, calling it with the capabilities of what it was bound to, in the caller's run.
     */
    private void bound(RustModule at, Manifest.Module module, Manifest.Behavior behavior,
                       BehaviorType it) {
        Manifest.Call call = Objects.requireNonNull(behavior.call().available());
        List<Crossing> takes = Objects.requireNonNull(crossings(module,
                behavior.parameters().types(), call.signature().takes(), Manifest.Way.GIVEN));
        Crossing answers = Objects.requireNonNull(crossing(module, behavior.answers().type(),
                call.signature().answers(), Manifest.Way.HANDED));
        String what = "behavior `" + it.key() + "`";
        List<String> names = parameterNames(behavior.parameters(), what, "run", "self");
        List<Manifest.Required> requires = requiresOf(it.key());
        Manifest.Construction construction = null;
        for (Manifest.Module each : manifest.modules()) {
            for (Manifest.Construction c : each.constructions()) {
                if ((each.name() + "." + c.name()).equals(it.key())) {
                    construction = c;
                }
            }
        }
        String bind = construction == null || construction.bind() == null ? "None"
                : "Some(library.symbols." + symbol(construction.bind()) + ")";
        // A requirement is named after the behavior it is where no other is of that name, and
        // after its place otherwise, as the PHP binding names one.
        Map<String, Long> counted = requires.stream().collect(Collectors.groupingBy(
                Manifest.Required::name, Collectors.counting()));
        List<String> requirementNames = new ArrayList<>();
        List<String> requirementParameters = new ArrayList<>();
        for (int place = 0; place < requires.size(); place++) {
            Manifest.Required required = requires.get(place);
            String name = counted.get(required.name()) == 1 && RustNames.takes(required.name())
                    && !RustNames.identifier(required.name(), required.name()).startsWith("r#")
                    && !required.name().equals("library")
                    ? required.name() : "dependency" + place;
            requirementNames.add(name);
            requirementParameters.add(", " + name + ": &'a "
                    + behaviorTypes.get(required.key()).requirement() + "<'_>");
        }
        String constructor = requires.isEmpty() ? """
                    /// `%s`, which requires nothing.
                    pub fn new(library: &'a crate::Library) -> Self {
                """.formatted(it.key()) : """
                    /// `%s`, bound to what stands for each behavior it requires.
                    pub fn bind(library: &'a crate::Library%s) -> Self {
                """.formatted(it.key(), String.join("", requirementParameters));
        at.items.append("""

                /// `%s` as an application holds it: bound to what stands for each behavior it
                /// requires, which each call is made with.
                pub struct %s<'a> {
                    bound: rt::Bound<'a>,
                }

                impl<'a> %s<'a> {
                %s        let requires: [&'a dyn rt::Requirement; %d] = [%s];
                        // SAFETY: the function is the library's making the capability of `%s`, and
                        // what is handed stands for what it requires, in order.
                        let bound = unsafe { rt::Bound::new(rt::Loaded::runtime(library), %s, &requires) };
                        %s { bound }
                    }
                """.formatted(it.key(), it.type(), it.type(), constructor, requires.size(),
                String.join(", ", requirementNames), it.key(), bind, it.type()));
        at.items.append(call("Calls `" + it.key() + "` with what this was bound to.", "    ",
                "pub fn call<'run>", "&self, ",
                "let requirements = self.bound.requirements(rt::Loaded::runtime(library));",
                names, takes, answers, call.function(), "requirements"));
        at.items.append("""
                }

                impl rt::Requirement for %s<'_> {
                    fn capability(&self) -> std::ptr::NonNull<rt::Capability> {
                        rt::Requirement::capability(&self.bound)
                    }

                    fn made(&self) -> rt::Made {
                        rt::Requirement::made(&self.bound)
                    }
                }
                """.formatted(it.type()));
    }

    // ---------------------------------------------------------------------------------------------
    // The crate.

    private void library() throws IOException {
        RustModule root = tree.get(List.of());
        StringBuilder modules = new StringBuilder();
        for (String child : root.children) {
            modules.append("pub mod ").append(child).append(";\n");
        }
        String rust = header() + """
                //! The Rust binding of a Souther library built by souther-native-compiler: a module for
                //! each module of the model, and `Library`, which loads the library and opens a run
                //! of it.

                #![allow(non_snake_case, non_camel_case_types, unused_unsafe, unused_parens, clippy::all)]

                pub use souther_binding_runtime::{
                    AlreadyRunning, Construction, Decimal, Failure, HostError, LoadError, NotADecimal,
                    Reading, raoh,
                };

                mod __ffi;
                %s
                /// A run of the library, as every function of the binding takes one.
                pub type Run<'run> = souther_binding_runtime::Run<'run, Library>;

                /// A run as a closure is handed it, which derefs to the [`Run`].
                pub type Scope<'run, 'outer> = souther_binding_runtime::Scope<'run, 'outer, Library>;

                /// What the library numbers the statuses its functions answer.
                const STATUSES: &[(&str, u32)] = &[%s];

                /// What the library numbers the outcomes a reading comes to.
                const OUTCOMES: &[(&str, i32)] = &[%s];

                /// The library this binding was generated for, loaded from a path.
                pub struct Library {
                    pub(crate) symbols: __ffi::Symbols,
                    pub(crate) words: souther_binding_runtime::Words,
                    runtime: souther_binding_runtime::Runtime,
                    // Dropped last, unloading the library once nothing above can be called.
                    _native: souther_binding_runtime::NativeLibrary,
                }

                impl Library {
                    /// The library at `path`.
                    ///
                    /// # Safety
                    ///
                    /// `path` is the library this binding was generated from: every function is
                    /// called as what the manifest the binding was generated from says it is, and a
                    /// library built from another program may name one alike that is something else.
                    ///
                    /// # Errors
                    ///
                    /// Where no library can be loaded from `path`, or it has none of a function the
                    /// binding calls.
                    pub unsafe fn load(path: impl AsRef<std::path::Path>) -> Result<Self, LoadError> {
                        // SAFETY: what the caller says.
                        unsafe {
                            let native = souther_binding_runtime::NativeLibrary::load(path)?;
                            let runtime = native.runtime(STATUSES)?;
                            let words = souther_binding_runtime::Words::load(&native, OUTCOMES)?;
                            let symbols = __ffi::Symbols::load(&native)?;
                            Ok(Library { symbols, words, runtime, _native: native })
                        }
                    }

                    /// Opens a run of the library on this thread, hands it to `f`, and drops
                    /// everything made in it once `f` has answered.
                    ///
                    /// # Errors
                    ///
                    /// [`AlreadyRunning`] where a run of this library is open on this thread already;
                    /// a run inside it is opened from it, with `scope`.
                    pub fn run<'lib, R>(
                        &'lib self,
                        f: impl for<'run> FnOnce(&mut Scope<'run, 'lib>) -> R,
                    ) -> Result<R, AlreadyRunning> {
                        souther_binding_runtime::run(self, f)
                    }
                }

                impl souther_binding_runtime::Loaded for Library {
                    fn runtime(&self) -> &souther_binding_runtime::Runtime {
                        &self.runtime
                    }
                }
                """.formatted(modules, numbers(manifest.statuses()), numbers(manifest.outcomes()));
        file(List.of("src", "lib.rs"), rust);
    }

    private static String numbers(Map<String, Integer> numbers) {
        return new TreeMap<>(numbers).entrySet().stream()
                .map(it -> "(\"" + it.getKey() + "\", " + it.getValue() + ")")
                .collect(Collectors.joining(", "));
    }

    private void ffi() throws IOException {
        StringBuilder fields = new StringBuilder();
        StringBuilder loads = new StringBuilder();
        for (Map.Entry<String, String> it : symbols.entrySet()) {
            fields.append("    pub(crate) ").append(it.getKey()).append(": ").append(it.getValue())
                    .append(",\n");
            loads.append("                ").append(it.getKey()).append(": library.function(\"")
                    .append(it.getKey()).append("\")?,\n");
        }
        String rust = header() + """
                //! Every function of the library the binding calls, as the manifest says it is.

                use souther_binding_runtime as rt;

                pub(crate) struct Symbols {
                %s}

                impl Symbols {
                    /// Each function, looked up in `library`.
                    ///
                    /// # Safety
                    ///
                    /// `library` is the one the binding was generated from, and stays loaded for as
                    /// long as anything here is called.
                    pub(crate) unsafe fn load(library: &rt::NativeLibrary) -> Result<Self, rt::LoadError> {
                        // SAFETY: what the caller says; each is of the type the manifest says.
                        unsafe {
                            Ok(Symbols {
                %s            })
                        }
                    }
                }
                """.formatted(fields, loads);
        file(List.of("src", "__ffi.rs"), rust);
    }

    private void moduleFile(List<String> path, RustModule module) throws IOException {
        StringBuilder rust = new StringBuilder(header());
        rust.append("//! Module `").append(String.join(".", path.stream()
                .map(RustNames::fileName).toList())).append("` of the model.\n\n");
        rust.append("#[allow(unused_imports)]\nuse souther_binding_runtime as rt;\n");
        for (String child : module.children) {
            rust.append("pub mod ").append(child).append(";\n");
        }
        rust.append(module.items);
        List<String> file = new ArrayList<>(List.of("src"));
        path.forEach(it -> file.add(RustNames.fileName(it)));
        file.add("mod.rs");
        this.file(file, rust.toString());
    }

    private void cargo() throws IOException {
        String toml = """
                # Generated by souther-native-compiler from souther.json. Written again on every build.
                [package]
                name = "%s"
                version = "0.0.0"
                edition = "2024"
                rust-version = "1.87"
                publish = false

                [dependencies]
                souther-binding-runtime = "%s"
                """.formatted(crate, RUNTIME_VERSION);
        file(List.of("Cargo.toml"), toml);
    }

    private static String header() {
        return "// Generated by souther-native-compiler from souther.json. Written again on every"
                + " build.\n\n";
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
