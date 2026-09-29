package souther.bindings;

import java.nio.file.Path;
import java.util.List;

/** What a binding is written as: the directory it stands in, and every file in it. */
public record Generated(Path root, List<Path> files) {

    public Generated {
        files = List.copyOf(files);
    }
}
