package souther.bindings.php;

import org.junit.jupiter.api.Test;
import souther.nativecode.Php;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What the runtime package offers a generated binding is what the protocol it speaks says it is.
 *
 * <p>A binding refuses to load over a runtime of another protocol, and that refusal is worth only
 * what the number is: a runtime that gains {@code Session::decimal} and keeps the number lets a
 * binding written against it load over one without, and the call fails later, in the middle of a
 * run. So the runtime's public surface — every class, and every public constant, property and
 * method — is recorded under the protocol that has it, and the surface this build has is held to
 * the record for {@code Binding::PROTOCOL}. A change to it fails here until the protocol moves (a
 * line in {@code Binding::MOVES}) and the new surface is recorded under the new number; the
 * records of versions before it stay as they were.
 *
 * <p>The listing is of what the surface means and not of how a PHP's Reflection spells it
 * ({@code php-runtime-surface.php} says how), because the record is checked on every PHP the
 * package runs on, and the spelling is not the same on all of them: one names the class {@code
 * self} stands for and another writes {@code self}. {@link #aTypeIsListedAsWhatItMeans} holds
 * that.
 */
class TheRuntimesSurfaceMovesWithItsProtocolTest {

    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    private static final Path RESOURCES =
            Path.of("src", "test", "resources", "souther", "bindings");

    /** The listing of a runtime directory, or of the classes a file declares. */
    private static final Path LISTING = RESOURCES.resolve("php-runtime-surface.php");

    /** Where each protocol's surface is recorded, one file for each, named by the number. */
    private static final Path RECORDED = RESOURCES.resolve("php-runtime-surface");

    @Test
    void theSurfaceIsTheOneItsProtocolRecorded() throws Exception {
        String protocol = Php.ran(List.of("-r", "require '"
                + RUNTIME.resolve("src").resolve("Binding.php").toAbsolutePath()
                + "'; echo \\Souther\\Runtime\\Binding::PROTOCOL;"));
        String surface = Php.ran(List.of(LISTING.toAbsolutePath().toString(), "surface",
                RUNTIME.toAbsolutePath().toString()));
        Path record = RECORDED.resolve(protocol + ".txt");
        String recorded =
                Files.exists(record) ? Files.readString(record, StandardCharsets.UTF_8) : "";
        if (!surface.equals(recorded)) {
            // Kept where it can be compared with the record, and copied over it once it is read.
            Files.writeString(Path.of("target", "php-runtime-surface-" + protocol + ".txt"),
                    surface, StandardCharsets.UTF_8);
        }

        assertThat(surface)
                .as("the runtime's surface is not the one protocol %s recorded: a binding written"
                        + " against one of the two would load over the other. Move the protocol"
                        + " (Binding::MOVES, Binding::PROTOCOL, PhpBindings.RUNTIME_PROTOCOL) and"
                        + " record target/php-runtime-surface-<n>.txt under the new number,"
                        + " leaving the records before it as they are", protocol)
                .isEqualTo(recorded);
    }

    /**
     * Two spellings of one type are one line: {@code self} and the class it stands for, {@code
     * parent} and the parent class, {@code ?T} and {@code T|null}, a union or an intersection in
     * any order. A listing that read them apart would call a PHP's way of writing a type a change
     * of the surface, and fail on the runner whose PHP writes it the other way.
     */
    @Test
    void aTypeIsListedAsWhatItMeans() throws Exception {
        String listed = Php.ran(List.of(LISTING.toAbsolutePath().toString(), "types",
                RESOURCES.resolve("php-surface-types.php").toAbsolutePath().toString()));

        // One declaring class each, so the class names are all that tells the two apart.
        List<String> lines = listed.lines()
                .map(it -> it.replaceAll("\\b(Fixture|Explicit)\\b", "C")).toList();
        int half = lines.size() / 2;
        assertThat(lines).hasSize(half * 2);
        assertThat(lines.subList(0, half)).isEqualTo(lines.subList(half, lines.size()));
        assertThat(String.join("\n", lines))
                .contains("C::of(C $a, C|null $b, Base $c, int|null|string $d, null|string $e): C")
                .contains("C::later(): static")
                .contains("C::both(Countable&Traversable $x): int|string");
    }
}
