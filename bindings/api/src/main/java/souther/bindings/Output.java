package souther.bindings;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;

/**
 * A directory that is what one generation wrote, and nothing else.
 *
 * <p>Written beside where it goes and put in place whole once everything in it is written, so that
 * the directory is only ever the binding of one manifest: never a binding with a class the model no
 * longer declares left from the one before, and never half of one where a later name was refused.
 * What it replaces has to be a binding of the same host this wrote, which it says by the mark the
 * host's generator names; a directory holding anything else, a binding of another host among it,
 * is refused rather than deleted.
 */
public final class Output {

    private final Path target;
    private final Path staging;

    private Output(Path target, Path staging) {
        this.target = target;
        this.staging = staging;
    }

    /**
     * Where a binding bound for {@code target} is written before it is put there, marked with
     * {@code mark}: the name of the file that says a directory is a binding of one host's.
     *
     * @throws NotBindable where {@code target} holds something no generation for that host wrote
     */
    public static Output replacing(Path target, String mark) throws IOException {
        Path absolute = replaceable(target, mark);
        Path parent = absolute.getParent();
        Files.createDirectories(parent);
        Path staging = Files.createTempDirectory(parent, absolute.getFileName() + ".writing-");
        Files.writeString(staging.resolve(mark), "Written by souther-native-compiler from"
                + " souther.json, and replaced whole on every generation.\n",
                StandardCharsets.UTF_8);
        return new Output(absolute, staging);
    }

    /**
     * {@code target} as it is replaced, where it is empty, absent or a binding marked with
     * {@code mark}.
     *
     * @throws NotBindable where {@code target} holds something no generation for that host wrote
     */
    public static Path replaceable(Path target, String mark) throws IOException {
        Path absolute = target.toAbsolutePath().normalize();
        if (Files.exists(absolute) && !ours(absolute, mark)) {
            throw new NotBindable(absolute + " holds files a binding did not write,"
                    + " and a binding replaces the directory it is written to whole");
        }
        return absolute;
    }

    /** Where what is written goes until it is put in place. */
    public Path staging() {
        return staging;
    }

    /** Where {@code written}, a path under {@link #staging()}, stands once it is put in place. */
    public Path placed(Path written) {
        return target.resolve(staging.relativize(written));
    }

    /** Puts what was written in place of what was there. */
    public void commit() throws IOException {
        if (!Files.exists(target)) {
            Files.move(staging, target, StandardCopyOption.ATOMIC_MOVE);
            return;
        }
        Path former = Files.createTempDirectory(target.getParent(),
                target.getFileName() + ".former-");
        Files.delete(former);
        Files.move(target, former, StandardCopyOption.ATOMIC_MOVE);
        try {
            Files.move(staging, target, StandardCopyOption.ATOMIC_MOVE);
        } catch (IOException e) {
            Files.move(former, target, StandardCopyOption.ATOMIC_MOVE);
            throw e;
        }
        remove(former);
    }

    /** Drops what was written, where it is not to be put in place. */
    public void abandon() throws IOException {
        remove(staging);
    }

    /** Whether {@code directory} is empty, or a binding marked with {@code mark}. */
    private static boolean ours(Path directory, String mark) throws IOException {
        if (!Files.isDirectory(directory)) {
            return false;
        }
        try (Stream<Path> entries = Files.list(directory)) {
            List<Path> held = entries.toList();
            return held.isEmpty() || held.contains(directory.resolve(mark));
        }
    }

    private static void remove(Path directory) throws IOException {
        if (!Files.exists(directory)) {
            return;
        }
        try (Stream<Path> walked = Files.walk(directory)) {
            for (Path each : walked.sorted(Comparator.reverseOrder()).toList()) {
                Files.delete(each);
            }
        }
    }
}
