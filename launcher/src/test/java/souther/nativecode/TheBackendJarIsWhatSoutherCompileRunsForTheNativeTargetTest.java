package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.Compiler;

import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Properties;
import java.util.jar.JarFile;
import java.util.jar.Manifest;
import java.util.zip.ZipEntry;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * {@code souther compile --target native} runs the backend jar as a process of its own, and chooses
 * it by what the jar's descriptor says and by nothing else: the target it answers to and the Souther
 * it was built against, read as written. A descriptor naming another Souther than the one the jar
 * carries would have the CLI start a backend that compiles in another language than the CLI checks,
 * and nothing between the two would say so. So the version in the descriptor is held here to the
 * Souther compiler this build compiles against, and the jar is run as the CLI runs it, alone, with
 * nothing of this build beside it.
 */
class TheBackendJarIsWhatSoutherCompileRunsForTheNativeTargetTest {

    private static final String DESCRIPTOR = "META-INF/souther/backend.properties";

    private static final String MONEY = """
            module shop.money exposing ( Money, double )

            data Money = Int
                invariant notNegative = value >= 0

            behavior double : (money: Money) -> Int
            let double (money) = money.value * 2
            """;

    private static Path jar() {
        return Path.of(System.getProperty("souther.backend.jar"));
    }

    @Test
    void theDescriptorNamesTheNativeTargetAndTheSoutherTheJarIsBuiltAgainst() throws Exception {
        String souther = Compiler.class.getPackage().getImplementationVersion();
        assertThat(souther).as("the Souther compiler on this build's class path says its version")
                .isNotBlank();

        try (JarFile archive = new JarFile(jar().toFile())) {
            ZipEntry entry = archive.getEntry(DESCRIPTOR);
            assertThat(entry).as(DESCRIPTOR).isNotNull();
            byte[] bytes;
            try (InputStream in = archive.getInputStream(entry)) {
                bytes = in.readAllBytes();
            }
            Properties descriptor = new Properties();
            descriptor.load(new java.io.ByteArrayInputStream(bytes));

            assertThat(descriptor.stringPropertyNames()).containsExactlyInAnyOrder("name",
                    "souther.version");
            assertThat(descriptor.getProperty("name")).isEqualTo("native");
            assertThat(descriptor.getProperty("souther.version")).isEqualTo(souther);
            assertThat(new String(bytes, StandardCharsets.UTF_8)).as("each value as the CLI compares it")
                    .isEqualTo("name=native\nsouther.version=" + souther + "\n");
        }
    }

    /**
     * The backend's own version is the manifest's, and is the compiler's: it is the release a
     * released backend fetches its driver and generators at, which the descriptor plays no part in.
     */
    @Test
    void theManifestStartsTheCommandAndSaysTheBackendsOwnVersion() throws Exception {
        try (JarFile archive = new JarFile(jar().toFile())) {
            Manifest manifest = archive.getManifest();

            assertThat(manifest.getMainAttributes().getValue("Main-Class"))
                    .isEqualTo(Main.class.getName());
            assertThat(manifest.getMainAttributes().getValue("Implementation-Version"))
                    .isEqualTo(System.getProperty("souther.native.version"));
        }
    }

    /**
     * Run as {@code java -jar}, which is how the CLI starts it: the compiler, the Souther it reads
     * the program with, and the API a generator is loaded against all come out of the one jar. A
     * clone's build is not a release, so the driver and the generator's jar are named to it.
     */
    @Test
    void theJarAloneBuildsALibraryAndABindingOfIt(@TempDir Path directory) throws Exception {
        Path sources = Files.createDirectories(directory.resolve("src"));
        Files.writeString(sources.resolve("money.sou"), MONEY);

        List<String> command = new ArrayList<>(List.of(
                Path.of(System.getProperty("java.home"), "bin", "java").toString(),
                "-Dsouther.native.driver=" + System.getProperty("souther.native.driver"),
                "-Dsouther.generator.rust=" + System.getProperty("souther.generator.rust"),
                "-jar", jar().toString(),
                "--library", "out/native", "--rust", "out/rust", "--crate", "shop", "src"));
        Process process = new ProcessBuilder(command).directory(directory.toFile())
                .redirectErrorStream(true).start();
        process.getOutputStream().close();
        String printed = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);

        assertThat(process.waitFor()).as(printed).isZero();
        assertThat(directory.resolve("out/native/souther.json")).exists();
        assertThat(directory.resolve("out/rust/Cargo.toml")).exists();
    }
}
