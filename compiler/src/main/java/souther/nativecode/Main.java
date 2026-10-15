package souther.nativecode;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.Declarations;
import souther.bindings.Manifest;
import souther.bindings.NotBindable;
import souther.compiler.CompilationSources;
import souther.compiler.CompilationSources.SourceFile;
import souther.compiler.diag.CompileException;
import souther.compiler.meta.ModulePath;
import souther.compiler.program.CheckedProgram;

import java.io.File;
import java.io.IOException;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/**
 * Compiles the sources named on the command line into one object file, or builds them into a
 * library for a host and, if asked, the bindings of it.
 *
 * <p>Nothing here decides what a library or a binding is. The program is checked once, the driver
 * writes the library from it ({@link NativeCompiler#library}), and each binding is generated from
 * the manifest the driver wrote ({@link BindingGenerator#generate}), never from the program a
 * second time. A binding is written by a generator's jar, which {@code --binding} names: by an id
 * of the catalog ({@link KnownBindings}), or by its coordinate and digest or its path. Both come to
 * one {@link GeneratorSpec}, and are run the one way {@link Bindings} runs a jar.
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
        StringBuilder catalog = new StringBuilder();
        for (KnownBindings.Kind kind : KnownBindings.all()) {
            StringBuilder options = new StringBuilder();
            for (String option : kind.options()) {
                options.append(options.isEmpty() ? "" : ", ").append(option).append("=<value>");
            }
            catalog.append("    %-17s %s binding; options: %s\n"
                    .formatted(kind.id(), kind.display(), options));
        }
        return "usage: souther compile --target native [--offline] [-cp <path>] -o <object> <source>...\n"
                + "       souther compile --target native [--offline] [-cp <path>] --library <dir>\n"
                + "           [--with <object>]...\n"
                + "           [--binding <generator> <dir> [--binding-option <key>=<value>]...]...\n"
                + "           <source>...\n"
                + "       souther compile --target native --fetch\n"
                + "       souther compile --target native --help\n"
                + "\n"
                + "  --offline           fetch nothing: use only the driver and the generators already kept\n"
                + "  --fetch             fetch the driver and every generator this does not have, and end\n"
                + "  --help, -h          write this, and end\n"
                + "  -cp <path>          the class path of other builds the program imports, as the souther\n"
                + "                      command takes it (also --class-path)\n"
                + "  -o <object>         where to write the object file\n"
                + "  --library <dir>     where to write the library: object, headers, manifest, shared library\n"
                + "  --with <object>     the object another build the program reaches was compiled to\n"
                + "  --binding <generator> <dir>\n"
                + "                      where to write the binding the generator writes; the generator is\n"
                + "                      one of this compiler's, by its id, or one of someone else's, as\n"
                + "                      groupId:artifactId:version@sha256:<hex> or a path to a jar\n"
                + "  --binding-option <key>=<value>\n"
                + "                      an option of the --binding before it, split at the first =\n"
                + "  <source>            a .sou file, or a directory holding some\n"
                + "\n"
                + "  built-in generators, with the options each takes; which it needs, and what a\n"
                + "  value may be, is the generator's to say:\n"
                + catalog;
    }

    /** What a command asks to be written, each of which is a whole command. */
    sealed interface Output {

        /** What is built from the sources: the only outputs that read them, or a driver. */
        sealed interface Build extends Output {
        }

        /** One object file. */
        record ObjectFile(Path into) implements Build {
        }

        /** What this needs and does not have, fetched: a command of its own, and writes no build. */
        record Fetch() implements Output {
        }

        /**
         * The usage, asked for with {@code --help} or {@code -h}: written and nothing else, since a
         * command line that asks how it is written is one that did what it asked.
         */
        record Usage() implements Output {
        }

        /** A library, reaching {@code alongside}, and each binding of it asked for. */
        record Library(Path into, List<Path> alongside, List<HostBinding> bindings)
                implements Build {

            public Library {
                alongside = List.copyOf(alongside);
                bindings = List.copyOf(bindings);
            }
        }
    }

    /**
     * The generator a binding was asked for with: an id of the catalog, or a jar. What the command
     * reads and nothing more; which jar that is, is {@link GeneratorSpec#of}.
     */
    sealed interface Asked {

        /** A binding of the catalog, by its id. */
        record Standard(KnownBindings.Kind kind) implements Asked {
        }

        /** A generator of someone else's, by its coordinate and digest or its path. */
        record External(GeneratorRef ref) implements Asked {
        }

        /**
         * What {@code --binding} names: an id of the catalog, or else what {@link GeneratorRef#parse}
         * reads. An id holds neither a {@code :} nor ends in {@code .jar}, so none is taken for a
         * jar's coordinate or path.
         */
        static Asked of(String generator) {
            Optional<KnownBindings.Kind> kind = KnownBindings.named(generator);
            if (kind.isPresent()) {
                return new Standard(kind.get());
            }
            return new External(GeneratorRef.parse(generator));
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
            return "--binding " + switch (asked) {
                case Asked.Standard standard -> standard.kind().id();
                case Asked.External external -> external.ref().toString();
            };
        }
    }

    /**
     * A command as read: what it writes, from which sources, reading the other builds they import
     * from {@code classPath}, and whether it may fetch what it needs and does not have.
     */
    record Command(Output output, List<Path> sources, List<Path> classPath, boolean offline) {

        CheckedProgram checked(CompilationSources read) {
            return CheckedProgram.of(read, classPath.isEmpty() ? ModulePath.EMPTY
                    : ModulePath.ofClassPath(classPath));
        }
    }

    /** A command line that is not a command, and why. */
    static final class NotACommand extends Exception {
        NotACommand(String why) {
            super(Objects.requireNonNull(why, "a command refused says why"));
        }
    }

    private Main() {
    }

    public static void main(String[] args) {
        System.exit(run(args, System.out, System.err));
    }

    static int run(String[] args, PrintStream out, PrintStream problems) {
        return run(args, out, problems, Fetching::standard);
    }

    static int run(String[] args, PrintStream out, PrintStream problems, Fetching allowed) {
        return run(args, out, problems, () -> allowed);
    }

    /** Where what a command may fetch is read from, asked only once there is a command to run. */
    private interface Allowed {
        Fetching fetching() throws NotFetched;
    }

    private static int run(String[] args, PrintStream out, PrintStream problems, Allowed allowed) {
        Command command;
        try {
            command = read(args);
        } catch (NotACommand e) {
            problems.println(e.getMessage());
            problems.print(USAGE);
            return WRONG_COMMAND;
        }
        try {
            return switch (command.output()) {
                // Before anything is read of where this fetches from: how the command is written is
                // the same whatever is installed beside it.
                case Output.Usage usage -> {
                    out.print(USAGE);
                    yield WROTE_IT;
                }
                case Output.Fetch fetch -> fetched(out, problems,
                        fetching(allowed, command, problems));
                case Output.Build build -> built(command, build,
                        fetching(allowed, command, problems), out, problems);
            };
        } catch (NotFetched e) {
            problems.println(e.getMessage());
            return WRONG_COMMAND;
        }
    }

    private static Fetching fetching(Allowed allowed, Command command, PrintStream problems)
            throws NotFetched {
        return allowed.fetching().withOffline(command.offline()).saying(problems);
    }

    /** What {@code build} asks for, built from the sources the command named. */
    private static int built(Command command, Output.Build build, Fetching fetching,
                             PrintStream out, PrintStream problems) {
        CompilationSources read;
        try {
            read = readSources(command.sources());
        } catch (IOException e) {
            problems.println(e.getMessage());
            return WRONG_COMMAND;
        }
        if (read.texts().isEmpty()) {
            problems.println("nothing to compile: no .sou file among what was named");
            return WRONG_COMMAND;
        }

        try {
            switch (build) {
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
                    out.println("wrote " + object.into() + " from " + sources(read));
                }
                case Output.Library library -> {
                    return written(command, library, read, fetching, out, problems);
                }
            }
        } catch (GeneratorFailed e) {
            problems.println(e.getMessage());
            return REFUSED;
        } catch (CompileException e) {
            Refusal.lines(e, read).forEach(problems::println);
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
    private static int written(Command command, Output.Library library, CompilationSources read,
                               Fetching fetching, PrintStream out, PrintStream problems)
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
            out.println("wrote the library " + library.into() + " from " + sources(read));
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
     * The driver a program is handed to. A release runs the bundle of its own release for this
     * platform, verified and unpacked for this command, which {@code held} owns, and reads no property
     * naming another: what it runs is what it holds a checksum for, as for its generators. A build that
     * is not a release has no bundle, and runs the driver its clone built, named by
     * {@link NativeCompiler#DRIVER_PROPERTY}, which is not this command's to delete.
     */
    private static Path driver(Holding held, Fetching fetching) throws IOException {
        if (fetching.isRelease()) {
            return NativeBundle.unpacked(held, fetching);
        }
        return NativeCompiler.namedDriver();
    }

    /**
     * {@code --fetch}: what a command may need and this does not have, fetched and kept, and nothing
     * built. It prepares the cache and nothing that runs: no copy of a jar and no driver unpacked.
     */
    private static int fetched(PrintStream out, PrintStream problems, Fetching fetching) {
        try {
            if (fetching.isRelease()) {
                out.println("the driver " + NativeBundle.kept(fetching));
            } else {
                out.println("the driver is this build's own, " + NativeCompiler.namedDriver());
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
        List<Named> named = new ArrayList<>();
        List<Path> sources = new ArrayList<>();
        List<Path> classPath = new ArrayList<>();
        boolean offline = false;
        boolean fetch = false;
        for (int at = 0; at < args.length; at++) {
            String held = args[at];
            switch (held) {
                case "--offline" -> offline = true;
                case "--fetch" -> fetch = true;
                case "--help", "-h" -> {
                    return new Command(new Output.Usage(), List.of(), List.of(), false);
                }
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
                    String generator = valueOf(args, ++at, held);
                    Asked asking;
                    try {
                        asking = Asked.of(generator);
                    } catch (IllegalArgumentException e) {
                        throw new NotACommand(held + ": " + e.getMessage());
                    }
                    if (asking instanceof Asked.Standard standard && named.stream()
                            .anyMatch(before -> before.asked().equals(standard))) {
                        throw new NotACommand(held + " " + generator + " is named twice");
                    }
                    named.add(new Named(asking, Path.of(valueOf(args, ++at, held + " " + generator)),
                            new LinkedHashMap<>()));
                }
                case "--binding-option" -> {
                    String option = valueOf(args, ++at, held);
                    if (named.isEmpty()) {
                        throw new NotACommand(held + " qualifies the --binding before it, and"
                                + " there is none");
                    }
                    int equals = option.indexOf('=');
                    if (equals <= 0) {
                        throw new NotACommand(held + " is <key>=<value>, and \"" + option
                                + "\" is not");
                    }
                    String key = option.substring(0, equals);
                    Named binding = named.getLast();
                    if (binding.asked() instanceof Asked.Standard standard
                            && !standard.kind().options().contains(key)) {
                        throw new NotACommand("--binding " + standard.kind().id()
                                + " takes no option \"" + key + "\"; it takes "
                                + String.join(", ", standard.kind().options()));
                    }
                    if (binding.options().put(key, option.substring(equals + 1)) != null) {
                        throw new NotACommand(held + " names " + key + " twice for one binding");
                    }
                }
                default -> {
                    if (held.startsWith("-")) {
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
                    || !classPath.isEmpty() || !named.isEmpty()) {
                throw new NotACommand("--fetch is a command of its own, and takes nothing else");
            }
            return new Command(new Output.Fetch(), List.of(), List.of(), false);
        }
        if (sources.isEmpty()) {
            throw new NotACommand("name the sources to compile");
        }
        if (object != null && library != null) {
            throw new NotACommand("-o and --library are two commands; name one");
        }
        if (library == null) {
            if (!alongside.isEmpty()) {
                throw new NotACommand("--with goes with --library");
            }
            if (!named.isEmpty()) {
                throw new NotACommand("--binding goes with --library");
            }
            if (object == null) {
                throw new NotACommand("-o <object> or --library <dir> says what to write; name one");
            }
            return new Command(new Output.ObjectFile(object), List.copyOf(sources),
                    List.copyOf(classPath), offline);
        }
        List<String> directories = new ArrayList<>(List.of("--library"));
        List<Path> placed = new ArrayList<>(List.of(library));
        List<HostBinding> bindings = new ArrayList<>();
        for (Named binding : named) {
            bindings.add(new HostBinding(binding.asked(), binding.into(), binding.options()));
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
    private record Named(Asked asked, Path into, Map<String, String> options) {
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

    private static String sources(CompilationSources read) {
        int count = read.texts().size();
        return count + (count == 1 ? " source" : " sources");
    }

    /**
     * The sources a command line named, read: a file as itself, and a directory as every
     * {@code .sou} under it, read the way Souther reads a tree of sources.
     *
     * <p>Held as one value from here on, because what the program is checked from, what a lone file
     * with no header is called, and what a refusal is quoted from are one list of texts in one
     * order, and the compile is the one that says which id each gets.
     */
    private static CompilationSources readSources(List<Path> named) throws IOException {
        List<SourceFile> files = new ArrayList<>();
        for (Path path : named) {
            if (Files.isDirectory(path)) {
                files.addAll(CompilationSources.readTree(path));
            } else {
                files.add(new SourceFile(path.toString(),
                        Files.readString(path, StandardCharsets.UTF_8)));
            }
        }
        return CompilationSources.files(files);
    }
}
