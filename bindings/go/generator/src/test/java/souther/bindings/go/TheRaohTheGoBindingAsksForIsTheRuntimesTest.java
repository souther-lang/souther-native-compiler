package souther.bindings.go;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A package that imports Raoh has to require it, since Go asks the module that imports a package to
 * name the module the package is from. The generated module names the release the runtime module
 * does: a host that gets both has one Raoh, and not two that moving one of them would make.
 */
class TheRaohTheGoBindingAsksForIsTheRuntimesTest {

    @Test
    void theGeneratedModuleRequiresTheRaohTheRuntimeModuleDoes() throws IOException {
        Matcher asked = Pattern.compile("github.com/raoh-project/raoh-go (\\S+)")
                .matcher(Files.readString(GoHost.RUNTIME.resolve("go.mod"), StandardCharsets.UTF_8));

        assertThat(asked.find()).as("the runtime module asks for Raoh").isTrue();
        assertThat(GoBindings.RAOH_VERSION).isEqualTo(asked.group(1));
    }
}
