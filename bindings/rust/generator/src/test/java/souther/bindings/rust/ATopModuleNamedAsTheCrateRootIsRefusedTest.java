package souther.bindings.rust;

import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.NotBindable;



import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A top module of the model is a module at the root of the crate, beside what the generator
 * declares there. A model whose top module has one of those names is refused by the generator, and
 * never becomes a crate rustc refuses.
 */
class ATopModuleNamedAsTheCrateRootIsRefusedTest {

    @Test
    void aTopModuleNamedAsTheDecodingARunLendsIsRefused(@TempDir Path into) throws Exception {
        TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), """
                module Decoding exposing ( Quantity )

                data Quantity = Int
                """);

        assertThatThrownBy(() -> RustHost.generated(library, into.resolve("binding"), "shop-binding"))
                .isInstanceOf(NotBindable.class)
                .hasMessageContaining("the generated `Decoding`");
    }
}
