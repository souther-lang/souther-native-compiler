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
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The command with its generators installed writes a library and the PHP and the Rust binding of
 * it, as the API does and through it: an application needs no Java of its own to build what it
 * runs.
 *
 * <p>The compiler's own tests hold the command to what it does with any generator, through ones that
 * only record what they were asked. What is here is what needs the real ones: that each is found by
 * the id the catalog names it by, and what each refuses before the library is built.
 */
class TheLauncherWritesTheBindingsItFindsTest {

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
    void everyBindingTheCatalogNamesHasAGeneratorHere() throws Exception {
        Bindings installed = Bindings.installed();

        for (KnownBindings.Kind kind : KnownBindings.all()) {
            assertThat(installed.generatorFor(kind).id()).isEqualTo(kind.id());
        }
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
        assertThat(ran.printed()).contains("from 2 sources").contains("wrote the PHP binding");
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

    /**
     * What a binding is asked with is its generator's to hold it to, and it does so before the
     * library is built, as it does for a name its language will not take.
     */
    @Test
    void aBindingMissingWhatItIsAskedWithIsRefusedBeforeTheLibraryIsWritten(@TempDir Path into)
            throws Exception {
        Path model = sources(into, Map.of("money.sou", MONEY));

        Ran php = run("--library", into.resolve("native").toString(),
                "--php", into.resolve("php").toString(), model.toString());
        Ran rust = run("--library", into.resolve("native").toString(),
                "--rust", into.resolve("rust").toString(), model.toString());

        assertThat(php.ended()).isEqualTo(2);
        assertThat(php.said()).contains("--php wants --namespace");
        assertThat(rust.ended()).isEqualTo(2);
        assertThat(rust.said()).contains("--rust wants --crate");
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
