package souther.nativecode;

import souther.bindings.NotBindable;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;

/**
 * A directory a binding is written to, which is what one generation wrote and nothing else.
 *
 * <p>A generator writes into an empty directory beside where the binding goes ({@link #staging}),
 * and the command puts it in place whole ({@link #commit}), so that the directory is only ever the
 * binding of one manifest: never a binding with a class the model no longer declares left from
 * the one before, and never half of one where a later name was refused. What it replaces has to be
 * a binding of the same generator, which the one file {@value #MARK} says, naming the generator; a
 * directory holding anything else, a binding of another host among it, is refused rather than
 * deleted.
 */
public final class BindingDirectory {

    /** The file that says a directory is a binding, and which generator wrote it. */
    public static final String MARK = ".souther-binding";

    private final Path target;
    private final Path staging;
    private final String generator;

    private BindingDirectory(Path target, Path staging, String generator) {
        this.target = target;
        this.staging = staging;
        this.generator = generator;
    }

    /**
     * {@code target} as it is replaced, where it is absent, empty, or a binding {@code generator}
     * wrote.
     *
     * @throws NotBindable where {@code target} holds anything else
     */
    public static Path replaceable(Path target, String generator) throws IOException {
        Path absolute = target.toAbsolutePath().normalize();
        if (Files.exists(absolute) && !ours(absolute, generator)) {
            throw new NotBindable(absolute + " holds files a binding did not write,"
                    + " and a binding replaces the directory it is written to whole");
        }
        return absolute;
    }

    /**
     * An empty directory beside {@code target} for {@code generator} to write the binding into.
     *
     * @throws NotBindable where {@code target} holds what no generation of {@code generator} wrote
     */
    public static BindingDirectory staging(Path target, String generator) throws IOException {
        Path absolute = replaceable(target, generator);
        Path parent = absolute.getParent();
        Files.createDirectories(parent);
        Path staging = Files.createTempDirectory(parent, absolute.getFileName() + ".writing-");
        return new BindingDirectory(absolute, staging, generator);
    }

    /** What a generator writes into, and how, for {@link #written}. */
    @FunctionalInterface
    public interface Writing {
        void into(Path directory) throws IOException;
    }

    /**
     * {@code target} as {@code writing} wrote it, put in place whole, or as it was where writing
     * threw.
     */
    public static Path written(Path target, String generator, Writing writing) throws IOException {
        BindingDirectory directory = staging(target, generator);
        try {
            writing.into(directory.staging());
        } catch (IOException | RuntimeException e) {
            directory.abandon();
            throw e;
        }
        directory.commit();
        return directory.target();
    }

    /** Where the binding is written until it is put in place. */
    public Path staging() {
        return staging;
    }

    /** Where the binding goes. */
    public Path target() {
        return target;
    }

    /** Marks what was written as {@code generator}'s binding, and puts it in place of what was there. */
    public void commit() throws IOException {
        Files.writeString(staging.resolve(MARK), "Written by souther-native-compiler, and replaced"
                + " whole on every generation.\ngenerator=" + generator + "\n", StandardCharsets.UTF_8);
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

    /** Whether {@code directory} is empty, or a binding {@code generator} wrote. */
    private static boolean ours(Path directory, String generator) throws IOException {
        if (!Files.isDirectory(directory)) {
            return false;
        }
        try (Stream<Path> entries = Files.list(directory)) {
            List<Path> held = entries.toList();
            if (held.isEmpty()) {
                return true;
            }
        }
        Path mark = directory.resolve(MARK);
        return Files.isRegularFile(mark) && Files.readAllLines(mark, StandardCharsets.UTF_8)
                .contains("generator=" + generator);
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
