package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Whatever links the runtime's static archive reads what it needs linked after it.
 *
 * <p>The archive carries Rust's standard library, and which system libraries that needs is the
 * target's and the toolchain's to say: the runtime's build asks {@code rustc} and writes the answer
 * beside the archive as {@code libsouther_native_runtime.link}. An executable linked with the
 * archive's path alone works where the linker adds those libraries itself and fails on the first
 * Linux that does not, with undefined references to the {@code libm} a big-integer crate reached
 * for. So every test here that names the archive names that file too, and this holds them to it,
 * as the Rust half's {@code tests/linking.rs} holds its own.
 */
class TheJavaTestsLinkTheArchiveThroughItsRequirementsTest {

    private static final Path TESTS = Path.of("src", "test", "java");

    @Test
    void everyTestThatLinksTheArchiveReadsWhatItNeeds() throws IOException {
        List<String> alone = new ArrayList<>();
        int linking = 0;
        try (Stream<Path> files = Files.walk(TESTS)) {
            for (Path file : files.filter(it -> it.toString().endsWith(".java")).toList()) {
                if (file.getFileName().toString().equals(
                        TheJavaTestsLinkTheArchiveThroughItsRequirementsTest.class.getSimpleName()
                                + ".java")) {
                    continue;
                }
                String text = Files.readString(file, StandardCharsets.UTF_8);
                if (text.contains("libsouther_native_runtime.a")) {
                    linking++;
                    if (!text.contains("libsouther_native_runtime.link")) {
                        alone.add(file.toString());
                    }
                }
            }
        }

        assertThat(linking).as("a test links the archive: this looks in the wrong place")
                .isPositive();
        assertThat(alone)
                .as("link the runtime's archive and do not read the requirements written beside it")
                .isEmpty();
    }
}
