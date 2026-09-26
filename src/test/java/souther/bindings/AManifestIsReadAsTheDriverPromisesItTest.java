package souther.bindings;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;
import tools.jackson.databind.node.ArrayNode;
import tools.jackson.databind.node.ObjectNode;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.function.Consumer;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * What a manifest promises a generator, whichever host it writes for, held where it is read: a
 * manifest breaking a promise is refused as one this does not read, before any generator takes it
 * apart, and is not a model a host cannot be given.
 */
class AManifestIsReadAsTheDriverPromisesItTest {

    /** Two modules each holding a list of values of a type of its own. */
    private static final String TWO_MODULES_OF_LISTS = """
            module shop exposing ( Item, Cart )

            data Item = { n: Int }
            data Cart = { items: List<Item> }
            """;

    private static final String SECOND_MODULE_OF_LISTS = """
            module stock exposing ( Part, Bin )

            data Part = { n: Int }
            data Bin = { parts: List<Part> }
            """;

    /** A published behavior depending on one its module keeps, which depends on one a host implements. */
    private static final String CONSTRUCTED = """
            module demo exposing ( Price, outer )

            data Price = Int

            behavior listed : () -> Price

            behavior adjusted : () -> Price
                depends on listed
            let adjusted (listed) = listed()

            behavior outer : () -> Price
                depends on adjusted
            let outer (adjusted) = adjusted()
            """;

    private static final JsonMapper JSON = JsonMapper.builder().build();

    private static NativeCompiler.Library built(Path into, String... sources) throws Exception {
        return NativeCompiler.library(CheckedProgram.of(List.of(sources)), into.resolve("native"));
    }

    /** Reads the library's manifest after {@code changing} the module named {@code module}. */
    private static Manifest readAfter(Path into, NativeCompiler.Library library, String module,
                                      Consumer<ObjectNode> changing) throws Exception {
        JsonNode manifest = JSON.readTree(library.manifest().toFile());
        for (JsonNode it : manifest.get("modules")) {
            if (it.get("name").stringValue().equals(module)) {
                changing.accept((ObjectNode) it);
            }
        }
        Path changed = into.resolve("changed.json");
        Files.writeString(changed, JSON.writeValueAsString(manifest), StandardCharsets.UTF_8);
        return Manifest.read(changed);
    }

    /** What the driver writes is read, the runtime's functions among it, which a host calls too. */
    @Test
    void theManifestTheDriverWroteIsRead(@TempDir Path into) throws Exception {
        Manifest read = Manifest.read(built(into, CONSTRUCTED).manifest());

        assertThat(read.abi()).isEqualTo(Manifest.ABI);
        assertThat(read.runtime()).extracting(Manifest.Function::name).contains("souther_reset");
        assertThat(read.modules()).extracting(Manifest.Module::name).containsExactly("demo");
    }

    /**
     * A manifest of a version this was not written for is refused as that, and not as whichever
     * member moved since: version 3, as the driver wrote it, said what a behavior answers as a
     * type.
     */
    @Test
    void aManifestOfAnotherVersionIsRefusedByItsVersion() {
        Path earlier = Path.of("src", "test", "resources", "souther", "bindings",
                "interface-v3.json");

        assertThatThrownBy(() -> Manifest.read(earlier))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("is version 3 of souther-native-interface for ABI generation"
                        + " 3, and this generator reads version 8")
                .hasMessageNotContaining("answers");
    }

    /**
     * What constructing a behavior requires is closed: a construction naming what nothing in the
     * manifest constructs or asks a host to implement is one a host would find out about at a call,
     * as something no binding can build.
     */
    @Test
    void aConstructionRequiringWhatNothingConstructsIsRefused(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library = built(into, CONSTRUCTED);

        assertThatThrownBy(() -> readAfter(into, library, "demo",
                module -> ((ArrayNode) module.get("injections")).removeAll()))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("requires demo.listed, which nothing in it constructs");
    }

    /**
     * Every list a module says is held to what a list of its element is built and read through,
     * whether or not a generator goes on to use it.
     */
    @Test
    void aListAModuleSaysOtherThanAListIsRefused(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = built(into, TWO_MODULES_OF_LISTS, SECOND_MODULE_OF_LISTS);

        assertThatThrownBy(() -> readAfter(into, library, "stock", module -> {
            ObjectNode at = (ObjectNode) module.get("lists").get(0).get("at");
            ((ArrayNode) at.get("takes")).remove(2);
        }))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("souther4_m_stock_l_value_at");
    }

    /** One element twice in a module is two things said of one list. */
    @Test
    void aListOfOneElementSaidTwiceIsRefused(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = built(into, TWO_MODULES_OF_LISTS, SECOND_MODULE_OF_LISTS);

        assertThatThrownBy(() -> readAfter(into, library, "shop", module -> {
            ArrayNode lists = (ArrayNode) module.get("lists");
            lists.add(lists.get(0).deepCopy());
        }))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("module `shop` two lists of");
    }
}
