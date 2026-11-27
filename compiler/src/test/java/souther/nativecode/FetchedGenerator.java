package souther.nativecode;

import org.jspecify.annotations.Nullable;
import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.NotBindable;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;

/**
 * A generator that a test packs into a jar and serves as the artifact of the PHP binding: it is
 * what is loaded once the jar is fetched, and it writes one file so that a build can be seen to have
 * used it. It carries a JSpecify annotation as the standard generators do, which its loader cannot
 * resolve and does not need to.
 *
 * <p>Asked for the namespace {@value #STOPPING}, it refuses with {@value #ASKED} instead: a test
 * that asks only which jar the command came to run sees this one answer, and the command ends
 * there, before a driver is fetched or a library built.
 */
public final class FetchedGenerator implements BindingGenerator {

    /** The namespace it refuses, so that a command ends once this has been asked. */
    public static final String STOPPING = "Stopping";

    /** What it refuses that namespace with. */
    public static final String ASKED = "a fetched generator was asked, and stops the command here";

    private @Nullable String written;

    @Override
    public void preflight(Map<String, String> options) {
        if (STOPPING.equals(options.get("namespace"))) {
            throw new NotBindable(ASKED);
        }
    }

    @Override
    public void generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        written = "written by a fetched generator";
        Files.writeString(into.resolve("fetched.txt"), written);
    }
}
