package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.TreeMap;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatCode;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * What a released compiler fetches, it fetches held to a checksum that the release wrote into the
 * compiler, so a compiler that carries none for something it fetches is a release that cannot be
 * used, and is found where it is built and not by the first user who needs the driver.
 *
 * <p>The file cannot be committed, since it is made from builds that follow the commit, so a clone
 * rebuilding a release version has none. That is refused; a snapshot is not a release, and is not held
 * to it.
 */
class AReleaseCarriesTheChecksumOfEverythingItFetchesTest {

    private static final String SUM = "ab".repeat(32);

    @Test
    void everyPlatformsBundleAndEveryGeneratorTheCatalogNamesIsExpected() {
        assertThat(ReleaseChecksums.expected()).contains("native.linux-x86_64",
                "native.linux-aarch64", "native.macos-x86_64", "native.macos-aarch64",
                "generator.php", "generator.rust", "generator.go").hasSize(7);
        assertThat(ReleaseChecksums.expected())
                .as("the keys the fetching reads are the ones a release is held to")
                .contains(ReleaseChecksums.bundle("linux-x86_64"), ReleaseChecksums.generator("php"));
    }

    @Test
    void aSnapshotIsNotHeldToIt(@TempDir Path into) {
        assertThatCode(() -> ReleaseChecksums.check("1.2.3-SNAPSHOT", into.resolve("none")))
                .doesNotThrowAnyException();
    }

    @Test
    void aReleaseWithNoFileCarriesNoChecksums(@TempDir Path into) {
        assertThatThrownBy(() -> ReleaseChecksums.check("1.2.3", into.resolve("none")))
                .isInstanceOf(IOException.class).hasMessageContaining("carries no checksums");
    }

    @Test
    void aReleaseWithAllOfThemIsAccepted(@TempDir Path into) throws IOException {
        Path file = written(into, complete());

        assertThatCode(() -> ReleaseChecksums.check("1.2.3", file)).doesNotThrowAnyException();
    }

    @Test
    void aReleaseLackingOneOfThemIsRefusedByName(@TempDir Path into) throws IOException {
        Map<String, String> lacking = complete();
        lacking.remove("generator.rust");
        Path file = written(into, lacking);

        assertThatThrownBy(() -> ReleaseChecksums.check("1.2.3", file))
                .isInstanceOf(IOException.class).hasMessageContaining("lacking [generator.rust]");
    }

    @Test
    void aReleaseCarryingOneItIsNotToIsRefused(@TempDir Path into) throws IOException {
        Map<String, String> extra = complete();
        extra.put("generator.python", SUM);
        Path file = written(into, extra);

        assertThatThrownBy(() -> ReleaseChecksums.check("1.2.3", file))
                .isInstanceOf(IOException.class)
                .hasMessageContaining("unexpected [generator.python]");
    }

    @Test
    void aValueThatIsNotASha256IsRefused(@TempDir Path into) throws IOException {
        Map<String, String> malformed = complete();
        malformed.put("native.macos-aarch64", "not a checksum");
        Path file = written(into, malformed);

        assertThatThrownBy(() -> ReleaseChecksums.check("1.2.3", file))
                .isInstanceOf(IOException.class)
                .hasMessageContaining("not a SHA-256 [native.macos-aarch64]");
    }

    private static Map<String, String> complete() {
        Map<String, String> all = new TreeMap<>();
        ReleaseChecksums.expected().forEach(key -> all.put(key, SUM));
        return all;
    }

    private static Path written(Path into, Map<String, String> checksums) throws IOException {
        StringBuilder text = new StringBuilder();
        checksums.forEach((key, sum) -> text.append(key).append('=').append(sum).append('\n'));
        return Files.writeString(into.resolve("release-checksums.properties"), text);
    }
}
