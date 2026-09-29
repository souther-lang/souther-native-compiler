package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every rust-cache step in a workflow is keyed on every file Cargo reads its settings from.
 *
 * <p>rust-cache keys on the manifests of a workspace's packages and its lock, and not on a virtual
 * workspace's root or a Cargo config, and a key that matches in full is not saved again. A profile
 * set in {@code native/Cargo.toml} was therefore rebuilt on every CI run from a cache that never
 * held it (#133), with every check green. A step added later without the key would do the same,
 * and nothing but the time a run takes would say so.
 */
class EveryRustCacheIsKeptUnderWhatDecidesTheBuildTest {

    private static final String KEY =
            "key: ${{ hashFiles('**/Cargo.toml', '**/.cargo/config.toml', '**/.cargo/config') }}";

    /** A step using rust-cache, and what it is written with up to the next step. */
    private static final Pattern STEP = Pattern.compile(
            "^([ ]*)- uses: Swatinem/rust-cache@\\S+[ ]*\\n((?:\\1  .*\\n|[ ]*\\n)*)",
            Pattern.MULTILINE);

    @Test
    void everyRustCacheStepIsKeyedOnEveryCargoSetting() throws IOException {
        List<String> unkeyed = new ArrayList<>();
        int steps = 0;
        try (Stream<Path> workflows = Files.list(Repository.file(".github", "workflows"))) {
            for (Path workflow : workflows.filter(it -> it.toString().endsWith(".yml")).toList()) {
                Matcher step = STEP.matcher(Files.readString(workflow));
                while (step.find()) {
                    steps++;
                    if (!step.group(2).contains(KEY)) {
                        unkeyed.add(workflow.getFileName() + ": " + step.group().strip());
                    }
                }
            }
        }
        assertThat(steps).as("rust-cache steps found").isPositive();
        assertThat(unkeyed).as("rust-cache steps not keyed on every Cargo setting").isEmpty();
    }
}
