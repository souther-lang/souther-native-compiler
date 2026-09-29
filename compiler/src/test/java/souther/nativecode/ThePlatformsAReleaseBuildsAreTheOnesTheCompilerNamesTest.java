package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The platforms a bundle is published for are written three times: by the compiler, which picks the
 * bundle for the platform it runs on; by the release workflow, which builds one for each; and by the
 * script that packs it. Nothing runs the release workflow before a release, so a platform named in
 * one and not another would be found when a user on it is told there is no driver for them.
 */
class ThePlatformsAReleaseBuildsAreTheOnesTheCompilerNamesTest {

    private static final Pattern MATRIX = Pattern.compile("^\\s*- platform: (\\S+)\\s*$",
            Pattern.MULTILINE);
    private static final Pattern ACCEPTED = Pattern.compile(
            "^\\s*(linux-[a-z0-9_]+ \\| .*?)\\) ;;\\s*$", Pattern.MULTILINE);

    @Test
    void theReleaseWorkflowBuildsABundleForEachOfThem() throws IOException {
        String workflow = Files.readString(Repository.file(".github", "workflows", "release.yml"));
        Set<String> built = new TreeSet<>();
        Matcher platform = MATRIX.matcher(workflow);
        while (platform.find()) {
            built.add(platform.group(1));
        }

        assertThat(built).isEqualTo(new TreeSet<>(NativeBundle.PLATFORMS));
    }

    @Test
    void thePackingScriptAcceptsEachOfThem() throws IOException {
        String script = Files.readString(Repository.file("scripts", "package-native-bundle.sh"));
        Matcher accepted = ACCEPTED.matcher(script);

        assertThat(accepted.find()).as("the case that names the platforms").isTrue();
        List<String> named = List.of(accepted.group(1).split("\\s*\\|\\s*"));
        assertThat(new TreeSet<>(named)).isEqualTo(new TreeSet<>(NativeBundle.PLATFORMS));
    }

    @Test
    void theChecksumScriptReadsEachOfThem() throws IOException {
        String script = Files.readString(Repository.file("scripts", "record-bundle-checksums.sh"));

        for (String platform : NativeBundle.PLATFORMS) {
            String[] parts = platform.split("-");
            assertThat(script).as("the pattern that reads %s", platform)
                    .contains(parts[0]).contains(parts[1]);
        }
        assertThat(script).contains("-ne " + NativeBundle.PLATFORMS.size());
    }

    @Test
    void theNameTheCompilerGivesThePlatformItRunsOnIsOneOfThem() throws Exception {
        assertThat(NativeBundle.PLATFORMS).contains(NativeBundle.platform("Mac OS X", "aarch64"),
                NativeBundle.platform("Mac OS X", "x86_64"), NativeBundle.platform("Linux", "amd64"),
                NativeBundle.platform("Linux", "aarch64"));
    }
}
