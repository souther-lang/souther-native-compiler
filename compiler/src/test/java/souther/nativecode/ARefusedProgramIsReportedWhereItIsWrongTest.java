package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermission;
import java.util.Set;

import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assumptions.assumeFalse;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

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

    @Test
    void sourcesOfOneNameAreToldApartAndEachIsQuotedForWhatIsWrongInIt(@TempDir Path room)
            throws Exception {
        Files.createDirectories(room.resolve("a"));
        Files.createDirectories(room.resolve("b"));
        Files.writeString(room.resolve("a/model.sou"), """
                module a

                behavior f : (n: Int) -> String

                let f (n) = n
                """);
        Files.writeString(room.resolve("b/model.sou"), """
                module b

                behavior g : (n: Int) -> String

                let g (n) = n
                """);

        // One refusal with a diagnostic in each source: each is said, under enough of its path to
        // tell it from the other, quoting the line of its own.
        Ran ran = run("-o", room.resolve("model.o").toString(), room.toString());

        assertThat(ran.ended()).isEqualTo(1);
        assertThat(quotedUnder(ran.said(), "a/model.sou:5:13")).contains("let f (n) = n")
                .doesNotContain("let g (n) = n");
        assertThat(quotedUnder(ran.said(), "b/model.sou:5:13")).contains("let g (n) = n")
                .doesNotContain("let f (n) = n");
    }

    /** What a report says from the header naming {@code place} up to the next report's header. */
    private static String quotedUnder(String said, String place) {
        int at = said.indexOf(place);
        assertThat(at).as("a report at " + place + " in\n" + said).isNotNegative();
        int next = said.indexOf("\n-- ", at);
        return said.substring(at, next < 0 ? said.length() : next);
    }

    @Test
    void aLoneFileWithNoHeaderIsTheModuleItsFileNames(@TempDir Path room) throws Exception {
        // As souther compile takes it (spec #modules): a header is optional in a lone file.
        Path source = Files.writeString(room.resolve("counting.sou"), """
                behavior next : (n: Int) -> Int

                let next (n) = n + 1
                """);
        Path into = room.resolve("counting.o");

        Ran ran = run("-o", into.toString(), source.toString());

        assertThat(ran.ended()).as(ran.said()).isZero();
        assertThat(into).isNotEmptyFile();
    }

    @Test
    void aLinkUnderADirectoryIsNotReadAsASource(@TempDir Path room) throws Exception {
        Path elsewhere = Files.writeString(room.resolve("elsewhere.txt"), """
                module elsewhere
                """);
        Path tree = Files.createDirectories(room.resolve("tree"));
        Files.createSymbolicLink(tree.resolve("linked.sou"), elsewhere);

        // A report quotes the source it is about, so what is read under a directory is what is
        // there, and a link to something that was never meant to be a source is refused.
        Ran ran = run("-o", room.resolve("tree.o").toString(), tree.toString());

        assertThat(ran.ended()).isEqualTo(2);
        assertThat(room.resolve("tree.o")).doesNotExist();
    }

    @Test
    void aDirectoryThatCannotAllBeReadIsRefusedRatherThanBuiltFromWhatCould(@TempDir Path room)
            throws Exception {
        Path tree = Files.createDirectories(room.resolve("project"));
        Files.createDirectories(tree.resolve("public"));
        Files.writeString(tree.resolve("public/counting.sou"), """
                module counting

                behavior doubled : (n: Int) -> Int

                let doubled (n) = n + n
                """);
        Path closed = Files.createDirectories(tree.resolve("private"));
        Files.writeString(closed.resolve("hidden.sou"), """
                module hidden

                behavior one : (n: Int) -> Int

                let one (n) = 1
                """);
        Path into = room.resolve("project.o");
        Set<PosixFilePermission> had;
        try {
            had = Files.getPosixFilePermissions(closed);
        } catch (UnsupportedOperationException e) {
            assumeTrue(false, "permissions cannot be taken away here: " + e);
            return;
        }
        Files.setPosixFilePermissions(closed, Set.of());
        Ran ran;
        try {
            assumeFalse(Files.isReadable(closed), "this process reads a directory with no permissions");
            ran = run("-o", into.toString(), tree.toString());
        } finally {
            Files.setPosixFilePermissions(closed, had);
        }

        // Nothing need import hidden, so what could be read compiles. An object file built from it
        // is a program nobody named.
        assertThat(ran.ended()).isEqualTo(2);
        assertThat(ran.said()).contains("private");
        assertThat(into).doesNotExist();
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
