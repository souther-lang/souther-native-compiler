package souther.nativecode;

import souther.compiler.diag.CompileException;
import souther.compiler.program.CheckedProgram;

import java.io.IOException;
import java.io.PrintStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

/** Compiles the sources named on the command line into one object file. */
public final class Main {

    private static final int WROTE_IT = 0;
    private static final int REFUSED = 1;
    private static final int WRONG_COMMAND = 2;

    private static final String USAGE = """
            usage: souther-native -o <object> <source>...

              -o <object>   where to write the object file
              <source>      a .sou file, or a directory holding some
            """;

    private Main() {
    }

    public static void main(String[] args) {
        System.exit(run(args, System.out, System.err));
    }

    static int run(String[] args, PrintStream out, PrintStream problems) {
        Path into = null;
        List<Path> sources = new ArrayList<>();
        for (int at = 0; at < args.length; at++) {
            String held = args[at];
            if ("-o".equals(held)) {
                if (++at == args.length) {
                    problems.println("-o wants a path");
                    return WRONG_COMMAND;
                }
                into = Path.of(args[at]);
            } else if (held.startsWith("-")) {
                problems.println("no such option: " + held);
                problems.print(USAGE);
                return WRONG_COMMAND;
            } else {
                sources.add(Path.of(held));
            }
        }
        if (into == null || sources.isEmpty()) {
            problems.print(USAGE);
            return WRONG_COMMAND;
        }

        List<Path> files;
        try {
            files = under(sources);
        } catch (IOException | UncheckedIOException e) {
            problems.println(e.getMessage());
            return WRONG_COMMAND;
        }
        if (files.isEmpty()) {
            problems.println("nothing to compile: no .sou file among what was named");
            return WRONG_COMMAND;
        }

        byte[] written;
        try {
            List<String> read = new ArrayList<>(files.size());
            for (Path file : files) {
                read.add(Files.readString(file, StandardCharsets.UTF_8));
            }
            written = NativeCompiler.compile(CheckedProgram.of(read));
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

        try {
            if (into.getParent() != null) {
                Files.createDirectories(into.getParent());
            }
            Files.write(into, written);
        } catch (IOException e) {
            problems.println(e.getMessage());
            return WRONG_COMMAND;
        }
        out.println("wrote " + into + " from " + files.size()
                + (files.size() == 1 ? " source" : " sources"));
        return WROTE_IT;
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
