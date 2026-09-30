package souther.bindings.go;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.NotBindable;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;
import java.util.Set;

/** The Go binding, as the command finds it: asked for the {@code package} it is written as. */
public final class GoBindingGenerator implements BindingGenerator {

    @Override
    public String id() {
        return "go";
    }

    @Override
    public Set<String> options() {
        return Set.of("package");
    }

    @Override
    public void preflight(Map<String, String> options) {
        GoBindings.refuseAhead(importPath(options));
    }

    @Override
    public void generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        GoBindings.generate(input, into, importPath(options));
    }

    private static String importPath(Map<String, String> options) {
        String path = options.get("package");
        if (path == null) {
            throw new NotBindable("--go wants --package, the import path of the package the binding"
                    + " is");
        }
        return path;
    }
}
