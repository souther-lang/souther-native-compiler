package souther.bindings.go;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The runtime module's version is its own, in the file beside its {@code go.mod}, and the generator
 * takes both from there and states neither: a compiler release that leaves the runtime as it was
 * requires the version already published, and the compiler's version is no part of it.
 *
 * <p>A module has to say its major version in its path from version 2 on and not before, and a tag
 * that does not is one Go refuses after it has been published, when nothing can be done about it, so
 * the two are held to one another here, in every build.
 */
class TheRuntimeModuleIsVersionedByItselfTest {

    private static final Pattern SEMVER =
            Pattern.compile("(\\d+)\\.(\\d+)\\.(\\d+)(-[0-9A-Za-z.-]+)?");

    @Test
    void theGeneratorStatesWhatTheModuleStatesOfItself() throws IOException {
        String version = Files.readString(GoHost.RUNTIME.resolve("VERSION"), StandardCharsets.UTF_8).strip();
        Matcher module = Pattern.compile("(?m)^module\\s+(\\S+)\\s*$")
                .matcher(Files.readString(GoHost.RUNTIME.resolve("go.mod"), StandardCharsets.UTF_8));

        assertThat(module.find()).isTrue();
        assertThat(RuntimeModule.THE.version()).isEqualTo(version);
        assertThat(RuntimeModule.THE.path()).isEqualTo(module.group(1));
        assertThat(RuntimeModule.THE.requirement()).isEqualTo("v" + version);
    }

    @Test
    void theVersionIsOneAndItsMajorIsInThePathFromTwoOn() {
        Matcher semver = SEMVER.matcher(RuntimeModule.THE.version());
        assertThat(semver.matches()).as(RuntimeModule.THE.version()).isTrue();

        int major = Integer.parseInt(semver.group(1));
        Matcher suffix = Pattern.compile("/v(\\d+)$").matcher(RuntimeModule.THE.path());
        if (major >= 2) {
            assertThat(suffix.find()).as("a module at version %d has /v%d in its path", major, major)
                    .isTrue();
            assertThat(Integer.parseInt(suffix.group(1))).isEqualTo(major);
        } else {
            assertThat(suffix.find()).as("a module before version 2 has no /vN in its path").isFalse();
        }
    }
}
