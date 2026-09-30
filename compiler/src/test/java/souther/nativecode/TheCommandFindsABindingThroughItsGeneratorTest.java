package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.NotBindable;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Which bindings there are is the command's catalog, and a binding is written by the generator that
 * answers to its id, whichever it is: a generator is asked what the command was given for it, in
 * the order its moments come, and one that is not there is a command that could be run where it
 * is, and not a command that is wrong.
 */
class TheCommandFindsABindingThroughItsGeneratorTest {

    private static final String MONEY = """
            module shop.money exposing ( Money )

            data Money = Int
                invariant notNegative = value >= 0
            """;

    /** A generator that writes one file and says what it was asked, or refuses or fails where told to. */
    private static class Recording implements BindingGenerator {
        final List<String> asked = new ArrayList<>();
        private final String id;
        private final String refuses;
        private final RuntimeException generating;

        Recording(String id, String refuses) {
            this(id, refuses, null);
        }

        Recording(String id, String refuses, RuntimeException generating) {
            this.id = id;
            this.refuses = refuses;
            this.generating = generating;
        }

        @Override
        public String id() {
            return id;
        }

        @Override
        public Set<String> options() {
            return Set.of("namespace", "crate");
        }

        @Override
        public void preflight(Map<String, String> options) {
            asked.add("preflight " + options);
            if (refuses != null) {
                throw new NotBindable(refuses);
            }
        }

        @Override
        public void generate(BindingInput input, Path into, Map<String, String> options)
                throws IOException {
            boolean empty;
            try (var held = Files.list(into)) {
                empty = held.findAny().isEmpty();
            }
            asked.add("generate " + options + " of " + input.manifest().modules().size()
                    + " module into " + (empty ? "an empty" : "a") + " directory");
            if (generating != null) {
                throw generating;
            }
            Files.writeString(into.resolve(id + ".txt"), "written by " + id);
        }
    }

    private record Ran(int ended, String printed, String said) {
    }

    @Test
    void aGeneratorIsAskedWhatTheCommandWasGivenForIt(@TempDir Path into) throws Exception {
        Recording php = new Recording("php", null);
        Path model = model(into);

        Ran ran = run(Bindings.of(List.of(php)), "--library", into.resolve("native").toString(),
                "--php", into.resolve("out").toString(), "--namespace", "Acme", model.toString());

        assertThat(ran.ended()).as(ran.said()).isZero();
        assertThat(php.asked).containsExactly(
                "preflight {namespace=Acme}",
                "generate {namespace=Acme} of 1 module into an empty directory");
        assertThat(ran.printed()).contains("wrote the PHP binding");
        assertThat(into.resolve("out").resolve("php.txt")).exists();
        assertThat(Files.readString(into.resolve("out").resolve(BindingDirectory.MARK)))
                .contains("generator=php");
    }

    /**
     * A generator refusing, or failing, after another has written its binding changes no directory:
     * every binding is written before any is put in place, and what was there stays.
     */
    @Test
    void aBindingRefusedOrFailedLeavesEveryDirectoryAsItWas(@TempDir Path into) throws Exception {
        Path model = model(into);
        Path out = Files.createDirectories(into.resolve("out"));
        Files.writeString(out.resolve(BindingDirectory.MARK), "generator=php\n");
        Files.writeString(out.resolve("before.txt"), "a binding written before");
        for (Recording rust : List.of(new Recording("rust", null, new NotBindable("no such crate")),
                new Recording("rust", null, new IllegalStateException("the generator's own bug")))) {
            Ran ran = run(Bindings.of(List.of(new Recording("php", null), rust)), "--library",
                    into.resolve("native").toString(), "--php", out.toString(), "--namespace",
                    "Acme", "--rust", into.resolve("rust").toString(), "--crate", "acme",
                    model.toString());

            assertThat(ran.ended()).as(ran.said()).isEqualTo(1);
            assertThat(out.resolve("before.txt")).exists();
            assertThat(out.resolve("php.txt")).doesNotExist();
            assertThat(into.resolve("rust")).doesNotExist();
            try (var beside = Files.list(into)) {
                assertThat(beside.map(it -> it.getFileName().toString()))
                        .as("nothing written is left beside them")
                        .containsExactlyInAnyOrder("model", "native", "out");
            }
        }
    }

    /** What a generator throws that is not a refusal is said as its failure, and not a trace. */
    @Test
    void aGeneratorsOwnFailureIsSaidInOneLine(@TempDir Path into) throws Exception {
        Path model = model(into);

        Ran ran = run(Bindings.of(List.of(new Recording("php", null,
                        new IllegalStateException("the generator's own bug")))), "--library",
                into.resolve("native").toString(), "--php", into.resolve("out").toString(),
                "--namespace", "Acme", model.toString());

        assertThat(ran.ended()).isEqualTo(1);
        assertThat(ran.said()).contains("the PHP generator failed, and wrote nothing:"
                + " java.lang.IllegalStateException: the generator's own bug")
                .doesNotContain("\tat ");
    }

    /** An option the catalog reads that the generator does not take is refused before anything is built. */
    @Test
    void anOptionTheGeneratorDoesNotTakeIsRefused(@TempDir Path into) throws Exception {
        Recording taking = new Recording("php", null) {
            @Override
            public Set<String> options() {
                return Set.of("crate");
            }
        };
        Path model = model(into);

        Ran ran = run(Bindings.of(List.of(taking)), "--library", into.resolve("native").toString(),
                "--php", into.resolve("out").toString(), "--namespace", "Acme", model.toString());

        assertThat(ran.ended()).isEqualTo(2);
        assertThat(ran.said()).contains("the PHP generator takes no --namespace");
        assertThat(into.resolve("native")).doesNotExist();
    }

    @Test
    void aGeneratorThatRefusesAheadStopsTheBuildBeforeTheLibraryIsWritten(@TempDir Path into)
            throws Exception {
        Recording php = new Recording("php", "not that namespace");
        Path model = model(into);

        Ran ran = run(Bindings.of(List.of(php)), "--library", into.resolve("native").toString(),
                "--php", into.resolve("out").toString(), "--namespace", "Acme", model.toString());

        assertThat(ran.ended()).isEqualTo(2);
        assertThat(ran.said()).contains("not that namespace");
        assertThat(php.asked).hasSize(1);
        assertThat(into.resolve("native")).doesNotExist();
    }

    @Test
    void aBindingWhoseGeneratorIsNotThereIsNamedForTheArtifactThatBringsIt(@TempDir Path into)
            throws Exception {
        Path model = model(into);

        Ran ran = run(Bindings.of(List.of(new Recording("rust", null))), "--library",
                into.resolve("native").toString(), "--php", into.resolve("out").toString(),
                "--namespace", "Acme", model.toString());

        assertThat(ran.ended()).isEqualTo(2);
        assertThat(ran.said()).contains("--php").contains("org.souther-lang:souther-binding-php");
        assertThat(into.resolve("native")).as("nothing is built for a binding it cannot write")
                .doesNotExist();
    }

    @Test
    void theUnavailableBindingIsAnAnswerDistinctFromAWrongCommand() {
        Bindings none = Bindings.of(List.of());

        assertThatThrownBy(() -> none.generatorFor(KnownBindings.askedBy("--php").orElseThrow()))
                .isInstanceOfSatisfying(BindingUnavailable.class,
                        e -> assertThat(e.kind().id()).isEqualTo("php"));
    }

    @Test
    void twoGeneratorsForOneIdAreRefusedAndNotOneChosen() {
        assertThatThrownBy(() -> Bindings.of(
                List.of(new Recording("php", null), new Recording("php", null))))
                .isInstanceOf(IllegalStateException.class).hasMessageContaining("\"php\"");
    }

    @Test
    void theCatalogIsWhatTheUsageAndTheOptionsAreReadFrom() throws Exception {
        Ran ran = run(Bindings.of(List.of()));

        assertThat(ran.ended()).isEqualTo(2);
        for (KnownBindings.Kind kind : KnownBindings.all()) {
            assertThat(ran.said()).contains(kind.flag() + " <dir>");
            for (String option : kind.options()) {
                assertThat(ran.said()).contains("--" + option + " <value>");
                assertThat(KnownBindings.qualifiedBy("--" + option)).contains(kind);
            }
            assertThat(KnownBindings.askedBy(kind.flag())).contains(kind);
        }
    }

    private static Path model(Path into) throws IOException {
        Path model = into.resolve("model");
        Files.createDirectories(model);
        Files.writeString(model.resolve("money.sou"), MONEY, StandardCharsets.UTF_8);
        return model;
    }

    private static Ran run(Bindings generators, String... args) {
        ByteArrayOutputStream printed = new ByteArrayOutputStream();
        ByteArrayOutputStream said = new ByteArrayOutputStream();
        int ended = Main.run(args, new PrintStream(printed, true, StandardCharsets.UTF_8),
                new PrintStream(said, true, StandardCharsets.UTF_8), generators);
        return new Ran(ended, printed.toString(StandardCharsets.UTF_8),
                said.toString(StandardCharsets.UTF_8));
    }
}
