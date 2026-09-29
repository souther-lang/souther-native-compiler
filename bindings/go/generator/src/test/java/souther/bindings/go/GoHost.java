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
        NativeCompiler.Library library = NativeCompiler.library(
                souther.nativecode.Checked.of(List.of(model)), into.resolve("native"));
        Generated binding = generated(library, into.resolve("binding"), importPath);
        return ran(into, binding, importPath, main, List.of(library.library().toString()));
    }

    /**
     * What a host whose {@code main.go} is {@code main} printed, built beside {@code binding} in
     * {@code into} and run with {@code arguments}, where the binding and the host pass vet and the
     * host built and ran with nothing said.
     */
    static String ran(Path into, Generated binding, String importPath, String main,
                      List<String> arguments) throws IOException, InterruptedException {
        workspace(into, binding, importPath, main);
        formatted(binding);
        go(into, List.of("vet", importPath + "/...", "host"));
        List<String> run = new ArrayList<>(List.of("run", "host"));
        run.addAll(arguments);
        return go(into, run);
    }

    /**
     * Refuses a generated file that gofmt would write differently: what the generator writes is
     * what a Go programmer would have, and a host that formats its dependencies is not left with a
     * diff.
     */
    private static void formatted(Generated binding) throws IOException, InterruptedException {
        ProcessBuilder builder = new ProcessBuilder("gofmt", "-l", ".")
                .directory(binding.root().toFile());
        Process process = builder.start();
        String listed = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        process.waitFor();
        if (!listed.isEmpty()) {
            throw new AssertionError("gofmt would change what the generator wrote:\n" + listed);
        }
    }

    /** Writes the workspace of the generated module, the runtime and a host whose {@code main.go} is {@code main}. */
    private static void workspace(Path into, Generated binding, String importPath, String main)
            throws IOException {
        Path host = into.resolve("host");
        Files.createDirectories(host);
        Files.writeString(into.resolve("go.work"), """
                go 1.27

                use (
                	%s
                	%s
                	%s
                )

                replace %s %s => %s

                replace %s v0.0.0 => %s
                """.formatted(quoted(into.relativize(binding.root())), quoted(RUNTIME), "./host",
                GoBindings.RUNTIME_MODULE_PATH, GoBindings.RUNTIME_VERSION, quoted(RUNTIME),
                importPath, "./" + quoted(into.relativize(binding.root()))),
                StandardCharsets.UTF_8);
        Files.writeString(host.resolve("go.mod"), """
                module host

                go 1.27

                require %s v0.0.0
                require github.com/raoh-project/raoh-go %s
                """.formatted(importPath, raohVersion()), StandardCharsets.UTF_8);
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

    private static String quoted(Path path) {
        return path.toString();
    }

    private static String quoted(String path) {
        return path;
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
