package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;

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
 * The command line builds what a host is handed, a library and the PHP binding of it, as the API
 * does and through it: an application needs no Java of its own to build what it runs.
 *
 * <p>A command is one output, read before anything is built: an option of the other output, or
 * one missing what it goes with, is a command refused and not a build half done. A binding that
 * would be refused whatever the manifest said is refused before the library is written.
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
    void aLibraryAndItsBindingAreWrittenFromTheSources(@TempDir Path into) throws Exception {
        Path model = sources(into, Map.of("money.sou", MONEY, "lines.sou", LINES));

        Ran ran = run("--library", into.resolve("native").toString(),
                "--php", into.resolve("php").toString(), "--namespace", "Acme\\Shop",
                model.toString());

        assertThat(ran.ended()).as(ran.said()).isZero();
        assertThat(into.resolve("native").resolve("souther.json")).exists();
        assertThat(into.resolve("php").resolve("Shop").resolve("Lines").resolve("Behaviors.php"))
                .exists();
        assertThat(ran.printed()).contains("from 2 sources").contains("under Acme\\Shop");
    }

    @Test
    void aLibraryIsWrittenWithoutABindingWhereNoneIsAskedFor(@TempDir Path into) throws Exception {
        Path model = sources(into, Map.of("money.sou", MONEY, "lines.sou", LINES));

        Ran ran = run("--library", into.resolve("native").toString(), model.toString());

        assertThat(ran.ended()).as(ran.said()).isZero();
        assertThat(into.resolve("native").resolve("souther.h")).exists();
        assertThat(into.resolve("php")).doesNotExist();
    }

    /**
     * Another build's module is read from its class path, as the souther command reads it, and its
     * object is linked into the library with {@code --with}: both, since the check reads the one
     * and the linker the other.
     */
    @Test
    void aLibraryReachesABuildMadeBefore(@TempDir Path into) throws Exception {
        Path before = sources(into.resolve("before"), Map.of("money.sou", MONEY));
        Path object = into.resolve("money.o");
        assertThat(run("-o", object.toString(), before.toString()).ended()).isZero();
        Path classes = into.resolve("classes");
        for (Map.Entry<String, ClassFileImage> each : Compiler.compile(MONEY).entrySet()) {
            Path file = classes.resolve(each.getKey().replace('.', '/') + ".class");
            Files.createDirectories(file.getParent());
            Files.write(file, each.getValue().bytes());
        }
        Path building = sources(into.resolve("building"), Map.of("lines.sou", LINES));

        Ran ran = run("-cp", classes.toString(), "--library", into.resolve("native").toString(),
                "--with", object.toString(), "--php", into.resolve("php").toString(),
                "--namespace", "Acme", building.toString());

        assertThat(ran.ended()).as(ran.said()).isZero();
        assertThat(into.resolve("php").resolve("Shop").resolve("Money").resolve("Money.php"))
                .as("the other build's type, offered by the object it wrote")
                .exists();
    }

    @Test
    void aNamespacePhpRefusesIsRefusedBeforeTheLibraryIsWritten(@TempDir Path into)
            throws Exception {
        Path model = sources(into, Map.of("money.sou", MONEY));

        Ran ran = run("--library", into.resolve("native").toString(),
                "--php", into.resolve("php").toString(), "--namespace", "class",
                model.toString());

        assertThat(ran.ended()).isEqualTo(2);
        assertThat(ran.said()).contains("class");
        assertThat(into.resolve("native")).doesNotExist();
    }

    @Test
    void aProgramTheLanguageRefusesWritesNothing(@TempDir Path into) throws Exception {
        Path model = sources(into, Map.of("lines.sou", LINES));

        Ran ran = run("--library", into.resolve("native").toString(),
                "--php", into.resolve("php").toString(), "--namespace", "Acme",
                model.toString());

        assertThat(ran.ended()).isEqualTo(1);
        assertThat(into.resolve("native")).doesNotExist();
        assertThat(into.resolve("php")).doesNotExist();
    }

    @Test
    void aCommandIsOneOutput() {
        refused("options for two outputs", "-o", "a.o", "--library", "out", "m.sou");
        refused("--with without a library", "-o", "a.o", "--with", "b.o", "m.sou");
        refused("--php without a library", "-o", "a.o", "--php", "php", "--namespace", "N",
                "m.sou");
        refused("--php without a namespace", "--library", "out", "--php", "php", "m.sou");
        refused("--namespace without --php", "--library", "out", "--namespace", "N", "m.sou");
        refused("the binding inside the library", "--library", "out", "--php", "out/php",
                "--namespace", "N", "m.sou");
        refused("the library inside the binding", "--library", "php/out", "--php", "php",
                "--namespace", "N", "m.sou");
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
                java.util.Optional.of(new Main.PhpBinding(Path.of("php"), "N"))));
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
