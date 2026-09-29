package souther.bindings.go;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * The runtime module the packages are written against, as the module says it of itself: its path
 * from its own {@code go.mod}, and its version from the {@code VERSION} beside it. Both are copied
 * into this generator's jar when it is built, so they are stated once, in the module, and nothing
 * here restates them.
 *
 * <p>The version is the runtime's own and moves when the runtime does, and not when the compiler
 * does: a compiler release that leaves the runtime as it was requires the version already
 * published. That the two are not one number is not a matter of taste: a Go module at version 2 or
 * later has {@code /v2} in its path, which a compiler's major version has no reason to put there.
 *
 * @param path    the module's path, which every import of the runtime is written with
 * @param version the module's version, without the {@code v}
 */
record RuntimeModule(String path, String version) {

    private static final String RESOURCES = "/souther/bindings/go/runtime/";

    /** The runtime module this generator was built with. */
    static final RuntimeModule THE = read();

    /** What a {@code go.mod} requires this at. */
    String requirement() {
        return "v" + version;
    }

    private static RuntimeModule read() {
        String mod = text("go.mod");
        Matcher path = Pattern.compile("(?m)^module\\s+(\\S+)\\s*$").matcher(mod);
        if (!path.find()) {
            throw new IllegalStateException("the runtime module's go.mod names no module");
        }
        return new RuntimeModule(path.group(1), text("VERSION").strip());
    }

    private static String text(String name) {
        try (InputStream in = RuntimeModule.class.getResourceAsStream(RESOURCES + name)) {
            if (in == null) {
                throw new IllegalStateException("the runtime module's " + name
                        + " was not built into this generator");
            }
            return new String(in.readAllBytes(), StandardCharsets.UTF_8);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }
}
