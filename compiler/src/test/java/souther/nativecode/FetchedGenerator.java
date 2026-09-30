package souther.nativecode;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.Set;

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
    public Set<String> options() {
        return Set.of("namespace");
    }

    @Override
    public void preflight(Map<String, String> options) {
    }

    @Override
    public void generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        Files.writeString(into.resolve("fetched.txt"), "written by a fetched generator");
    }
}
