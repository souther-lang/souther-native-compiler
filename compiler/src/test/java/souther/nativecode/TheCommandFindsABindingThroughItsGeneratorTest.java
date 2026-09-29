package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.Generated;
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

    /** A generator that writes nothing and says what it was asked. */
    private static final class Recording implements BindingGenerator {
        final List<String> asked = new ArrayList<>();
        private final String id;
        private final String refuses;

        Recording(String id, String refuses) {
            this.id = id;
            this.refuses = refuses;
        }

        @Override
        public String id() {
            return id;
        }

        @Override
        public void preflight(Path into, Map<String, String> options) {
            asked.add("preflight " + into.getFileName() + " " + options);
            if (refuses != null) {
                throw new NotBindable(refuses);
            }
        }

        @Override
        public Generated generate(BindingInput input, Path into, Map<String, String> options)
                throws IOException {
            asked.add("generate " + into.getFileName() + " " + options
                    + " of " + input.manifest().modules().size() + " module");
            Files.createDirectories(into);
            return new Generated(into, List.of());
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
                "preflight out {namespace=Acme}",
                "generate out {namespace=Acme} of 1 module");
        assertThat(ran.printed()).contains("wrote the PHP binding");
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
