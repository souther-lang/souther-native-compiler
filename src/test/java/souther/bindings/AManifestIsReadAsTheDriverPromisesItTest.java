package souther.bindings;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Documents;
import souther.nativecode.NativeCompiler;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;
import tools.jackson.databind.node.ArrayNode;
import tools.jackson.databind.node.ObjectNode;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
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

    /** A behavior answering a union with an `Int` among its cases, and an injection answering one. */
    private static final String CARRYING = """
            module owing exposing ( Free, quantityOf, doubled )

            data Free

            behavior quantityOf : (paid: Int) -> Int | Free
            let quantityOf (paid) = if paid > 0 then paid else Free

            behavior chooseQuantity : (paid: Int) -> Int | Free

            behavior doubled : (paid: Int) -> Int
                depends on chooseQuantity
            let doubled (paid, chooseQuantity) = match chooseQuantity(paid) with
                | Int as n -> n * 2
                | Free -> 0
            """;

    private static final JsonMapper JSON = JsonMapper.builder().build();

    /** Reads the library's manifest after {@code changing} the whole of it. */
    private static Manifest readWholeAfter(Path into, NativeCompiler.Library library,
                                           Consumer<ObjectNode> changing) throws Exception {
        ObjectNode manifest = (ObjectNode) JSON.readTree(library.manifest().toFile());
        changing.accept(manifest);
        Path changed = into.resolve("changed.json");
        Files.writeString(changed, JSON.writeValueAsString(manifest), StandardCharsets.UTF_8);
        return Manifest.read(changed);
    }

    /** The behavior named {@code name} of {@code module}. */
    private static ObjectNode behavior(ObjectNode module, String name) {
        for (JsonNode it : module.get("behaviors")) {
            if (it.get("name").stringValue().equals(name)) {
                return (ObjectNode) it;
            }
        }
        throw new IllegalArgumentException("no behavior " + name);
    }

    private static NativeCompiler.Library built(Path into, String... sources) throws Exception {
        return NativeCompiler.library(Checked.of(List.of(sources)), into.resolve("native"));
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
                        + " 3, and this generator reads version 12")
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
            ObjectNode at = (ObjectNode) module.get("lists").get(0).get("read").get("at");
            ((ArrayNode) at.get("takes")).remove(2);
        }))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("souther" + Manifest.ABI + "_m_stock_l_value_at");
    }

    /**
     * How a case no declaration names is made and read is said for every such case a host crosses,
     * wherever the manifest names it, and not only where a behavior answers it: here what a host
     * implements answers the `Int`, and no behavior does. A manifest leaving it out is refused as
     * incomplete, and not read as a union a host's language cannot hold.
     */
    @Test
    void aCaseAHostCrossesCarriedWithNothingToMakeItIsRefused(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library = built(into, CARRYING);

        assertThatThrownBy(() -> readWholeAfter(into, library, manifest -> {
            ArrayNode cases = (ArrayNode) manifest.get("cases");
            for (int at = cases.size() - 1; at >= 0; at--) {
                if (cases.get(at).get("case").get("name").stringValue().equals("Int")) {
                    cases.remove(at);
                }
            }
            ArrayNode behaviors = (ArrayNode) manifest.get("modules").get(0).get("behaviors");
            for (int at = behaviors.size() - 1; at >= 0; at--) {
                if (behaviors.get(at).get("name").stringValue().equals("quantityOf")) {
                    behaviors.remove(at);
                }
            }
        }))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("hands across Primitive[name=Int], and nothing of how a value"
                        + " of it is made or read");
    }

    /**
     * An answer that is a union says what tells its cases apart, and that is there exactly where
     * the behavior can be called; a set of cases is never empty. A manifest breaking any of these
     * is refused, and not read as a behavior a host cannot be handed the answer of.
     */
    @Test
    void aUnionAnswerIsSaidWholeOrTheManifestIsRefused(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = built(into, CARRYING);

        assertThatThrownBy(() -> readAfter(into, library, "owing",
                module -> ((ObjectNode) behavior(module, "quantityOf").get("answers"))
                        .putNull("union")))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("says nothing of its cases");
        assertThatThrownBy(() -> readAfter(into, library, "owing",
                module -> ((ObjectNode) behavior(module, "quantityOf").get("answers").get("union"))
                        .putNull("case")))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("its answer's case is told no way");
        assertThatThrownBy(() -> readAfter(into, library, "owing",
                module -> ((ArrayNode) behavior(module, "quantityOf").get("answers").get("union")
                        .get("cases")).removeAll()))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("with no case in it");
    }

    /**
     * A list is reached the way it crosses: a cart's items are handed over to its constructor and
     * handed back by its reader, so the manifest has to say what builds such a list as well as what
     * reads one, and saying only the one is refused.
     */
    @Test
    void aListHandedOverWithNothingToBuildItThroughIsRefused(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = built(into, TWO_MODULES_OF_LISTS, SECOND_MODULE_OF_LISTS);

        assertThatThrownBy(() -> readAfter(into, library, "shop",
                module -> ((ObjectNode) module.get("lists").get(0)).putNull("construct")))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("says nothing to build one through");
        assertThatThrownBy(() -> readAfter(into, library, "shop",
                module -> ((ObjectNode) module.get("lists").get(0)).putNull("read")))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("says nothing to read one through");
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
                .hasMessageContaining("module `shop` says two lists of");
    }

    private static final Manifest.Shape VALUES = new Manifest.Shape.Leaf(Manifest.Word.VALUE);

    private static Manifest.ListCrossing listOf(Manifest.Shape element, String name) {
        List<Manifest.Parameter> built = new ArrayList<>(
                List.of(Manifest.Parameter.given(Manifest.Word.COUNT)));
        element.words().forEach(word -> built.add(Manifest.Parameter.slice(word)));
        List<Manifest.Parameter> at = new ArrayList<>(List.of(
                Manifest.Parameter.given(Manifest.Word.LIST),
                Manifest.Parameter.given(Manifest.Word.COUNT)));
        element.words().forEach(word -> at.add(Manifest.Parameter.room(word)));
        return new Manifest.ListCrossing(element,
                new Manifest.Function(name + "_construct", built, Manifest.Word.LIST),
                new Manifest.ListRead(new Manifest.Function(name + "_length",
                        List.of(Manifest.Parameter.given(Manifest.Word.LIST)), Manifest.Word.COUNT),
                        new Manifest.Function(name + "_at", at, Manifest.Word.BOOL)));
    }

    private static Manifest.Module moduleOf(List<Manifest.ListCrossing> lists) {
        return new Manifest.Module("m", List.of(), List.of(), List.of(), List.of(), List.of(), lists,
                List.of());
    }

    /**
     * A part of a manifest holds what the driver promises of it however it is made, and not only
     * where a manifest is read: a generator handed a module is handed one that keeps its promises.
     */
    @Test
    void aPartOfAManifestIsNotMadeBreakingAPromise() {
        Manifest.ListCrossing values = listOf(VALUES, "values");

        assertThatThrownBy(() -> new Manifest.ListCrossing(
                new Manifest.Shape.Option(new Manifest.Shape.Leaf(Manifest.Word.INT)),
                values.construct(), values.read()))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("values_construct");
        assertThatThrownBy(() -> moduleOf(List.of(values, listOf(VALUES, "again"))))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("module `m` says two lists of");
    }

    /**
     * Which shape a type crosses in is the driver's to say, and is read as it says it, whatever the
     * type: a `Date` said to cross as two words, as a later driver may say, is read, and not held
     * to how any type crosses today. What is held is that each function takes and answers the words
     * of the shapes said beside it.
     */
    @Test
    void aShapeIsReadAsTheDriverSaysItWhateverTheType(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = built(into, SHAPED);

        Manifest read = readAfter(into, library, "shaped", module -> {
            ObjectNode pair = (ObjectNode) module.get("values").get(0);
            pair.set("type", JSON.readTree("{\"kind\": \"primitive\", \"name\": \"Date\"}"));
        });

        Manifest.PublishedValue pair = read.modules().getFirst().values().getFirst();
        assertThat(pair.type()).isEqualTo(new Manifest.Type.Primitive("Date"));
        assertThat(pair.read().available()).isNotNull();
    }

    /**
     * A list a module hands across is one it says the functions of: one handed across with
     * nothing to build it through is one no host can reach.
     */
    @Test
    void aListHandedAcrossWithNothingToBuildItThroughIsRefused(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library = built(into, SHAPED);

        assertThatThrownBy(() -> readAfter(into, library, "shaped",
                module -> ((ArrayNode) module.get("lists")).removeAll()))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("module `shaped` hands a list of");
    }

    /**
     * A function value a module hands across is one it says the functions of, as a list is: one
     * handed across with nothing to call it through is one no host can reach.
     */
    @Test
    void aFunctionValueHandedAcrossWithNothingToCallItThroughIsRefused(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library = Documents.library(Documents.FUNCTIONS,
                into.resolve("native"));

        assertThat(Manifest.read(library.manifest()).modules().getFirst().functions())
                .isNotEmpty();
        assertThatThrownBy(() -> readAfter(into, library, "m",
                module -> ((ArrayNode) module.get("functions")).remove(0)))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("module `m` hands a function of");
    }

    /** Why nothing reaches a value is read as the reason and where it stands. */
    @Test
    void whyNothingReachesAValueIsRead(@TempDir Path into) throws Exception {
        Manifest read = Manifest.read(built(into, SHAPED).manifest());

        Manifest.PublishedValue either = read.modules().getFirst().values().stream()
                .filter(it -> it.name().equals("either")).findFirst().orElseThrow();
        assertThat(either.read()).isEqualTo(new Manifest.Reach.Unavailable<>(new Manifest.Refusal(
                Manifest.Reason.NO_DISCRIMINATOR, List.of(new Manifest.Step.Answers()))));
    }

    /** A tuple, a list of tuples, and a union a host would be handed with nothing to say which case it is. */
    private static final String SHAPED = """
            module shaped exposing ( Free, pair, pairs, either )

            data Free

            let pair: (Int, Bool) = (3, true)

            let pairs = [(1, true)]

            let either: Int | Free = Free
            """;

    /** What a module was made of is copied, so changing it afterwards changes nothing it holds. */
    @Test
    void aModuleHoldsWhatItWasMadeOfAndNotTheListItWasHanded() {
        List<Manifest.ListCrossing> handed = new ArrayList<>(List.of(listOf(VALUES, "values")));
        Manifest.Module module = moduleOf(handed);

        handed.add(listOf(VALUES, "again"));

        assertThat(module.lists()).hasSize(1);
    }
}
