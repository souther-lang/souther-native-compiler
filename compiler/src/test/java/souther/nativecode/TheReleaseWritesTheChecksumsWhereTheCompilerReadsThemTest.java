package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The file of checksums a release writes is at one path in the compiler's artifact, and that path is
 * named where the release writes it, where the build checks it, and where the repository is told not
 * to commit it. Nothing runs the release before it is needed, so a path that parted from the others
 * would be found as a compiler that carries none.
 */
class TheReleaseWritesTheChecksumsWhereTheCompilerReadsThemTest {

    private static final String RESOURCE = "src/main/resources/" + ReleaseChecksums.RESOURCE;

    @Test
    void theReleaseWorkflowWritesItThere() throws IOException {
        assertThat(Files.readString(Repository.file(".github", "workflows", "release.yml")))
                .contains("compiler/" + RESOURCE);
    }

    @Test
    void theBuildChecksItThere() throws IOException {
        assertThat(Files.readString(Repository.file("compiler", "pom.xml")))
                .contains("${project.build.outputDirectory}/" + ReleaseChecksums.RESOURCE);
    }

    @Test
    void theRepositoryDoesNotCommitIt() throws IOException {
        assertThat(Files.readAllLines(Repository.file(".gitignore"))).contains("compiler/" + RESOURCE);
    }
}
