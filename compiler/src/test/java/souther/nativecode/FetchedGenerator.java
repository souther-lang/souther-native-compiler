package souther.nativecode;

import org.jspecify.annotations.Nullable;
import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;

/**
 * A generator that a test packs into a jar and serves as the artifact of the PHP binding: it is
 * what is loaded once the jar is fetched, and it writes one file so that a build can be seen to have
 * used it. It carries a JSpecify annotation as the standard generators do, which its loader cannot
 * resolve and does not need to.
 */
public final class FetchedGenerator implements BindingGenerator {

    private @Nullable String written;

    @Override
    public void preflight(Map<String, String> options) {
    }

    @Override
    public void generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        written = "written by a fetched generator";
        Files.writeString(into.resolve("fetched.txt"), written);
    }
}
