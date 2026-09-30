package souther.bindings.testkit;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * The testkit builds a library from sources into the directory it is told, and hands over what a
 * generator is given with the library and its header beside it; sources the language refuses are
 * said as that.
 */
class ALibraryIsBuiltForAGeneratorTest {

    private static final String MONEY = """
            module shop.money exposing ( Money, cents )

            data Money = Int
                invariant notNegative = value >= 0

            let cents: Int = 100
            """;

    @Test
    void aLibraryIsBuiltWhereTheTestSays(@TempDir Path into) throws Exception {
        TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), MONEY);

        assertThat(library.library()).startsWith(into.resolve("native")).exists();
        assertThat(library.header()).startsWith(into.resolve("native")).exists();
        assertThat(library.bindingInput().manifest().modules())
                .anyMatch(module -> module.name().equals("shop.money"));
        library.bindingInput().declarations().copyTo(into.resolve("declarations.h"));
        assertThat(into.resolve("declarations.h")).isNotEmptyFile();
    }

    @Test
    void sourcesTheLanguageRefusesAreSaidAsThat(@TempDir Path into) {
        assertThatThrownBy(() -> SoutherBindingTest.compile(into.resolve("native"), """
                module shop exposing ( missing )
                """)).isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("not a program the language takes");
        assertThatThrownBy(() -> SoutherBindingTest.compile(into.resolve("native")))
                .isInstanceOf(IllegalArgumentException.class);
    }
}
