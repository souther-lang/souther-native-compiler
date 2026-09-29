package souther.nativecode;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Builds a library from a transport document written by hand, for a test in another package that
 * asks what a binding makes of what no checked program of today's language writes.
 */
public final class Documents {

    /**
     * The document the driver's own tests of function values are held to, which no source writes
     * whole: `m` publishing function values of every shape a host is handed, among them one that
     * answers a function and one that makes an optional, which the checker refuses to write.
     */
    public static final Path FUNCTIONS =
            Repository.file("native", "crates", "compiler", "tests", "functions.transport.json");

    private Documents() {
    }

    /** The library {@code document} is built into, in {@code into}. */
    public static NativeCompiler.Library library(Path document, Path into)
            throws IOException, InterruptedException {
        return NativeCompiler.library(Files.readString(document, StandardCharsets.UTF_8), into);
    }
}
