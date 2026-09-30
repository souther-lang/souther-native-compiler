package souther.nativecode;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

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

    /**
     * Whether PHP compiles each of {@code files}, each on its own, asked of one PHP.
     *
     * <p>PHP 8.3 and later lint every file {@code -l} is given, each apart from the others: a file
     * whose error is fatal (a class named {@code int}) ends that file's compile, not the run, and
     * a class one file declares is not declared to the next. What it ends with says only whether
     * every file compiled, so each file is read from the line PHP prints for it, in the order they
     * were given. A PHP that does not print one such line for every file (one before 8.3 lints the
     * first alone) fails the run rather than answering for the first.
     */
    public static Map<Path, Boolean> compiles(List<Path> files)
            throws IOException, InterruptedException {
        if (files.isEmpty()) {
            return Map.of();
        }
        if (new HashSet<>(files).size() != files.size()) {
            throw new IllegalArgumentException("a file asked about twice: " + files);
        }
        List<String> arguments = new ArrayList<>();
        arguments.add("-l");
        files.forEach(file -> arguments.add(file.toString()));
        Ran ran = run(arguments);
        List<String> lines = ran.printed.lines().toList();
        Map<Path, Boolean> compiles = new LinkedHashMap<>();
        for (int i = 0; i < files.size() && i < lines.size(); i++) {
            String file = files.get(i).toString();
            if (lines.get(i).equals("No syntax errors detected in " + file)) {
                compiles.put(files.get(i), true);
            } else if (lines.get(i).equals("Errors parsing " + file)) {
                compiles.put(files.get(i), false);
            } else {
                break;
            }
        }
        if (compiles.size() != files.size() || lines.size() != files.size()
                || (ran.status == 0) != !compiles.containsValue(false)) {
            throw new AssertionError("php " + arguments + " did not say of each file, in turn,"
                    + " whether it compiles; it ended with " + ran.status
                    + "\nprinted:\n" + ran.printed + "\nsaid:\n" + ran.said);
        }
        return compiles;
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
