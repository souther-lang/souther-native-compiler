package acceptance;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.NotBindable;
import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** The generator tested as its author would, with the API and the testkit of a release and nothing else. */
class ModuleListGeneratorTest {

    @Test
    void theModulesOfALibraryAreWritten(@TempDir Path into) throws Exception {
        TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), """
                module shop.money exposing ( Money )

                data Money = Int
                    invariant notNegative = value >= 0
                """, """
                module shop.lines exposing ( Line, total )
                import shop.money ( Money )

                data Line = { price: Money, quantity: Int }

                behavior total : (line: Line) -> Int
                let total (line) = line.price.value * line.quantity
                """);
        Path binding = Files.createDirectories(into.resolve("binding"));

        new ModuleListGenerator().generate(library.bindingInput(), binding, Map.of());

        assertEquals("shop.lines\nshop.money\n",
                Files.readString(binding.resolve("modules.txt")).lines().sorted()
                        .reduce("", (all, line) -> all + line + "\n"));
        assertTrue(Files.size(binding.resolve("library.h")) > 0);
        assertTrue(Files.isRegularFile(library.library()));
        assertTrue(Files.isRegularFile(library.header()));
    }

    @Test
    void anOptionItDoesNotTakeIsRefused() {
        assertThrows(NotBindable.class,
                () -> new ModuleListGenerator().preflight(Map.of("unknown", "1")));
    }
}
