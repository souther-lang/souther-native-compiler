package souther.bindings.go;

import souther.bindings.BindingInput;
import souther.bindings.Declarations;
import souther.bindings.Generated;
import souther.bindings.Manifest;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Repository;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * A Go host of a library a test built, as an application would be one: a module depending on the
 * binding generated from the library's manifest, with the runtime module this repository holds in
 * place of the one a registry would have, built with the Go toolchain and run.
 *
 * <p>What the host prints is what a test compares. What Go says goes apart from it, and a run
 * saying anything there fails: a vet finding or a C warning in what the binding wrote, or in the
 * host, is a failure, since a binding a host cannot build without them is one it builds with
 * {@code -Werror} at its peril.
 *
 * <p>The three are one workspace, which is where the runtime is put in place of the registry's, so
 * that vet reads what the generator wrote as a package of the workspace's own.
 */
final class GoHost {

    /** Where the runtime module stands. */
    static final Path RUNTIME = Repository.file("bindings", "go", "runtime").toAbsolutePath();

    private GoHost() {
    }

    /** The binding of {@code library} generated into {@code into} as the package {@code importPath}. */
    static Generated generated(NativeCompiler.Library library, Path into, String importPath)
            throws IOException {
        return GoBindings.generate(new BindingInput(Manifest.read(library.manifest()),
                Declarations.at(library.declarations())), into, importPath);
    }

    /**
     * What a host whose {@code main.go} is {@code main} printed, over the library {@code model}
     * builds, whose binding is generated as the package {@code importPath}.
     */
    static String ran(Path into, String model, String importPath, String main)
            throws IOException, InterruptedException {
        return ran(into, List.of(model), importPath, main);
    }

    /** As above, for a library of several modules, each written as a source. */
    static String ran(Path into, List<String> modules, String importPath, String main)
            throws IOException, InterruptedException {
        NativeCompiler.Library library = NativeCompiler.library(
                souther.nativecode.Checked.of(modules), into.resolve("native"));
        Generated binding = generated(library, into.resolve("binding"), importPath);
        return ran(into, binding, importPath, main, List.of(library.library().toString()));
    }

    /** A binding generated as the package {@code importPath}, which a host depends on. */
    record Binding(Generated generated, String importPath) {
    }

    /**
     * What a host whose {@code main.go} is {@code main} printed, built beside {@code binding} in
     * {@code into} and run with {@code arguments}, where the binding and the host pass vet and the
     * host built and ran with nothing said.
     */
    static String ran(Path into, Generated binding, String importPath, String main,
                      List<String> arguments) throws IOException, InterruptedException {
        return ran(into, List.of(new Binding(binding, importPath)), main, arguments);
    }

    /**
     * What a host printed that depends on each of {@code bindings}: several in one program, which is
     * what a binary that uses two libraries is.
     */
    static String ran(Path into, List<Binding> bindings, String main, List<String> arguments)
            throws IOException, InterruptedException {
        workspace(into, bindings, main);
        List<String> vet = new ArrayList<>(List.of("vet"));
        for (Binding binding : bindings) {
            formatted(binding.generated());
            askedTheRunFirst(binding.generated());
            declaredSumTypes(binding.generated());
            vet.add(binding.importPath() + "/...");
        }
        vet.add("host");
        go(into, vet);
        List<String> run = new ArrayList<>(List.of("run", "host"));
        run.addAll(arguments);
        return go(into, run);
    }

    /** A function of a generated package that takes a run, up to the end of it. */
    private static final Pattern RUN_FUNCTION = Pattern.compile(
            "(?ms)^func (?:\\([^)]*\\) )?\\w+\\(r \\*lib\\.Run[^\\n]*\\{\\n(.*?)\\n\\}\\n");

    /**
     * Refuses a generated function that takes a run and makes a computation in it without asking the
     * run first. Every such function begins {@code souther.Making(r)}: the check is what a function
     * that takes a run does, in the one place, and no function has it only because of what else it
     * writes. So a function that calls the library ({@code souther.Called}) or a function of the
     * host's ({@code Host__.Fn}) is held to it whatever it is made of.
     */
    private static void askedTheRunFirst(Generated binding) throws IOException {
        for (Path file : binding.files()) {
            if (!file.toString().endsWith(".go")) {
                continue;
            }
            Matcher function = RUN_FUNCTION.matcher(Files.readString(file, StandardCharsets.UTF_8));
            while (function.find()) {
                String body = function.group(1);
                boolean computes = body.contains("souther.Called(") || body.contains("Host__.Fn(");
                if (computes && !body.startsWith("\tsouther.Making(r)\n")) {
                    throw new AssertionError(file + " has a function that computes before it asks its run:\n"
                            + function.group());
                }
            }
        }
    }

    /** A sealed interface: one unexported method and nothing else, which only this package can meet. */
    private static final Pattern SEALED = Pattern.compile(
            "(?m)^(?<doc>(?://[^\\n]*\\n)*)type (?<name>\\w+) interface \\{\\n\\t[a-z]\\w*\\(\\)\\n\\}\\n");

    /**
     * Refuses a sealed interface that is not declared a sum type to go-check-sumtype: every such
     * interface is a union or a sum's cases, whose members the model closes, and a type switch over
     * one that leaves a case out is found only where the check knows it is a sum.
     */
    private static void declaredSumTypes(Generated binding) throws IOException {
        for (Path file : binding.files()) {
            if (!file.toString().endsWith(".go")) {
                continue;
            }
            Matcher sealed = SEALED.matcher(Files.readString(file, StandardCharsets.UTF_8));
            while (sealed.find()) {
                if (!sealed.group("doc").endsWith("//sumtype:decl\n")) {
                    throw new AssertionError(file + " has a sealed interface " + sealed.group("name")
                            + " that is not declared a sum type:\n" + sealed.group());
                }
            }
        }
    }

    /**
     * Refuses a generated file that gofmt would write differently: what the generator writes is
     * what a Go programmer would have, and a host that formats its dependencies is not left with a
     * diff.
     */
    static void formatted(Generated binding) throws IOException, InterruptedException {
        ProcessBuilder builder = new ProcessBuilder("gofmt", "-d", ".")
                .directory(binding.root().toFile());
        Process process = builder.start();
        String difference =
                new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        process.waitFor();
        if (!difference.isEmpty()) {
            throw new AssertionError("gofmt would change what the generator wrote:\n" + difference);
        }
    }

    /** Writes the workspace of the generated modules, the runtime and a host whose {@code main.go} is {@code main}. */
    private static void workspace(Path into, List<Binding> bindings, String main)
            throws IOException {
        Path host = into.resolve("host");
        Files.createDirectories(host);
        StringBuilder work = new StringBuilder("go " + RuntimeModule.THE.go() + "\n\nuse (\n");
        StringBuilder require = new StringBuilder();
        StringBuilder replace = new StringBuilder();
        for (Binding binding : bindings) {
            Path relative = into.relativize(binding.generated().root());
            work.append("\t").append(relative).append("\n");
            require.append("require ").append(binding.importPath()).append(" v0.0.0\n");
            replace.append("\nreplace ").append(binding.importPath()).append(" v0.0.0 => ./")
                    .append(relative).append("\n");
        }
        work.append("\t").append(RUNTIME).append("\n\t./host\n)\n\nreplace ")
                .append(RuntimeModule.THE.path()).append(" ").append(RuntimeModule.THE.requirement())
                .append(" => ").append(RUNTIME).append("\n").append(replace);
        Files.writeString(into.resolve("go.work"), work.toString(), StandardCharsets.UTF_8);
        Files.writeString(host.resolve("go.mod"), "module host\n\ngo " + RuntimeModule.THE.go() + "\n\n" + require
                + "require github.com/raoh-project/raoh-go " + raohVersion() + "\n",
                StandardCharsets.UTF_8);
        Files.writeString(host.resolve("main.go"), main, StandardCharsets.UTF_8);
    }

    /** The version of Raoh the runtime module asks for, which a host that imports it asks for too. */
    private static String raohVersion() throws IOException {
        Matcher it = Pattern.compile("github.com/raoh-project/raoh-go (\\S+)")
                .matcher(Files.readString(RUNTIME.resolve("go.mod"), StandardCharsets.UTF_8));
        if (!it.find()) {
            throw new AssertionError("the runtime module asks for no Raoh");
        }
        return it.group(1);
    }

    /** What Go printed, run with {@code arguments} in {@code into}, where it ended well and said nothing else. */
    private static String go(Path into, List<String> arguments)
            throws IOException, InterruptedException {
        List<String> command = new ArrayList<>(List.of("go"));
        command.addAll(arguments);
        // What it says goes to a file rather than to a pipe read second: two pipes read one after
        // the other deadlock where the one not being read fills up first.
        Path said = Files.createTempFile("go-", ".said");
        try {
            ProcessBuilder builder = new ProcessBuilder(command).directory(into.toFile())
                    .redirectError(said.toFile());
            // A warning of the C compiler is a failure, whatever a developer's own settings add.
            builder.environment().put("CGO_CFLAGS", "-Wall -Wextra -Werror");
            builder.environment().remove("GOFLAGS");
            // The Go that is installed, and never another that Go would fetch on its own.
            builder.environment().put("GOTOOLCHAIN", "local");
            Process process = builder.start();
            String printed =
                    new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            int status = process.waitFor();
            String saidThere = Files.readString(said, StandardCharsets.UTF_8);
            if (status != 0 || !saidThere.isEmpty()) {
                throw new AssertionError(command + " ended with " + status + "\nprinted:\n"
                        + printed + "\nsaid:\n" + saidThere);
            }
            return printed;
        } finally {
            Files.deleteIfExists(said);
        }
    }
}
