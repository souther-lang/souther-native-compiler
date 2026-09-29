package souther.nativecode;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.Generated;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/**
 * A generator that a test packs into a jar and serves as the artifact of the PHP binding: it is
 * what is loaded once the jar is fetched, and it writes one file so that a build can be seen to have
 * used it.
 */
public final class FetchedGenerator implements BindingGenerator {

    @Override
    public String id() {
        return "php";
    }

    @Override
    public void preflight(Path into, Map<String, String> options) {
    }

    @Override
    public Generated generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        Files.createDirectories(into);
        Path written = Files.writeString(into.resolve("fetched.txt"), "written by a fetched generator");
        return new Generated(into, List.of(written));
    }
}
