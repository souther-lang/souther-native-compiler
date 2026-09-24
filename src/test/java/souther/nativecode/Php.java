package souther.nativecode;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * PHP, as every test runs it.
 *
 * <p>What a script prints is what a test compares, and what PHP says about the run goes apart from
 * it: read together, a line the environment adds (a startup warning about an extension the runner
 * happens to load) makes a script that did what it should compare as one that did not. What PHP
 * says goes to stderr, where it is read too: a warning or a notice from what a binding does is a
 * failure of the run, and a run saying anything there fails, with what it said.
 */
public final class Php {

    /**
     * Settings every run is given, ahead of a test's own: diagnostics to stderr, all of them, and
     * each once; and no JIT, which no test is about and which some environments warn about at
     * startup.
     */
    private static final List<String> SETTINGS = List.of(
            "-d", "display_errors=stderr",
            "-d", "log_errors=0",
            "-d", "error_reporting=-1",
            "-d", "opcache.jit=disable");

    private Php() {
    }

    /** What PHP printed, run with {@code arguments}, where it ended well and said nothing else. */
    public static String ran(List<String> arguments) throws IOException, InterruptedException {
        Ran ran = run(arguments);
        if (ran.status != 0 || !ran.said.isEmpty()) {
            throw new AssertionError("php " + arguments + " ended with " + ran.status
                    + "\nprinted:\n" + ran.printed + "\nsaid:\n" + ran.said);
        }
        return ran.printed;
    }

    /** Whether PHP compiles {@code file}, by what it ends with. */
    public static boolean compiles(Path file) throws IOException, InterruptedException {
        return run(List.of("-l", file.toString())).status == 0;
    }

    private record Ran(int status, String printed, String said) {
    }

    private static Ran run(List<String> arguments) throws IOException, InterruptedException {
        List<String> command = new ArrayList<>();
        command.add("php");
        command.addAll(SETTINGS);
        command.addAll(arguments);
        // What it says goes to a file rather than to a pipe read second: two pipes read one after
        // the other deadlock where the one not being read fills up first.
        Path said = Files.createTempFile("php-", ".said");
        try {
            Process process = new ProcessBuilder(command).redirectError(said.toFile()).start();
            String printed =
                    new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            int status = process.waitFor();
            return new Ran(status, printed, Files.readString(said, StandardCharsets.UTF_8));
        } finally {
            Files.deleteIfExists(said);
        }
    }
}
