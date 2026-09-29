package souther.bindings.php;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.Generated;
import souther.bindings.NotBindable;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;

/** The PHP binding, as the command finds it: asked for a {@code namespace} to write it under. */
public final class PhpBindingGenerator implements BindingGenerator {

    @Override
    public String id() {
        return "php";
    }

    @Override
    public void preflight(Path into, Map<String, String> options) throws IOException {
        PhpBindings.refuseAhead(into, namespace(options));
    }

    @Override
    public Generated generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        return PhpBindings.generate(input, into, namespace(options));
    }

    private static String namespace(Map<String, String> options) {
        String namespace = options.get("namespace");
        if (namespace == null) {
            throw new NotBindable("--php wants --namespace, the namespace the binding is under");
        }
        return namespace;
    }
}
