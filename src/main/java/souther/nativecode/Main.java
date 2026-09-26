package souther.nativecode;

import souther.bindings.NotBindable;
import souther.bindings.php.PhpBindings;
import souther.bindings.rust.RustBindings;
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
import java.util.List;
import java.util.stream.Stream;

/**
 * Compiles the sources named on the command line into one object file, or builds them into a
 * library for a host and, if asked, the PHP and the Rust binding of it.
 *
 * <p>Nothing here decides what a library or a binding is. The program is checked once, the driver
 * writes the library from it ({@link NativeCompiler#library}), and each binding is generated from
 * the manifest the driver wrote ({@link PhpBindings#generate}, {@link RustBindings#generate}), never
 * from the program a second time.
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

    private static final String USAGE = """
            usage: souther-native [-cp <path>] -o <object> <source>...
                   souther-native [-cp <path>] --library <dir> [--with <object>]...
                                  [--php <dir> --namespace <ns>] [--rust <dir> --crate <name>]
                                  <source>...

              -cp <path>         the class path of other builds the program imports, as the souther
                                 command takes it (also --class-path)
              -o <object>        where to write the object file
              --library <dir>    where to write the library: object, headers, manifest, shared library
              --with <object>    the object another build the program reaches was compiled to
              --php <dir>        where to write the PHP binding of the library
              --namespace <ns>   the PHP namespace the binding is written under
              --rust <dir>       where to write the Rust binding of the library, as a crate
              --crate <name>     the name of the crate the Rust binding is written as
              <source>           a .sou file, or a directory holding some
            """;

    /** What a command asks to be written, each of which is a whole command. */
    sealed interface Output {

        /** One object file. */
        record ObjectFile(Path into) implements Output {
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
     * A binding of the library for one host's language, and where it is written. Each is generated
     * by its own generator from the manifest; nothing here is shared between them but where each
     * goes.
     */
    sealed interface HostBinding {

        /** The directory the binding is written into, replaced whole. */
        Path into();

        /** A PHP binding, under a namespace. */
        record Php(Path into, String namespace) implements HostBinding {
        }

        /** A Rust binding, as a crate of a name. */
        record Rust(Path into, String crate) implements HostBinding {
        }
    }

    /**
     * A command as read: what it writes, from which sources, reading the other builds they import
     * from {@code classPath}.
     */
    record Command(Output output, List<Path> sources, List<Path> classPath) {

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
                case Output.ObjectFile object -> {
                    byte[] written = NativeCompiler.compile(command.checked(read));
                    if (object.into().getParent() != null) {
                        Files.createDirectories(object.into().getParent());
                    }
                    Files.write(object.into(), written);
                    out.println("wrote " + object.into() + " from " + sources(files));
                }
                case Output.Library library -> {
                    for (HostBinding binding : library.bindings()) {
                        try {
                            switch (binding) {
                                case HostBinding.Php php ->
                                        PhpBindings.refuseAhead(php.into(), php.namespace());
                                case HostBinding.Rust rust ->
                                        RustBindings.refuseAhead(rust.into(), rust.crate());
                            }
                        } catch (NotBindable e) {
                            problems.println(e.getMessage());
                            return WRONG_COMMAND;
                        }
                    }
                    List<byte[]> alongside = new ArrayList<>(library.alongside().size());
                    for (Path object : library.alongside()) {
                        alongside.add(Files.readAllBytes(object));
                    }
                    NativeCompiler.Library built = NativeCompiler.library(
                            command.checked(read), alongside, library.into());
                    out.println("wrote the library " + library.into() + " from " + sources(files));
                    for (HostBinding binding : library.bindings()) {
                        switch (binding) {
                            case HostBinding.Php php -> {
                                try {
                                    PhpBindings.generate(built.manifest(), built.declarations(),
                                            php.into(), php.namespace());
                                } catch (NotBindable e) {
                                    problems.println("the PHP binding is not written: "
                                            + e.getMessage());
                                    return REFUSED;
                                }
                                out.println("wrote the PHP binding " + php.into() + " under "
                                        + php.namespace());
                            }
                            case HostBinding.Rust rust -> {
                                try {
                                    RustBindings.generate(built.manifest(), rust.into(),
                                            rust.crate());
                                } catch (NotBindable e) {
                                    problems.println("the Rust binding is not written: "
                                            + e.getMessage());
                                    return REFUSED;
                                }
                                out.println("wrote the Rust binding " + rust.into() + " as the"
                                        + " crate " + rust.crate());
                            }
                        }
                    }
                }
            }
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
     * The command {@code args} say, or why they say none: an option that belongs to one output
     * named with the other, or with what it goes with missing, is refused here rather than read as
     * something the command did not mean.
     */
    static Command read(String[] args) throws NotACommand {
        Path object = null;
        Path library = null;
        List<Path> alongside = new ArrayList<>();
        Path php = null;
        String namespace = null;
        Path rust = null;
        String crate = null;
        List<Path> sources = new ArrayList<>();
        List<Path> classPath = new ArrayList<>();
        for (int at = 0; at < args.length; at++) {
            String held = args[at];
            switch (held) {
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
                case "--php" -> php = once(held, php, Path.of(valueOf(args, ++at, held)));
                case "--namespace" -> namespace = once(held, namespace, valueOf(args, ++at, held));
                case "--rust" -> rust = once(held, rust, Path.of(valueOf(args, ++at, held)));
                case "--crate" -> crate = once(held, crate, valueOf(args, ++at, held));
                default -> {
                    if (held.startsWith("-")) {
                        throw new NotACommand("no such option: " + held);
                    }
                    sources.add(Path.of(held));
                }
            }
        }
        if (sources.isEmpty()) {
            throw new NotACommand(null);
        }
        if (object != null && library != null) {
            throw new NotACommand("-o and --library are two commands; name one");
        }
        if (library == null) {
            for (var named : List.of(new Named("--with", !alongside.isEmpty()),
                    new Named("--php", php != null), new Named("--namespace", namespace != null),
                    new Named("--rust", rust != null), new Named("--crate", crate != null))) {
                if (named.given()) {
                    throw new NotACommand(named.option() + " goes with --library");
                }
            }
            if (object == null) {
                throw new NotACommand(null);
            }
            return new Command(new Output.ObjectFile(object), List.copyOf(sources),
                    List.copyOf(classPath));
        }
        if (php != null && namespace == null) {
            throw new NotACommand("--php wants --namespace, the namespace the binding is under");
        }
        if (namespace != null && php == null) {
            throw new NotACommand("--namespace goes with --php");
        }
        if (rust != null && crate == null) {
            throw new NotACommand("--rust wants --crate, the name of the crate the binding is");
        }
        if (crate != null && rust == null) {
            throw new NotACommand("--crate goes with --rust");
        }
        List<Named> directories = new ArrayList<>(List.of(new Named("--library", true)));
        List<Path> placed = new ArrayList<>(List.of(library));
        List<HostBinding> bindings = new ArrayList<>();
        if (php != null) {
            bindings.add(new HostBinding.Php(php, namespace));
            directories.add(new Named("--php", true));
            placed.add(php);
        }
        if (rust != null) {
            bindings.add(new HostBinding.Rust(rust, crate));
            directories.add(new Named("--rust", true));
            placed.add(rust);
        }
        for (int one = 0; one < placed.size(); one++) {
            for (int other = one + 1; other < placed.size(); other++) {
                if (within(placed.get(one), placed.get(other))
                        || within(placed.get(other), placed.get(one))) {
                    throw new NotACommand(directories.get(other).option() + " and "
                            + directories.get(one).option() + " are each replaced whole, so"
                            + " neither can hold the other");
                }
            }
        }
        return new Command(new Output.Library(library, alongside, bindings),
                List.copyOf(sources), List.copyOf(classPath));
    }

    private record Named(String option, boolean given) {
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
