package souther.nativecode;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.Declarations;
import souther.bindings.Manifest;
import souther.bindings.NotBindable;
import souther.compiler.diag.CompileException;
import souther.compiler.meta.ModulePath;
import souther.compiler.program.CheckedProgram;

import java.io.File;
import java.io.IOException;
import java.io.PrintStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.stream.Stream;

/**
 * Compiles the sources named on the command line into one object file, or builds them into a
 * library for a host and, if asked, the PHP and the Rust binding of it.
 *
 * <p>Nothing here decides what a library or a binding is. The program is checked once, the driver
 * writes the library from it ({@link NativeCompiler#library}), and each binding is generated from
 * the manifest the driver wrote ({@link BindingGenerator#generate}), never from the program a
 * second time. A binding is written by a generator's jar: one the catalog names
 * ({@link KnownBindings}), or one named with {@code --binding} by its coordinate and digest or its
 * path. Both come to one {@link GeneratorSpec}, and are run the one way {@link Bindings} runs a jar.
 *
 * <p>Each directory is replaced whole by what writes it, and each is a directory of its own: a
 * binding refused after its library was written leaves the new library and the binding that was
 * there before. What a binding would be refused for whatever the manifest says, a name its language
 * will not take or a directory holding what no generation wrote, is refused before the library is
 * built.
 */
public final class Main {

    private static final int WROTE_IT = 0;
    private static final int REFUSED = 1;
    private static final int WRONG_COMMAND = 2;

    private static final String USAGE = usage();

    private static String usage() {
        StringBuilder asked = new StringBuilder();
        StringBuilder described = new StringBuilder();
        for (KnownBindings.Kind kind : KnownBindings.all()) {
            asked.append(asked.isEmpty() ? "" : " ").append("[").append(kind.flag())
                    .append(" <dir>");
            described.append("  %-19s where to write the %s binding of the library\n"
                    .formatted(kind.flag() + " <dir>", kind.display()));
            for (String option : kind.options()) {
                asked.append(" --").append(option).append(" <value>");
                described.append("  %-19s an option of the %s binding\n"
                        .formatted("--" + option + " <value>", kind.display()));
            }
            asked.append("]");
        }
        return "usage: souther-native [--offline] [-cp <path>] -o <object> <source>...\n"
                + "       souther-native [--offline] [-cp <path>] --library <dir> [--with <object>]...\n"
                + "                      " + asked + "\n"
                + "                      [--binding <generator> <dir> [--binding-option <key>=<value>]...]...\n"
                + "                      <source>...\n"
                + "       souther-native --fetch\n"
                + "\n"
                + "  --offline           fetch nothing: use only the driver and the generators already kept\n"
                + "  --fetch             fetch the driver and every generator this does not have, and end\n"
                + "  -cp <path>          the class path of other builds the program imports, as the souther\n"
                + "                      command takes it (also --class-path)\n"
                + "  -o <object>         where to write the object file\n"
                + "  --library <dir>     where to write the library: object, headers, manifest, shared library\n"
                + "  --with <object>     the object another build the program reaches was compiled to\n"
                + described
                + "  --binding <generator> <dir>\n"
                + "                      where to write the binding a generator of someone else's writes; the\n"
                + "                      generator is groupId:artifactId:version@sha256:<hex>, or a path to a jar\n"
                + "  --binding-option <key>=<value>\n"
                + "                      an option of the --binding before it, split at the first =\n"
                + "  <source>            a .sou file, or a directory holding some\n";
    }

    /** What a command asks to be written, each of which is a whole command. */
    sealed interface Output {

        /** One object file. */
        record ObjectFile(Path into) implements Output {
        }

        /** What this needs and does not have, fetched: a command of its own, and writes no build. */
        record Fetch() implements Output {
        }

        /** A library, reaching {@code alongside}, and each binding of it asked for. */
        record Library(Path into, List<Path> alongside, List<HostBinding> bindings)
                implements Output {

            public Library {
                alongside = List.copyOf(alongside);
                bindings = List.copyOf(bindings);
            }
        }
    }

    /**
     * The generator a binding was asked for with: a flag of the catalog, or {@code --binding} and a
     * jar. What the command reads and nothing more; which jar that is, is {@link GeneratorSpec#of}.
     */
    sealed interface Asked {

        /** A binding of the catalog, by its flag. */
        record Standard(KnownBindings.Kind kind) implements Asked {
        }

        /** A generator named with {@code --binding}. */
        record External(GeneratorRef ref) implements Asked {
        }
    }

    /**
     * A binding of the library for one host's language, where it is written, and the options it was
     * asked with, each keyed by name. What they mean is its generator's.
     */
    record HostBinding(Asked asked, Path into, Map<String, String> options) {

        HostBinding {
            options = Map.copyOf(options);
        }

        /** The option that asked for it, as a refusal of the command line names it. */
        String option() {
            return switch (asked) {
                case Asked.Standard standard -> standard.kind().flag();
                case Asked.External external -> "--binding " + external.ref();
            };
        }
    }

    /**
     * A command as read: what it writes, from which sources, reading the other builds they import
     * from {@code classPath}, and whether it may fetch what it needs and does not have.
     */
    record Command(Output output, List<Path> sources, List<Path> classPath, boolean offline) {

        CheckedProgram checked(List<String> read) {
            return classPath.isEmpty() ? CheckedProgram.of(read)
                    : CheckedProgram.of(read, ModulePath.ofClassPath(classPath));
        }
    }

    /** A command line that is not a command, and why. */
    static final class NotACommand extends Exception {
        NotACommand(String why) {
            super(why);
        }
    }

    private Main() {
    }

    public static void main(String[] args) {
        System.exit(run(args, System.out, System.err));
    }

    static int run(String[] args, PrintStream out, PrintStream problems) {
        return run(args, out, problems, Fetching.standard());
    }

    static int run(String[] args, PrintStream out, PrintStream problems, Fetching allowed) {
        Command command;
        try {
            command = read(args);
        } catch (NotACommand e) {
            if (e.getMessage() != null) {
                problems.println(e.getMessage());
            }
            problems.print(USAGE);
            return WRONG_COMMAND;
        }
        Fetching fetching = allowed.withOffline(command.offline()).saying(problems);
        if (command.output() instanceof Output.Fetch) {
            return fetched(out, problems, fetching);
        }

        List<Path> files;
        try {
            files = under(command.sources());
        } catch (IOException | UncheckedIOException e) {
            problems.println(e.getMessage());
            return WRONG_COMMAND;
        }
        if (files.isEmpty()) {
            problems.println("nothing to compile: no .sou file among what was named");
            return WRONG_COMMAND;
        }

        try {
            List<String> read = new ArrayList<>(files.size());
            for (Path file : files) {
                read.add(Files.readString(file, StandardCharsets.UTF_8));
            }
            switch (command.output()) {
                case Output.Fetch fetch -> throw new IllegalStateException("handled above");
                case Output.ObjectFile object -> {
                    byte[] written;
                    try (Holding held = new Holding()) {
                        written = NativeCompiler.compile(command.checked(read),
                                driver(held, fetching));
                    }
                    if (object.into().getParent() != null) {
                        Files.createDirectories(object.into().getParent());
                    }
                    Files.write(object.into(), written);
                    out.println("wrote " + object.into() + " from " + sources(files));
                }
                case Output.Library library -> {
                    return written(command, library, files, read, fetching, out, problems);
                }
            }
        } catch (GeneratorFailed e) {
            problems.println(e.getMessage());
            return REFUSED;
        } catch (CompileException e) {
            problems.println(e.getMessage());
            return REFUSED;
        } catch (NotLowered e) {
            // What this backend has not got round to, which is a different thing from a program
            // the language refuses — so it says which it was rather than one word for both.
            problems.println("this backend does not write that yet: " + e.getMessage());
            return REFUSED;
        } catch (IOException e) {
            problems.println(e.getMessage());
            return WRONG_COMMAND;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            problems.println("interrupted while the driver was running");
            return WRONG_COMMAND;
        }
        return WROTE_IT;
    }

    /**
     * The library and each binding of it asked for. Every generator is loaded and asked its
     * {@code preflight}, and every directory checked, before the library is built; every binding is
     * written before any is put in place, so that one refused or failed leaves every directory as it
     * was. What it holds on the way, each generator loaded and each directory being written, is let
     * go of on every way out, whatever was thrown: a generator closed, and a directory not put in
     * place dropped.
     */
    private static int written(Command command, Output.Library library, List<Path> files,
                               List<String> read, Fetching fetching, PrintStream out,
                               PrintStream problems)
            throws IOException, GeneratorFailed, InterruptedException {
        try (Holding held = new Holding()) {
            List<Bindings.Generator> generators = new ArrayList<>();
            for (HostBinding binding : library.bindings()) {
                Bindings.Generator generator =
                        Bindings.load(fetching, GeneratorSpec.of(binding.asked(), fetching));
                held.hold(generator, () -> closed(generator, problems));
                generators.add(generator);
                try {
                    generator.preflight(binding.options());
                    BindingDirectory.replaceable(binding.into(), generator.id());
                } catch (NotBindable e) {
                    problems.println(e.getMessage());
                    return WRONG_COMMAND;
                }
            }
            Path driver = driver(held, fetching);
            List<byte[]> alongside = new ArrayList<>(library.alongside().size());
            for (Path object : library.alongside()) {
                alongside.add(Files.readAllBytes(object));
            }
            NativeCompiler.Library built = NativeCompiler.library(
                    command.checked(read), alongside, library.into(), driver);
            out.println("wrote the library " + library.into() + " from " + sources(files));
            BindingInput input = new BindingInput(ManifestReader.read(built.manifest()),
                    Declarations.at(built.declarations()));
            List<BindingDirectory> staged = new ArrayList<>();
            for (int at = 0; at < generators.size(); at++) {
                HostBinding binding = library.bindings().get(at);
                Bindings.Generator generator = generators.get(at);
                BindingDirectory directory = BindingDirectory.staging(binding.into(),
                        new BindingDirectory.Mark(generator.id(), generator.artifact()));
                held.hold(directory, directory::abandon);
                staged.add(directory);
                try {
                    generator.generate(input, directory.staging(), binding.options());
                } catch (NotBindable e) {
                    problems.println("the binding " + binding.into() + " is not written: "
                            + e.getMessage());
                    return REFUSED;
                }
            }
            for (int at = 0; at < staged.size(); at++) {
                staged.get(at).commit();
                out.println("wrote the binding " + library.bindings().get(at).into() + " with "
                        + generators.get(at).named());
            }
            return WROTE_IT;
        }
    }

    /**
     * Lets go of {@code generator}. A copy of its jar that could not be deleted is said, and left to
     * the system's temporary directory: nothing the command wrote depends on it.
     */
    private static void closed(Bindings.Generator generator, PrintStream problems) {
        try {
            generator.close();
        } catch (IOException e) {
            problems.println("could not delete the copy of " + generator.named() + "'s jar: "
                    + e.getMessage());
        }
    }

    /**
     * The driver a program is handed to: the one {@link NativeCompiler#DRIVER_PROPERTY} names, which a
     * clone's build names and which is not this command's to delete; or, where none is named, the
     * bundle of this release for this platform, unpacked for this command and owned by {@code held}.
     * It is handed to the compiler, and no property is set for it.
     */
    private static Path driver(Holding held, Fetching fetching) throws NotFetched {
        String named = System.getProperty(NativeCompiler.DRIVER_PROPERTY);
        return named != null ? Path.of(named) : NativeBundle.unpacked(held, fetching);
    }

    /**
     * {@code --fetch}: what a command may need and this does not have, fetched and kept, and nothing
     * built. It prepares the cache and nothing that runs: no copy of a jar and no driver unpacked.
     */
    private static int fetched(PrintStream out, PrintStream problems, Fetching fetching) {
        try {
            if (NativeCompiler.hasDriver()) {
                out.println("the driver is this build's own");
            } else {
                out.println("the driver " + NativeBundle.kept(fetching));
            }
            for (KnownBindings.Kind kind : KnownBindings.all()) {
                GeneratorSpec spec = GeneratorSpec.standard(kind, fetching);
                switch (spec.ref()) {
                    case GeneratorRef.Local local ->
                            out.println(spec.display() + " is this build's own, " + local.path());
                    case GeneratorRef.Maven maven -> out.println(spec.display() + " "
                            + GeneratorArtifacts.kept(fetching, maven));
                }
            }
            return WROTE_IT;
        } catch (IOException e) {
            problems.println(e.getMessage());
            return WRONG_COMMAND;
        }
    }

    /**
     * The command {@code args} say, or why they say none: an option that belongs to one output
     * named with the other, or with what it goes with missing, is refused here rather than read as
     * something the command did not mean.
     */
    static Command read(String[] args) throws NotACommand {
        Path object = null;
        Path library = null;
        List<Path> alongside = new ArrayList<>();
        Map<KnownBindings.Kind, Path> asked = new HashMap<>();
        Map<KnownBindings.Kind, Map<String, String>> qualified = new HashMap<>();
        List<Named> external = new ArrayList<>();
        Map<String, String> externalOptions = null;
        List<Path> sources = new ArrayList<>();
        List<Path> classPath = new ArrayList<>();
        boolean offline = false;
        boolean fetch = false;
        for (int at = 0; at < args.length; at++) {
            String held = args[at];
            switch (held) {
                case "--offline" -> offline = true;
                case "--fetch" -> fetch = true;
                case "-o" -> object = once(held, object, Path.of(valueOf(args, ++at, held)));
                case "--library" -> library = once(held, library, Path.of(valueOf(args, ++at, held)));
                case "-cp", "--class-path" -> {
                    for (String entry : valueOf(args, ++at, held).split(File.pathSeparator)) {
                        if (!entry.isBlank()) {
                            classPath.add(Path.of(entry));
                        }
                    }
                }
                case "--with" -> alongside.add(Path.of(valueOf(args, ++at, held)));
                case "--binding" -> {
                    GeneratorRef ref;
                    try {
                        ref = GeneratorRef.parse(valueOf(args, ++at, held));
                    } catch (IllegalArgumentException e) {
                        throw new NotACommand(held + ": " + e.getMessage());
                    }
                    externalOptions = new LinkedHashMap<>();
                    external.add(new Named(ref, Path.of(valueOf(args, ++at, held + " <generator>")),
                            externalOptions));
                }
                case "--binding-option" -> {
                    String option = valueOf(args, ++at, held);
                    if (externalOptions == null) {
                        throw new NotACommand(held + " qualifies the --binding before it, and"
                                + " there is none");
                    }
                    int equals = option.indexOf('=');
                    if (equals <= 0) {
                        throw new NotACommand(held + " is <key>=<value>, and \"" + option
                                + "\" is not");
                    }
                    String key = option.substring(0, equals);
                    if (externalOptions.put(key, option.substring(equals + 1)) != null) {
                        throw new NotACommand(held + " names " + key + " twice for one binding");
                    }
                }
                default -> {
                    Optional<KnownBindings.Kind> asks = KnownBindings.askedBy(held);
                    Optional<KnownBindings.Kind> qualifies = KnownBindings.qualifiedBy(held);
                    if (asks.isPresent()) {
                        // A --binding-option after a flag of the catalog would read as the
                        // flag's, and belongs to no --binding before it.
                        externalOptions = null;
                        asked.put(asks.get(),
                                once(held, asked.get(asks.get()), Path.of(valueOf(args, ++at, held))));
                    } else if (qualifies.isPresent()) {
                        Map<String, String> options = qualified.computeIfAbsent(qualifies.get(),
                                kind -> new HashMap<>());
                        options.put(held.substring(2),
                                once(held, options.get(held.substring(2)), valueOf(args, ++at, held)));
                    } else if (held.startsWith("-")) {
                        throw new NotACommand("no such option: " + held);
                    } else {
                        sources.add(Path.of(held));
                    }
                }
            }
        }
        if (fetch) {
            if (offline) {
                throw new NotACommand("--fetch and --offline ask for opposite things");
            }
            if (object != null || library != null || !sources.isEmpty() || !alongside.isEmpty()
                    || !classPath.isEmpty() || !asked.isEmpty() || !qualified.isEmpty()
                    || !external.isEmpty()) {
                throw new NotACommand("--fetch is a command of its own, and takes nothing else");
            }
            return new Command(new Output.Fetch(), List.of(), List.of(), false);
        }
        if (sources.isEmpty()) {
            throw new NotACommand(null);
        }
        if (object != null && library != null) {
            throw new NotACommand("-o and --library are two commands; name one");
        }
        if (library == null) {
            if (!alongside.isEmpty()) {
                throw new NotACommand("--with goes with --library");
            }
            if (!external.isEmpty()) {
                throw new NotACommand("--binding goes with --library");
            }
            for (KnownBindings.Kind kind : KnownBindings.all()) {
                if (asked.containsKey(kind)) {
                    throw new NotACommand(kind.flag() + " goes with --library");
                }
                for (String option : kind.options()) {
                    if (qualified.getOrDefault(kind, Map.of()).containsKey(option)) {
                        throw new NotACommand("--" + option + " goes with --library");
                    }
                }
            }
            if (object == null) {
                throw new NotACommand(null);
            }
            return new Command(new Output.ObjectFile(object), List.copyOf(sources),
                    List.copyOf(classPath), offline);
        }
        for (KnownBindings.Kind kind : KnownBindings.all()) {
            if (!asked.containsKey(kind) && qualified.containsKey(kind)) {
                throw new NotACommand("--" + qualified.get(kind).keySet().iterator().next()
                        + " goes with " + kind.flag());
            }
        }
        List<String> directories = new ArrayList<>(List.of("--library"));
        List<Path> placed = new ArrayList<>(List.of(library));
        List<HostBinding> bindings = new ArrayList<>();
        for (KnownBindings.Kind kind : KnownBindings.all()) {
            if (asked.containsKey(kind)) {
                bindings.add(new HostBinding(new Asked.Standard(kind), asked.get(kind),
                        qualified.getOrDefault(kind, Map.of())));
            }
        }
        for (Named binding : external) {
            bindings.add(new HostBinding(new Asked.External(binding.ref()), binding.into(),
                    binding.options()));
        }
        for (HostBinding binding : bindings) {
            directories.add(binding.option());
            placed.add(binding.into());
        }
        for (int one = 0; one < placed.size(); one++) {
            for (int other = one + 1; other < placed.size(); other++) {
                if (within(placed.get(one), placed.get(other))
                        || within(placed.get(other), placed.get(one))) {
                    throw new NotACommand(directories.get(other) + " and "
                            + directories.get(one) + " are each replaced whole, so"
                            + " neither can hold the other");
                }
            }
        }
        return new Command(new Output.Library(library, alongside, bindings),
                List.copyOf(sources), List.copyOf(classPath), offline);
    }

    /** A {@code --binding} as read so far: its options are added to as the command line goes on. */
    private record Named(GeneratorRef ref, Path into, Map<String, String> options) {
    }

    private static String valueOf(String[] args, int at, String option) throws NotACommand {
        if (at == args.length) {
            throw new NotACommand(option + " wants a value");
        }
        return args[at];
    }

    private static <T> T once(String option, T before, T now) throws NotACommand {
        if (before != null) {
            throw new NotACommand(option + " is named twice");
        }
        return now;
    }

    private static boolean within(Path inner, Path outer) {
        return inner.toAbsolutePath().normalize().startsWith(outer.toAbsolutePath().normalize());
    }

    private static String sources(List<Path> files) {
        return files.size() + (files.size() == 1 ? " source" : " sources");
    }

    private static List<Path> under(List<Path> named) throws IOException {
        List<Path> files = new ArrayList<>();
        for (Path path : named) {
            if (Files.isDirectory(path)) {
                try (Stream<Path> walked = Files.walk(path)) {
                    walked.filter(it -> it.toString().endsWith(".sou")).sorted().forEach(files::add);
                }
            } else {
                files.add(path);
            }
        }
        return files;
    }
}
