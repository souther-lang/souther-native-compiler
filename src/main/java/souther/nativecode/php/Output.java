package souther.nativecode.php;

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
 * What it replaces has to be a binding this wrote, which it says by {@link #MARK}; a directory
 * holding anything else is refused rather than deleted.
 */
final class Output {

    /** What says a directory is a binding this wrote, and may be replaced whole. */
    static final String MARK = ".souther-php-binding";

    private final Path target;
    private final Path staging;

    private Output(Path target, Path staging) {
        this.target = target;
        this.staging = staging;
    }

    /**
     * Where a binding bound for {@code target} is written before it is put there.
     *
     * @throws PhpBindings.NotBindable where {@code target} holds something no generation wrote
     */
    static Output replacing(Path target) throws IOException {
        Path absolute = target.toAbsolutePath().normalize();
        if (Files.exists(absolute) && !ours(absolute)) {
            throw new PhpBindings.NotBindable(absolute + " holds files a binding did not write,"
                    + " and a binding replaces the directory it is written to whole");
        }
        Path parent = absolute.getParent();
        Files.createDirectories(parent);
        Path staging = Files.createTempDirectory(parent, absolute.getFileName() + ".writing-");
        Files.writeString(staging.resolve(MARK), "Written by souther-native-compiler from"
                + " souther.json, and replaced whole on every generation.\n",
                StandardCharsets.UTF_8);
        return new Output(absolute, staging);
    }

    /** Where what is written goes until it is put in place. */
    Path staging() {
        return staging;
    }

    /** Where {@code written}, a path under {@link #staging()}, stands once it is put in place. */
    Path placed(Path written) {
        return target.resolve(staging.relativize(written));
    }

    /** Puts what was written in place of what was there. */
    void commit() throws IOException {
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
    void abandon() throws IOException {
        remove(staging);
    }

    /** Whether {@code directory} is empty, or a binding this wrote. */
    private static boolean ours(Path directory) throws IOException {
        if (!Files.isDirectory(directory)) {
            return false;
        }
        try (Stream<Path> entries = Files.list(directory)) {
            List<Path> held = entries.toList();
            return held.isEmpty() || held.contains(directory.resolve(MARK));
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
