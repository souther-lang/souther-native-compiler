package souther.bindings;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * The C declarations the build wrote for an FFI to read, which a binding carries beside itself and
 * loads the library through.
 *
 * <p>A generator can put them somewhere and can do nothing else with them. What they say about the
 * library's ABI is the driver's answer, and a generator that read it would be giving a second one,
 * so the only thing it is handed is the copy.
 */
public interface Declarations {

    /** Writes the declarations to {@code target}, which must not exist. */
    void copyTo(Path target) throws IOException;

    /** The declarations the build wrote at {@code file}. */
    static Declarations at(Path file) {
        return target -> Files.copy(file, target);
    }
}
