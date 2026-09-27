package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A list built in one object and read in another, which is where its layout stops being one
 * object's own business.
 *
 * <p>The list crosses inside a value of a declared type, which is how a list crosses objects today:
 * a behavior answering a list itself is also something a host is handed, and that is #53. What the
 * two objects have to agree on is the list's layout, which both read from {@code souther-native-abi},
 * and what its elements are, which are values of a type the first build declares: tagged with the
 * token the linker resolves, and laid out by the declaration each build read.
 *
 * <p>Three readings of such a list are put to the pair: {@code List.length} and {@code List.get}
 * here over a list built there, {@code ==} here between it and one built here, and a document read
 * here into a list of the other build's type, each element by the reader that build defines.
 */
class AListCrossesBetweenTwoBuildsTest {

    private static final String BUILT_BEFORE = """
            module lib.orders exposing ( OrderLine, Order, ordered, sizes, bought )

            data OrderLine = { sku: String, quantity: Int }
                invariant positive = quantity > 0

            data Order = { number: Int, lines: List<OrderLine> }

            let sizes: List<Int> = [10, 20, 30]

            let bought: Set<OrderLine> = Set.fromList([
                OrderLine { sku = "a", quantity = 2 },
                OrderLine { sku = "b", quantity = 3 }
            ])

            behavior ordered : (a: Int, b: Int) -> Order
            let ordered (a, b) = Order { number = 1, lines = [
                OrderLine { sku = "a", quantity = a },
                OrderLine { sku = "b", quantity = b }
            ] }
            """;

    private static final String READING_IT = """
            module app.reads exposing ( counted, second, same, sized, holding, Basket )
            import lib.orders ( OrderLine, Order, ordered, sizes, bought )

            data Basket = { lines: List<OrderLine> }

            behavior counted : (a: Int, b: Int) -> Int
            let counted (a, b) = List.length(ordered(a, b).lines)

            behavior second : (a: Int, b: Int) -> Int
            let second (a, b) = match List.get(1, ordered(a, b).lines) with
                | Some line -> line.quantity
                | None -> -1

            behavior sized : (a: Int, b: Int) -> Int
            let sized (a, b) = match List.get(a, sizes) with
                | Some size -> size + b
                | None -> -1

            behavior holding : (a: Int, b: Int) -> Bool
            let holding (a, b) = Set.contains(OrderLine { sku = "a", quantity = a }, bought)
                && Set.fromList([OrderLine { sku = "b", quantity = b }, OrderLine { sku = "a", quantity = 2 }])
                    == bought

            behavior same : (a: Int, b: Int) -> Bool
            let same (a, b) = ordered(a, b).lines
                == [OrderLine { sku = "a", quantity = 2 }, OrderLine { sku = "b", quantity = 3 }]
            """;

    @Test
    void aListBuiltThereIsReadHere() throws Exception {
        Running running = running();

        assertThat(run(running, "counted", 2, 3)).isEqualTo(answered(new ObservedValue.Integer(2)));
        assertThat(run(running, "second", 2, 3)).isEqualTo(answered(new ObservedValue.Integer(3)));
    }

    /**
     * A list that is itself what crosses: a value the other build publishes, reached through the
     * entry that build defines for it. Its type is held to meaning the same in both objects, which
     * a list does where its elements do.
     */
    @Test
    void aListThatIsWhatAnotherBuildPublishesIsReadHere() throws Exception {
        Running running = running();

        assertThat(run(running, "sized", 2, 1)).isEqualTo(answered(new ObservedValue.Integer(31)));
        assertThat(run(running, "sized", 3, 1)).isEqualTo(answered(new ObservedValue.Integer(-1)));
    }

    /**
     * A set that is what crosses: built there, of that build's own type, and asked here whether it
     * holds a value built here, and whether it is equal to one built here in another order. Each
     * member was put in under the hash that object worked out, and is asked for under the hash this
     * one works out, which the two compose alike (`souther_native_abi::HASHING`).
     */
    @Test
    void aSetBuiltThereIsAskedHereByWhatItHolds() throws Exception {
        Running running = running();

        assertThat(run(running, "holding", 2, 3)).isEqualTo(answered(new ObservedValue.Bool(true)));
        assertThat(run(running, "holding", 4, 3)).isEqualTo(answered(new ObservedValue.Bool(false)));
        assertThat(run(running, "holding", 2, 5)).isEqualTo(answered(new ObservedValue.Bool(false)));
    }

    @Test
    void aListBuiltThereIsEqualToOneBuiltHereByWhatItHolds() throws Exception {
        Running running = running();

        assertThat(run(running, "same", 2, 3)).isEqualTo(answered(new ObservedValue.Bool(true)));
        assertThat(run(running, "same", 2, 4)).isEqualTo(answered(new ObservedValue.Bool(false)));
    }

    /** Each element is read by the reader of the build that declares its type, clause and all. */
    @Test
    void aListOfTheOtherBuildsTypeIsReadHereByThatBuildsReader() throws Exception {
        String harness = new Decoding().type("app.reads", "Basket")
                .row("basket", "Basket", "{\"lines\":[{\"sku\":\"a\",\"quantity\":1}]}")
                .row("basket broken line", "Basket",
                        "{\"lines\":[{\"sku\":\"a\",\"quantity\":1},{\"sku\":\"b\",\"quantity\":0}]}")
                .harness();

        assertThat(AValueIsReadFromTheFormItIsWrittenInTest.run(compiled(),
                List.of(new NativeArtifacts.Bytes(NativeArtifacts.object(builtBefore()))), harness))
                .isEqualTo("""
                        basket: value {"lines":[{"sku":"a","quantity":1}]}
                        basket broken line: issues [@/lines/1 invariant_violation {"clause":"positive","module":"lib.orders","type":"OrderLine"}]
                        """);
    }

    private static Running running() throws Exception {
        return Running.of(compiled(), List.of(NativeArtifacts.object(builtBefore())));
    }

    private static RunOutcome run(Running running, String name, long a, long b) throws Exception {
        CheckedModule module = compiled().modules().getFirst();
        CheckedBehavior behavior = module.behaviors().stream()
                .filter(it -> it.name().name().equals(name))
                .findFirst()
                .orElseThrow(() -> new AssertionError("no behavior " + name));
        return running.answeredOrEnded(module, behavior,
                List.of(new ObservedValue.Integer(a), new ObservedValue.Integer(b)));
    }

    private static RunOutcome answered(ObservedValue value) {
        return new RunOutcome.Answered(value);
    }

    private static CheckedProgram compiled() {
        Map<String, ClassFileImage> published = Compiler.compile(BUILT_BEFORE);
        return Checked.of(List.of(READING_IT), ModulePath.of(published));
    }

    private static CheckedProgram builtBefore() {
        return Checked.of(List.of(BUILT_BEFORE));
    }
}
