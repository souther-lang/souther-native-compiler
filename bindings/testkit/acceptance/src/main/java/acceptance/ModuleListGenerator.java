package acceptance;

import souther.bindings.BindingGenerator;
import souther.bindings.BindingInput;
import souther.bindings.Manifest;
import souther.bindings.NotBindable;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.stream.Collectors;

/** A generator of someone else's: writes the name of each module of a library, one to a line. */
public final class ModuleListGenerator implements BindingGenerator {

    @Override
    public void preflight(Map<String, String> options) {
        for (String key : options.keySet()) {
            throw new NotBindable("this binding takes no option \"" + key + "\"");
        }
    }

    @Override
    public void generate(BindingInput input, Path into, Map<String, String> options)
            throws IOException {
        Files.writeString(into.resolve("modules.txt"), input.manifest().modules().stream()
                .map(Manifest.Module::name).collect(Collectors.joining("\n", "", "\n")));
        input.declarations().copyTo(into.resolve("library.h"));
    }
}
