package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.nativecode.transport.ProgramWriter;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.util.ArrayList;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A helper whose {@code sortBy} key answers what its list holds leaves that type open, and the sort
 * is settled at each call, on the order the checker says for the type the call hands it.
 *
 * <p>The checker reads such a helper on its own with the key's answer still a variable, and says no
 * order for it there. It does not hand that reading over: a helper that does not call itself is
 * expanded where it is called, and the sort crosses at the types of that call with the order the
 * checker settled for them. So one helper sorts numbers, a newtype over a number, and the same two
 * units as two enumerations list them, each on its own order, and nothing crosses saying an order
 * over a variable. The driver refuses one that does (#119): a pin of the checker that starts writing
 * one fails here and in the driver's own coherence test, and is what decides how it should cross.
 */
class ASortInAHelperIsSettledWhereTheHelperIsCalledTest {

    private static final String SORTING = """
            module sorting

            data Won
            data Lost
            data Stage = Won | Lost
            data Late = Lost | Won
            data Amount = Int

            let byItself (xs) = List.sortBy(x -> x, xs)

            let stage (n: Int): Stage = if n == 0 then Won else Lost

            let late (n: Int): Late = if n == 0 then Won else Lost

            let staged (s: Stage): Int = match s with
                | Won -> 0
                | Lost -> 1

            let lated (s: Late): Int = match s with
                | Won -> 0
                | Lost -> 1

            behavior ints : (xs: List<Int>) -> List<Int>
            let ints (xs) = byItself(xs)

            behavior amounts : (xs: List<Int>) -> List<Int>
            let amounts (xs) = List.map(a -> a.value, byItself(List.map(x -> Amount(x), xs)))

            behavior stages : (ns: List<Int>) -> List<Int>
            let stages (ns) = List.map(s -> staged(s), byItself(List.map(n -> stage(n), ns)))

            behavior lates : (ns: List<Int>) -> List<Int>
            let lates (ns) = List.map(s -> lated(s), byItself(List.map(n -> late(n), ns)))

            example ints
                | "in the order of the numbers" : ([3, 1, 2]) -> [1, 2, 3]

            example amounts
                | "in the order of the numbers they wrap" : ([30, 10, 20]) -> [10, 20, 30]

            example stages
                | "won first, as Stage lists it" : ([1, 0, 1, 0]) -> [0, 0, 1, 1]

            example lates
                | "lost first, as Late lists it" : ([0, 1, 0]) -> [1, 0, 0]
            """;

    private static final JsonMapper JSON = JsonMapper.builder().build();

    @Test
    void eachCallSortsOnTheOrderOfWhatItHands() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(SORTING);
    }

    /**
     * Every sort crosses with the type it orders and the order the checker settled for it, and none
     * with a variable or with no order: the four calls are four settled sorts.
     */
    @Test
    void noSortCrossesWithItsOrderLeftOpen() {
        JsonNode written = JSON.readTree(
                ProgramWriter.written(Checked.of(List.of(SORTING))));
        List<JsonNode> subjects = new ArrayList<>();
        collect(written, subjects);

        assertThat(subjects).hasSize(4);
        assertThat(subjects).allSatisfy(it -> {
            assertThat(it.get("ordering").isNull()).as("%s says no order", it).isFalse();
            assertThat(it.get("type").findValue("var")).as("%s orders a variable", it).isNull();
        });
        assertThat(subjects).extracting(it -> it.get("ordering").toString())
                .contains("{\"prim\":\"INT\"}",
                        "{\"ref\":{\"is\":\"declared\",\"declared\":\"sorting.Stage\"}}",
                        "{\"ref\":{\"is\":\"declared\",\"declared\":\"sorting.Late\"}}");
    }

    private static void collect(JsonNode node, List<JsonNode> subjects) {
        if (node.isObject() && node.path("is").asString("").equals("orderingsubject")) {
            subjects.add(node);
        }
        for (JsonNode it : node) {
            collect(it, subjects);
        }
    }
}
