package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * The command line builds what a host is handed, a library and the bindings of it, as the API does
 * and through it: an application needs no Java of its own to build what it runs.
 *
 * <p>A command is one output, read before anything is built: an option of the other output, or
 * one qualifying a binding that was not asked for, is a command refused and not a build half done.
 * A binding of the catalog is asked for by its flag, and one of someone else's by {@code --binding};
 * what the standard generators write is held to the launcher's tests, which run their jars.
 */
class TheCommandLineBuildsALibraryAndItsBindingTest {

    private static final String MONEY = """
            module shop.money exposing ( Money )

            data Money = Int
                invariant notNegative = value >= 0
            """;

    private static final String LINES = """
            module shop.lines exposing ( Line, total )
            import shop.money ( Money )

            data Line = { price: Money, quantity: Int }

            behavior total : (line: Line) -> Int
            let total (line) = line.price.value * line.quantity
            """;

    private record Ran(int ended, String printed, String said) {
    }

    @Test
    void aLibraryIsWrittenWithoutABindingWhereNoneIsAskedFor(@TempDir Path into) throws Exception {
        Path model = sources(into, Map.of("money.sou", MONEY, "lines.sou", LINES));

        Ran ran = run("--library", into.resolve("native").toString(), model.toString());

        assertThat(ran.ended()).as(ran.said()).isZero();
        assertThat(into.resolve("native").resolve("souther.h")).exists();
        assertThat(into.resolve("php")).doesNotExist();
    }

    @Test
    void aCommandIsOneOutput() {
        refused("options for two outputs", "-o", "a.o", "--library", "out", "m.sou");
        refused("--with without a library", "-o", "a.o", "--with", "b.o", "m.sou");
        refused("--php without a library", "-o", "a.o", "--php", "php", "--namespace", "N",
                "m.sou");
        refused("--namespace without --php", "--library", "out", "--namespace", "N", "m.sou");
        refused("the binding inside the library", "--library", "out", "--php", "out/php",
                "--namespace", "N", "m.sou");
        refused("the library inside the binding", "--library", "php/out", "--php", "php",
                "--namespace", "N", "m.sou");
        refused("--rust without a library", "-o", "a.o", "--rust", "rust", "--crate", "c",
                "m.sou");
        refused("--crate without --rust", "--library", "out", "--crate", "c", "m.sou");
        refused("the Rust binding inside the library", "--library", "out", "--rust", "out/rust",
                "--crate", "c", "m.sou");
        refused("the PHP binding inside the Rust one", "--library", "out", "--rust", "b",
                "--crate", "c", "--php", "b/php", "--namespace", "N", "m.sou");
        refused("one output named twice", "--library", "a", "--library", "b", "m.sou");
        refused("an option wanting a value", "m.sou", "--library");
        refused("no output", "m.sou");
        refused("no source", "--library", "out");
    }

    @Test
    void aCommandIsReadAsTheOutputItNames() throws Exception {
        Main.Command command = Main.read(new String[]{"-cp", "a" + java.io.File.pathSeparator + "b",
                "--library", "out", "--with", "x.o", "--with", "y.o", "--php", "php",
                "--namespace", "N", "m.sou"});

        assertThat(command.classPath()).containsExactly(Path.of("a"), Path.of("b"));
        assertThat(command.output()).isEqualTo(new Main.Output.Library(Path.of("out"),
                List.of(Path.of("x.o"), Path.of("y.o")),
                List.of(binding("php", "php", "namespace", "N"))));
    }

    @Test
    void aCommandAsksForEachBindingItNames() throws Exception {
        Main.Command command = Main.read(new String[]{"--library", "out", "--rust", "rust",
                "--crate", "acme", "--php", "php", "--namespace", "N", "m.sou"});

        assertThat(command.output()).isEqualTo(new Main.Output.Library(Path.of("out"), List.of(),
                List.of(binding("php", "php", "namespace", "N"),
                        binding("rust", "rust", "crate", "acme"))));
    }

    /**
     * A generator named with {@code --binding} is read by its coordinate and digest, or as a path; an
     * option of it is split at its first {@code =}, and belongs to the {@code --binding} before it.
     */
    @Test
    void aBindingIsReadWithTheOptionsThatFollowIt() throws Exception {
        String digest = "ab".repeat(32);
        Main.Command command = Main.read(new String[]{"--library", "out",
                "--binding", "com.acme:souther-binding-kotlin:1.2.0@sha256:" + digest, "kotlin",
                "--binding-option", "package=com.acme.shop", "--binding-option", "style=a=b",
                "--binding", "build/generator.jar", "local",
                "--binding", "/tmp/build@experimental/g:1.jar", "at", "m.sou"});

        assertThat(command.output()).isEqualTo(new Main.Output.Library(Path.of("out"), List.of(),
                List.of(new Main.HostBinding(new Main.Asked.External(new GeneratorRef.Maven(
                                MavenCoordinate.parse("com.acme:souther-binding-kotlin:1.2.0"), digest)),
                                Path.of("kotlin"), Map.of("package", "com.acme.shop", "style", "a=b")),
                        new Main.HostBinding(new Main.Asked.External(new GeneratorRef.Local(
                                Path.of("build/generator.jar"))), Path.of("local"), Map.of()),
                        new Main.HostBinding(new Main.Asked.External(new GeneratorRef.Local(
                                Path.of("/tmp/build@experimental/g:1.jar"))), Path.of("at"),
                                Map.of()))));
    }

    @Test
    void aBindingIsRefusedWhatItCannotMean() {
        String digest = "ab".repeat(32);
        refused("a coordinate with no digest", "--library", "out", "--binding",
                "com.acme:kotlin:1.2.0", "kotlin", "m.sou");
        refused("a digest that is not SHA-256", "--library", "out", "--binding",
                "com.acme:kotlin:1.2.0@md5:" + digest, "kotlin", "m.sou");
        refused("a digest that is not one", "--library", "out", "--binding",
                "com.acme:kotlin:1.2.0@sha256:abc", "kotlin", "m.sou");
        refused("neither a jar nor a coordinate", "--library", "out", "--binding",
                "build/generator", "kotlin", "m.sou");
        refused("a coordinate of two parts", "--library", "out", "--binding",
                "com.acme:kotlin@sha256:" + digest, "kotlin", "m.sou");
        refused("a --binding with no directory", "--library", "out", "m.sou", "--binding",
                "g.jar");
        refused("an option with no --binding", "--library", "out", "--binding-option", "a=b",
                "m.sou");
        refused("an option after a flag of the catalog", "--library", "out", "--binding", "g.jar",
                "b", "--php", "php", "--namespace", "N", "--binding-option", "a=b", "m.sou");
        refused("an option with no key", "--library", "out", "--binding", "g.jar", "b",
                "--binding-option", "=b", "m.sou");
        refused("an option with no =", "--library", "out", "--binding", "g.jar", "b",
                "--binding-option", "ab", "m.sou");
        refused("a key named twice", "--library", "out", "--binding", "g.jar", "b",
                "--binding-option", "a=1", "--binding-option", "a=2", "m.sou");
        refused("a --binding without a library", "-o", "a.o", "--binding", "g.jar", "b", "m.sou");
        refused("a --binding inside the library", "--library", "out", "--binding", "g.jar",
                "out/b", "m.sou");
        refused("two bindings in one directory", "--library", "out", "--binding", "g.jar", "b",
                "--binding", "h.jar", "b", "m.sou");
        refused("--fetch with a --binding", "--fetch", "--binding", "g.jar", "b");
    }

    private static Main.HostBinding binding(String id, String into, String option, String value) {
        return new Main.HostBinding(new Main.Asked.Standard(KnownBindings.all().stream()
                .filter(kind -> kind.id().equals(id)).findFirst().orElseThrow()), Path.of(into),
                Map.of(option, value));
    }

    private static void refused(String what, String... args) {
        assertThatThrownBy(() -> Main.read(args)).as(what).isInstanceOf(Main.NotACommand.class);
        assertThat(run(args).ended()).as(what).isEqualTo(2);
    }

    private static Path sources(Path into, Map<String, String> files) throws Exception {
        Path model = into.resolve("model");
        Files.createDirectories(model);
        for (Map.Entry<String, String> each : files.entrySet()) {
            Files.writeString(model.resolve(each.getKey()), each.getValue(), StandardCharsets.UTF_8);
        }
        return model;
    }

    private static Ran run(String... args) {
        ByteArrayOutputStream printed = new ByteArrayOutputStream();
        ByteArrayOutputStream said = new ByteArrayOutputStream();
        int ended = Main.run(args, new PrintStream(printed, true, StandardCharsets.UTF_8),
                new PrintStream(said, true, StandardCharsets.UTF_8));
        return new Ran(ended, printed.toString(StandardCharsets.UTF_8),
                said.toString(StandardCharsets.UTF_8));
    }
}
