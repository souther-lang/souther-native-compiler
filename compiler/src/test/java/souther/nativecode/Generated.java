package souther.nativecode;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Stream;

/**
 * A binding a test had written: the directory it stands in, and every file a generator wrote in
 * it, which is every file but the mark the command puts beside them ({@link BindingDirectory#MARK}).
 */
public record Generated(Path root, List<Path> files) {

    public Generated {
        files = List.copyOf(files);
    }

    /** The binding standing in {@code root}. */
    public static Generated of(Path root) {
        try (Stream<Path> walked = Files.walk(root)) {
            return new Generated(root, walked.filter(Files::isRegularFile)
                    .filter(it -> !it.getFileName().toString().equals(BindingDirectory.MARK))
                    .sorted().toList());
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }
}
