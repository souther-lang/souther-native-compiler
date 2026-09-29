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
 * What each binding is asked for is the catalog's, and what the generators write is held to the
 * launcher's tests, where they are installed.
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

    private static Main.HostBinding binding(String id, String into, String option, String value) {
        return new Main.HostBinding(KnownBindings.all().stream()
                .filter(kind -> kind.id().equals(id)).findFirst().orElseThrow(), Path.of(into),
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
