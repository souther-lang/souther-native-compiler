package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A program the language refuses is said as the language's human report, as {@code souther compile}
 * says it for the JVM: the source named, the line quoted and a caret under the place, and not the
 * exception's message on a line of its own.
 */
class ARefusedProgramIsReportedWhereItIsWrongTest {

    @Test
    void aSyntaxErrorIsQuotedWhereItIs(@TempDir Path room) throws Exception {
        Path source = Files.writeString(room.resolve("bad.sou"), """
                module bad

                behavior x : (n: Int) -> Int

                let x (n) = n +
                """);
        Path into = room.resolve("bad.o");

        Ran ran = run(source.toString(), "-o", into.toString());

        assertThat(ran.ended()).isEqualTo(1);
        assertThat(ran.said()).contains("-- SYNTAX ERROR").contains("E2302")
                .contains("bad.sou:").contains("^");
        assertThat(into).doesNotExist();
    }

    @Test
    void aTypeErrorIsQuotedOverMoreThanOneLine(@TempDir Path room) throws Exception {
        Path source = Files.writeString(room.resolve("typed.sou"), """
                module typed

                behavior f : (n: Int) -> String

                let f (n) = n
                """);

        Ran ran = run(source.toString(), "-o", room.resolve("typed.o").toString());

        assertThat(ran.ended()).isEqualTo(1);
        assertThat(ran.said()).contains("typed.sou:5:").contains("let f (n) = n").contains("^");
        assertThat(ran.said().lines().count()).isGreaterThan(1);
    }

    @Test
    void theSourceQuotedIsTheOneTheReportIsAbout(@TempDir Path room) throws Exception {
        Files.writeString(room.resolve("a.sou"), """
                module a

                behavior fine : (n: Int) -> Int

                let fine (n) = n + 1
                """);
        Files.writeString(room.resolve("b.sou"), """
                module b

                behavior broken : (n: Int) -> Int

                let broken (n) = n + missing
                """);

        // A report names its source by the position it was handed over in; read off anything else
        // and the caret lands in a text the report says nothing about.
        Ran ran = run("--library", room.resolve("native").toString(), room.toString());

        assertThat(ran.ended()).isEqualTo(1);
        assertThat(ran.said()).contains("b.sou:5:22").contains("let broken (n) = n + missing")
                .doesNotContain("a.sou").doesNotContain("let fine (n) = n + 1");
        assertThat(room.resolve("native")).doesNotExist();
    }

    private record Ran(int ended, String printed, String said) {
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
