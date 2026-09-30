package souther.bindings.rust;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.NotBindable;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;

/** The Rust binding, as the command finds it: asked for the {@code crate} it is written as. */
public final class RustBindingGenerator implements BindingGenerator {

    @Override
    public void preflight(Map<String, String> options) {
        RustBindings.refuseAhead(crate(options));
    }

    @Override
    public void generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        RustBindings.generate(input, into, crate(options));
    }

    private static String crate(Map<String, String> options) {
        for (String key : options.keySet()) {
            if (!key.equals("crate")) {
                throw new NotBindable("the Rust binding takes no option \"" + key + "\"; it takes"
                        + " crate");
            }
        }
        String crate = options.get("crate");
        if (crate == null) {
            throw new NotBindable("--rust wants --crate, the name of the crate the binding is");
        }
        return crate;
    }
}
