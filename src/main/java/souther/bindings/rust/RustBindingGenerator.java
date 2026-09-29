package souther.bindings.rust;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.Generated;
import souther.bindings.NotBindable;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;

/** The Rust binding, as the command finds it: asked for the {@code crate} it is written as. */
public final class RustBindingGenerator implements BindingGenerator {

    @Override
    public String id() {
        return "rust";
    }

    @Override
    public void preflight(Path into, Map<String, String> options) throws IOException {
        RustBindings.refuseAhead(into, crate(options));
    }

    @Override
    public Generated generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        return RustBindings.generate(input, into, crate(options));
    }

    private static String crate(Map<String, String> options) {
        String crate = options.get("crate");
        if (crate == null) {
            throw new NotBindable("--rust wants --crate, the name of the crate the binding is");
        }
        return crate;
    }
}
