package souther.bindings.go;

import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A generated {@code go.mod} requires the runtime module at the release the generator is, and the
 * runtime is published at that release by the release process, with the module's path in the tag. A
 * build that is no release requires what Go writes for a module put in place by a {@code replace}.
 */
class TheRuntimeAPackageRequiresIsTheReleaseItWasWrittenByTest {

    @Test
    void aReleaseRequiresItself() {
        assertThat(GoBindings.runtimeVersion("0.1.0")).isEqualTo("v0.1.0");
        assertThat(GoBindings.runtimeVersion("1.4.2-rc.1")).isEqualTo("v1.4.2-rc.1");
    }

    @Test
    void whatIsNoReleaseRequiresWhatAReplaceIsGiven() {
        assertThat(GoBindings.runtimeVersion(null)).isEqualTo(GoBindings.UNRELEASED);
        assertThat(GoBindings.runtimeVersion("")).isEqualTo(GoBindings.UNRELEASED);
        assertThat(GoBindings.runtimeVersion("0.1.0-SNAPSHOT")).isEqualTo(GoBindings.UNRELEASED);
    }

    @Test
    void theTestsRunFromClassesAndSoRequireWhatAReplaceIsGiven() {
        assertThat(GoBindings.runtimeVersion()).isEqualTo(GoBindings.UNRELEASED);
    }
}
