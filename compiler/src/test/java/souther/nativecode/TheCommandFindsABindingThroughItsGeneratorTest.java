package souther.nativecode;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.PrintStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A binding is written by a generator's jar, whichever way it was asked for: a jar named with
 * {@code --binding}, or the jar of a binding the catalog names. The generator is asked what the
 * command was given for it, in the order its moments come, and the directory it wrote is owned by the
 * generator its jar says it is.
 *
 * <p>The jars are written by the test from generators it compiled ({@link TestGenerators}), and
 * loaded as the command loads any.
 */
class TheCommandFindsABindingThroughItsGeneratorTest {

    private static final String MONEY = """
            module shop.money exposing ( Money )

            data Money = Int
                invariant notNegative = value >= 0
            """;

    private record Ran(int ended, String printed, String said) {
    }

    @BeforeEach
    @AfterEach
    void nothingAsked() {
        System.clearProperty(TestGenerators.ASKED);
    }

    @Test
    void aGeneratorIsAskedWhatTheCommandWasGivenForIt(@TempDir Path into) throws Exception {
        Path jar = GeneratorJar.of("acme.recording", TestGenerators.Recording.class)
                .writtenTo(into.resolve("jars/recording.jar"));
        Path model = model(into);

        Ran ran = run("--library", into.resolve("native").toString(), "--binding", jar.toString(),
                into.resolve("out").toString(), "--binding-option", "namespace=Acme=Shop",
                model.toString());

        assertThat(ran.ended()).as(ran.said()).isZero();
        assertThat(asked()).containsExactly(
                "preflight {namespace=Acme=Shop}",
                "generate {namespace=Acme=Shop} of 1 module into an empty directory");
        assertThat(ran.printed()).contains("wrote the binding " + into.resolve("out"));
        assertThat(into.resolve("out").resolve("generated.txt")).exists();
        BindingDirectory.Mark mark = BindingDirectory.read(
                Files.readString(into.resolve("out").resolve(BindingDirectory.MARK)));
        assertThat(mark).isEqualTo(new BindingDirectory.Mark("acme.recording",
                new BindingDirectory.Artifact.Local(Fetching.sha256(Files.readAllBytes(jar)))));
        assertThat(Files.readString(into.resolve("out").resolve(BindingDirectory.MARK)))
                .as("a local jar's path is not kept").doesNotContain(into.toString());
    }

    /**
     * A generator refusing, or failing, after another has written its binding changes no directory:
     * every binding is written before any is put in place, and what was there stays.
     */
    @Test
    void aBindingRefusedOrFailedLeavesEveryDirectoryAsItWas(@TempDir Path into) throws Exception {
        Path model = model(into);
        Path first = GeneratorJar.of("acme.first", TestGenerators.Recording.class)
                .writtenTo(into.resolve("jars/first.jar"));
        Path out = Files.createDirectories(into.resolve("out"));
        Files.writeString(out.resolve(BindingDirectory.MARK), BindingDirectory.written(
                new BindingDirectory.Mark("acme.first", new BindingDirectory.Artifact.Local("0".repeat(64)))));
        Files.writeString(out.resolve("before.txt"), "a binding written before");
        for (Class<? extends TestGenerators.Recording> second : List.of(
                TestGenerators.RefusingLate.class, TestGenerators.Failing.class)) {
            Path jar = GeneratorJar.of("acme.second", second)
                    .writtenTo(into.resolve("jars/" + second.getSimpleName() + ".jar"));

            Ran ran = run("--library", into.resolve("native").toString(), "--binding",
                    first.toString(), out.toString(), "--binding", jar.toString(),
                    into.resolve("second").toString(), model.toString());

            assertThat(ran.ended()).as(ran.said()).isEqualTo(1);
            assertThat(out.resolve("before.txt")).exists();
            assertThat(out.resolve("generated.txt")).doesNotExist();
            assertThat(into.resolve("second")).doesNotExist();
            try (var beside = Files.list(into)) {
                assertThat(beside.map(it -> it.getFileName().toString()))
                        .as("nothing written is left beside them")
                        .containsExactlyInAnyOrder("jars", "model", "native", "out");
            }
        }
    }

    /**
     * What a generator throws that is not a refusal is said as its failure, and not a trace: an
     * I/O failure of its own as well, which is the same type as the command's own and told apart
     * by where it was thrown, and one thrown making it.
     */
    @Test
    void aGeneratorsOwnFailureIsSaidInOneLine(@TempDir Path into) throws Exception {
        Path model = model(into);
        Map<Class<? extends TestGenerators.Recording>, String> thrown = Map.of(
                TestGenerators.Failing.class,
                "java.lang.IllegalStateException: the generator's own bug",
                TestGenerators.FailingOnItsDisk.class,
                "java.io.IOException: the generator's own disk",
                TestGenerators.FailingToBeMade.class,
                "the generator's own constructor");
        for (Map.Entry<Class<? extends TestGenerators.Recording>, String> each : thrown.entrySet()) {
            Path jar = GeneratorJar.of("acme.failing", each.getKey())
                    .writtenTo(into.resolve("jars/" + each.getKey().getSimpleName() + ".jar"));

            Ran ran = run("--library", into.resolve("native").toString(), "--binding",
                    jar.toString(), into.resolve("out").toString(), model.toString());

            assertThat(ran.ended()).as(ran.said()).isEqualTo(1);
            assertThat(ran.said()).contains("the generator acme.failing failed, and wrote nothing: ")
                    .contains(each.getValue()).doesNotContain("\tat ");
            assertThat(into.resolve("out")).doesNotExist();
        }
    }

    /**
     * What a generator throws is its own whatever its type, an {@link Error} among it: at every moment
     * the command calls it, it is said in one line as the generator's failure, and nothing the command
     * was writing is left, the directory it was writing the binding into among it.
     */
    @Test
    void anErrorOfTheGeneratorsOwnIsItsFailureAndLeavesNothing(@TempDir Path into)
            throws Exception {
        Path model = model(into);
        Map<Class<? extends TestGenerators.Recording>, String> thrown = Map.of(
                TestGenerators.ErringAhead.class, "the generator's own assertion, ahead",
                TestGenerators.Erring.class, "the generator's own assertion",
                TestGenerators.ErringToBeMade.class, "the generator's own assertion, made");
        for (Map.Entry<Class<? extends TestGenerators.Recording>, String> each : thrown.entrySet()) {
            Path jar = GeneratorJar.of("acme.erring", each.getKey())
                    .writtenTo(into.resolve("jars/" + each.getKey().getSimpleName() + ".jar"));

            Ran ran = run("--library", into.resolve("native").toString(), "--binding",
                    jar.toString(), into.resolve("out").toString(), model.toString());

            assertThat(ran.ended()).as(ran.said()).isEqualTo(1);
            assertThat(ran.said()).contains("the generator acme.erring failed, and wrote nothing: "
                    + "java.lang.AssertionError: " + each.getValue()).doesNotContain("\tat ");
            try (var beside = Files.list(into)) {
                assertThat(beside.map(it -> it.getFileName().toString()))
                        .as("nothing left beside where the binding was to go")
                        .allMatch(it -> List.of("jars", "model", "native").contains(it));
            }
        }
    }

    /**
     * A mark says who owns its directory in every version of the mark alike, so a directory a later
     * command marked is still its generator's here; and a directory another generator owns is refused
     * with that generator named.
     */
    @Test
    void aDirectoryIsOwnedByWhatItsMarkSaysInAnyVersion(@TempDir Path into) throws Exception {
        Path model = model(into);
        Path jar = GeneratorJar.of("acme.owner", TestGenerators.Recording.class)
                .writtenTo(into.resolve("jars/owner.jar"));
        Path out = Files.createDirectories(into.resolve("out"));
        Files.writeString(out.resolve(BindingDirectory.MARK), """
                {"format": "souther-binding", "version": 7, "generator": "acme.owner",
                 "provenance": {"whatever": "a later version says"}}
                """);

        Ran later = run("--library", into.resolve("native").toString(), "--binding",
                jar.toString(), out.toString(), model.toString());
        assertThat(later.ended()).as(later.said()).isZero();

        Files.writeString(out.resolve(BindingDirectory.MARK), """
                {"format": "souther-binding", "version": 7, "generator": "acme.another"}
                """);
        Ran another = run("--library", into.resolve("native").toString(), "--binding",
                jar.toString(), out.toString(), model.toString());
        assertThat(another.ended()).isEqualTo(2);
        assertThat(another.said()).contains("the binding the generator acme.another wrote");
    }

    @Test
    void aGeneratorThatRefusesAheadStopsTheBuildBeforeTheLibraryIsWritten(@TempDir Path into)
            throws Exception {
        Path jar = GeneratorJar.of("acme.refusing", TestGenerators.RefusingAhead.class)
                .writtenTo(into.resolve("jars/refusing.jar"));
        Path model = model(into);

        Ran ran = run("--library", into.resolve("native").toString(), "--binding", jar.toString(),
                into.resolve("out").toString(), model.toString());

        assertThat(ran.ended()).isEqualTo(2);
        assertThat(ran.said()).contains("not that namespace");
        assertThat(asked()).hasSize(1);
        assertThat(into.resolve("native")).doesNotExist();
    }

    /**
     * The directory a binding goes to is owned by the generator its jar says it is, and not by the
     * jar: a newer jar of the same generator replaces what an older one wrote, and another generator's
     * jar is refused it, as is a directory marked in a form this does not read.
     */
    @Test
    void aDirectoryIsOwnedByTheGeneratorAndNotByTheJarThatWroteIt(@TempDir Path into)
            throws Exception {
        Path model = model(into);
        Path older = GeneratorJar.of("acme.owner", TestGenerators.Recording.class)
                .writtenTo(into.resolve("jars/older.jar"));
        Path newer = GeneratorJar.of("acme.owner", TestGenerators.Recording.class)
                .holding(TestGenerators.Looking.class).writtenTo(into.resolve("jars/newer.jar"));
        Path another = GeneratorJar.of("acme.another", TestGenerators.Recording.class)
                .writtenTo(into.resolve("jars/another.jar"));
        Path out = into.resolve("out");

        Ran first = run("--library", into.resolve("native").toString(), "--binding", older.toString(),
                out.toString(), model.toString());
        Ran second = run("--library", into.resolve("native").toString(), "--binding",
                newer.toString(), out.toString(), model.toString());
        Ran refused = run("--library", into.resolve("native").toString(), "--binding",
                another.toString(), out.toString(), model.toString());

        assertThat(first.ended()).as(first.said()).isZero();
        assertThat(second.ended()).as(second.said()).isZero();
        assertThat(BindingDirectory.read(Files.readString(out.resolve(BindingDirectory.MARK))))
                .isEqualTo(new BindingDirectory.Mark("acme.owner", new BindingDirectory.Artifact.Local(
                        Fetching.sha256(Files.readAllBytes(newer)))));
        assertThat(refused.ended()).isEqualTo(2);
        assertThat(refused.said()).contains("the binding the generator acme.owner wrote")
                .contains("which acme.another does not replace");

        Files.writeString(out.resolve(BindingDirectory.MARK), "generator=acme.owner\n");
        Ran oldMark = run("--library", into.resolve("native").toString(), "--binding",
                newer.toString(), out.toString(), model.toString());
        assertThat(oldMark.ended()).isEqualTo(2);
        assertThat(oldMark.said()).contains("holds files a binding did not write");
    }

    /**
     * A binding of the catalog is the jar of its generator too: in a build that is not a release,
     * the one the build names, which has to say it is that generator; and where none is named, the
     * command says where one is, and builds nothing.
     */
    @Test
    void aBindingOfTheCatalogIsTheJarItsBuildNames(@TempDir Path into) throws Exception {
        Path model = model(into);
        String property = GeneratorSpec.DEVELOPMENT_PROPERTY + "php";
        Path php = GeneratorJar.of("php", TestGenerators.Recording.class)
                .writtenTo(into.resolve("jars/php.jar"));
        Path rust = GeneratorJar.of("rust", TestGenerators.Recording.class)
                .writtenTo(into.resolve("jars/rust.jar"));
        String before = System.getProperty(property);
        try {
            System.clearProperty(property);
            Ran none = run("--library", into.resolve("native").toString(), "--php",
                    into.resolve("out").toString(), "--namespace", "Acme", model.toString());
            assertThat(none.ended()).isEqualTo(2);
            assertThat(none.said()).contains("-D" + property);
            assertThat(into.resolve("native")).doesNotExist();

            System.setProperty(property, rust.toString());
            Ran another = run("--library", into.resolve("native").toString(), "--php",
                    into.resolve("out").toString(), "--namespace", "Acme", model.toString());
            assertThat(another.ended()).isEqualTo(2);
            assertThat(another.said()).contains("says it is the generator \"rust\"")
                    .contains("\"php\"");

            System.setProperty(property, php.toString());
            Ran ran = run("--library", into.resolve("native").toString(), "--php",
                    into.resolve("out").toString(), "--namespace", "Acme", model.toString());
            assertThat(ran.ended()).as(ran.said()).isZero();
            assertThat(ran.printed()).contains("with the PHP generator");
            assertThat(asked()).startsWith("preflight {namespace=Acme}");
        } finally {
            if (before == null) {
                System.clearProperty(property);
            } else {
                System.setProperty(property, before);
            }
        }
    }

    /**
     * A jar named with {@code --binding} has a qualified id: one with no dot is reserved for the
     * bindings this command ships, those it ships now and those it has not yet, so that no generator of
     * someone else's comes to be refused, with the directories it owns, when the catalog grows.
     */
    @Test
    void aJarNamedOnTheCommandLineHasAQualifiedId(@TempDir Path into) throws Exception {
        for (String id : List.of("php", "swift", "kotlin-binding")) {
            Path jar = GeneratorJar.of(id, TestGenerators.Watched.class)
                    .writtenTo(into.resolve("jars/" + id + ".jar"));

            Ran ran = run("--library", into.resolve("native").toString(), "--binding",
                    jar.toString(), into.resolve("out").toString(), model(into).toString());

            assertThat(ran.ended()).as(id).isEqualTo(2);
            assertThat(ran.said()).as(id).contains("reserved for the bindings this command ships");
            assertThat(asked()).as("none of its code ran").isEmpty();
        }
    }

    @Test
    void theCatalogIsWhatTheUsageAndTheOptionsAreReadFrom() {
        Ran ran = run();

        assertThat(ran.ended()).isEqualTo(2);
        assertThat(ran.said()).contains("--binding <generator> <dir>")
                .contains("--binding-option <key>=<value>");
        for (KnownBindings.Kind kind : KnownBindings.all()) {
            assertThat(ran.said()).contains(kind.flag() + " <dir>");
            for (String option : kind.options()) {
                assertThat(ran.said()).contains("--" + option + " <value>");
                assertThat(KnownBindings.qualifiedBy("--" + option)).contains(kind);
            }
            assertThat(KnownBindings.askedBy(kind.flag())).contains(kind);
            assertThat(kind.id()).as("an id the catalog names is a reserved one")
                    .matches(IdRule.RESERVED);
        }
    }

    private static List<String> asked() {
        String said = System.getProperty(TestGenerators.ASKED, "");
        return said.isEmpty() ? List.of() : said.lines().toList();
    }

    private static Path model(Path into) throws IOException {
        Path model = into.resolve("model");
        Files.createDirectories(model);
        Files.writeString(model.resolve("money.sou"), MONEY, StandardCharsets.UTF_8);
        return model;
    }

    /** As a build from a clone runs, with nothing to fetch. */
    static Fetching unreleased(Path cache) {
        return new Fetching(cache, false, URI.create("http://127.0.0.1:9/maven"),
                URI.create("http://127.0.0.1:9/releases"), null, Map.of(), address -> {
                    throw new IOException("a build that is not a release fetches nothing: " + address);
                });
    }

    private static Ran run(String... args) {
        ByteArrayOutputStream printed = new ByteArrayOutputStream();
        ByteArrayOutputStream said = new ByteArrayOutputStream();
        Path cache;
        try {
            cache = Files.createTempDirectory("souther-cache");
        } catch (IOException e) {
            throw new java.io.UncheckedIOException(e);
        }
        int ended = Main.run(args, new PrintStream(printed, true, StandardCharsets.UTF_8),
                new PrintStream(said, true, StandardCharsets.UTF_8), unreleased(cache));
        return new Ran(ended, printed.toString(StandardCharsets.UTF_8),
                said.toString(StandardCharsets.UTF_8));
    }
}
