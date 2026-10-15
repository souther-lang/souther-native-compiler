package souther.bindings.rust;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A crate the binding writes depends on the runtime crate by a version, and asks for the oldest Rust
 * it builds with. Both are facts of the runtime crate, written again in the generator, so the two are
 * held together here: a runtime released at a version the binding does not ask for, or one that
 * moves to a newer Rust (as it did with Raoh 0.9), would otherwise leave every generated crate asking
 * for what does not build it.
 */
class TheCrateTheRustBindingWritesAsksForWhatTheRuntimeIsTest {

    private static String runtime(String key) throws IOException {
        Path toml = Path.of(System.getProperty("souther.repository"), "bindings", "rust", "runtime",
                "Cargo.toml");
        Matcher said = Pattern.compile("(?m)^" + Pattern.quote(key) + " = \"([^\"]+)\"$")
                .matcher(Files.readString(toml));
        assertThat(said.find()).as("the runtime crate says its %s", key).isTrue();
        return said.group(1);
    }

    @Test
    void itAsksForTheRustTheRuntimeAsksFor() throws IOException {
        assertThat(RustBindings.RUST_VERSION).isEqualTo(runtime("rust-version"));
    }

    /** A caret requirement: the runtime's version is the one asked for, or a later one Cargo takes. */
    @Test
    void theRuntimeItDependsOnIsTheOneTheCrateIs() throws IOException {
        String version = runtime("version");
        String[] asked = RustBindings.RUNTIME_VERSION.split("\\.");
        String[] is = version.split("\\.");

        assertThat(is[0]).isEqualTo(asked[0]);
        if (asked[0].equals("0")) {
            assertThat(is[1]).as("a 0.x crate is compatible only within its minor").isEqualTo(asked[1]);
        }
    }
}
