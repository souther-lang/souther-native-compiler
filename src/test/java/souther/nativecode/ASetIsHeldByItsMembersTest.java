package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.ValueName;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every kernel of {@code Set} and {@code Map}, the helpers the standard library writes over them,
 * and the {@code List} functions that go through one, held to what the JVM answers for the same
 * program.
 *
 * <p>The rows are the oracle, as they are for the other kernels: each is run on the JVM, and the
 * native run is held to what it answered. No row reads the order a set or a map lists its members
 * in, which the language does not say (spec §stdlib-set): a listing is sorted before it is
 * answered, or folded with what does not care.
 *
 * <p>The rows reach what a hash has to agree with: two {@code Decimal}s of one amount at two
 * scales are one member, two strings built apart are one key, a set of a sum's case stands as a
 * set of the sum and is asked of another case, and a fold the checker rewrote into a walk builds a
 * map a step reads as it goes.
 */
class ASetIsHeldByItsMembersTest {

    private static final String SETS = """
            module sets

            data Lost
            data Won
            data Stage = Won | Lost
            data Sku = String

            behavior counted : (xs: List<Int>) -> Int
            let counted (xs) = Set.size(Set.fromList(xs))

            behavior members : (xs: List<Int>) -> List<Int>
            let members (xs) = List.sort(Set.toList(Set.fromList(xs)))

            behavior holds : (xs: List<Int>, x: Int) -> Bool
            let holds (xs, x) = Set.contains(x, Set.fromList(xs))

            behavior same : (xs: List<Int>, ys: List<Int>) -> Bool
            let same (xs, ys) = Set.fromList(xs) == Set.fromList(ys)

            behavior joined : (xs: List<Int>, ys: List<Int>) -> List<Int>
            let joined (xs, ys) = List.sort(Set.toList(Set.union(Set.fromList(xs), Set.fromList(ys))))

            behavior shared : (xs: List<Int>, ys: List<Int>) -> List<Int>
            let shared (xs, ys) =
                List.sort(Set.toList(Set.intersection(Set.fromList(xs), Set.fromList(ys))))

            behavior apart : (xs: List<Int>, ys: List<Int>) -> List<Int>
            let apart (xs, ys) =
                List.sort(Set.toList(Set.difference(Set.fromList(xs), Set.fromList(ys))))

            behavior grown : (xs: List<Int>, x: Int) -> List<Int>
            let grown (xs, x) = List.sort(Set.toList(Set.insert(x, Set.fromList(xs))))

            behavior shrunk : (xs: List<Int>, x: Int) -> List<Int>
            let shrunk (xs, x) = List.sort(Set.toList(Set.remove(x, Set.fromList(xs))))

            behavior alone : (x: Int) -> Int
            let alone (x) = Set.size(Set.insert(x, Set.singleton(x)))

            behavior emptied : (xs: List<Int>) -> Bool
            let emptied (xs) = Set.isEmpty(Set.fromList(xs))

            behavior amounts : (xs: List<Decimal>) -> Int
            let amounts (xs) = Set.size(Set.fromList(xs))

            behavior texts : (xs: List<String>) -> Int
            let texts (xs) = Set.size(Set.fromList(List.map(s -> String.append(s, "!"), xs)))

            behavior skus : (xs: List<String>) -> Int
            let skus (xs) = Set.size(Set.fromList(List.map(s -> Sku(s), xs)))

            behavior pairs : (xs: List<Int>) -> Int
            let pairs (xs) = Set.size(Set.fromList(List.map(x -> (Int.floorMod(x, 2), x > 0), xs)))

            behavior nested : (xs: List<Int>, ys: List<Int>) -> Int
            let nested (xs, ys) = Set.size(Set.fromList([Set.fromList(xs), Set.fromList(ys)]))

            behavior wonOnly : (lost: Bool) -> Bool
            let wonOnly (lost) = {
                let won: Set<Won> = Set.singleton(Won)
                let stages: Set<Stage> = won
                let asked: Stage = if lost then Lost else Won
                Set.contains(asked, stages)
            }

            let emptySets = Set.singleton(Set.fromList([]))
            let emptyLists = Set.singleton([[]])

            behavior emptyWidened : (n: Int) -> Bool
            let emptyWidened (n) = {
                let sets: Set<Set<Int>> = emptySets
                let lists: Set<List<List<Int>>> = emptyLists
                let none: Set<Int> = Set.fromList(List.rangeInclusive(1, n))
                let inner: List<Int> = List.rangeInclusive(1, n)
                Set.contains(none, sets) && Set.contains([inner], lists)
            }

            behavior keptMember : (n: Int) -> String
            let keptMember (n) = {
                let built = Set.fromList([1.0m, 1.00m, 2.00m])
                let grown = Set.insert(2.0m, built)
                String.join(",", List.map(k -> String.fromDecimal(k), List.sort(Set.toList(grown))))
            }

            let scaled (s: Set<Decimal>): String =
                String.join(",", List.map(k -> String.fromDecimal(k), List.sort(Set.toList(s))))

            behavior keptJoined : (n: Int) -> String
            let keptJoined (n) =
                if n == 0 then scaled(Set.union(Set.fromList([1.0m]), Set.fromList([1.00m, 2m])))
                else scaled(Set.union(Set.fromList([1.0m]), Set.fromList([1.00m])))

            behavior keptShared : (n: Int) -> String
            let keptShared (n) =
                if n == 0 then scaled(Set.intersection(Set.fromList([1.0m, 2m]), Set.fromList([1.00m])))
                else scaled(Set.intersection(Set.fromList([1.0m]), Set.fromList([1.00m])))

            behavior parities : (xs: List<Int>) -> Int
            let parities (xs) = Set.size(Set.map(x -> Int.floorMod(x, 2), Set.fromList(xs)))

            behavior kept : (xs: List<Int>) -> List<Int>
            let kept (xs) = List.sort(Set.toList(Set.filter(x -> x > 2, Set.fromList(xs))))

            behavior split : (xs: List<Int>) -> Int
            let split (xs) = {
                let (above, rest) = Set.partition(x -> x > 2, Set.fromList(xs))
                Set.size(above) * 100 + Set.size(rest)
            }

            behavior summed : (xs: List<Int>) -> Int
            let summed (xs) = Set.fold((acc, x) -> acc + x, 0, Set.fromList(xs))

            behavior firsts : (xs: List<Int>) -> List<Int>
            let firsts (xs) = List.distinct(xs)

            behavior firstsBy : (xs: List<Int>) -> List<Int>
            let firstsBy (xs) = List.distinctBy(x -> Int.floorMod(x, 3), xs)

            behavior unique : (xs: List<Int>) -> Bool
            let unique (xs) = List.allDistinctBy(x -> x, xs)

            example counted
                | "none" : ([]) -> 0
                | "each once" : ([3, 1, 2]) -> 3
                | "some twice" : ([3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5]) -> 7

            example members
                | "some twice" : ([3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5]) -> [1, 2, 3, 4, 5, 6, 9]

            example holds
                | "there" : ([1, 2, 3], 2) -> true
                | "not there" : ([1, 2, 3], 4) -> false
                | "the smallest Int but one" : ([-9223372036854775807], -9223372036854775807) -> true

            example same
                | "in another order" : ([1, 2, 3], [3, 2, 1, 1]) -> true
                | "one more" : ([1, 2, 3], [1, 2, 3, 4]) -> false
                | "one other" : ([1, 2, 3], [1, 2, 4]) -> false
                | "both empty" : ([], []) -> true

            example joined
                | "overlapping" : ([1, 2, 3], [3, 4]) -> [1, 2, 3, 4]

            example shared
                | "overlapping" : ([1, 2, 3], [3, 4, 2]) -> [2, 3]

            example apart
                | "overlapping" : ([1, 2, 3], [3, 4, 2]) -> [1]

            example grown
                | "a new one" : ([1, 2], 3) -> [1, 2, 3]
                | "one it holds" : ([1, 2], 2) -> [1, 2]

            example shrunk
                | "one it holds" : ([1, 2, 3], 2) -> [1, 3]
                | "one it does not" : ([1, 2, 3], 7) -> [1, 2, 3]

            example alone
                | "one" : (7) -> 1

            example emptied
                | "empty" : ([]) -> true
                | "not" : ([1]) -> false

            example amounts
                | "one amount at two scales" : ([1.0m, 1.00m, 2.5m]) -> 2

            example texts
                | "built apart" : (["a", "a", "b"]) -> 2

            example skus
                | "wrapped alike" : (["a", "a", "b"]) -> 2

            example pairs
                | "by parity and sign" : ([1, 3, -1, 2, 4, -2]) -> 4

            example nested
                | "the same members" : ([1, 2], [2, 1]) -> 1
                | "other members" : ([1, 2], [2, 3]) -> 2

            example wonOnly
                | "the case it holds" : (false) -> true
                | "another case of the sum" : (true) -> false

            example emptyWidened
                | "an empty set and list built of nothing, asked of as sets of Int" : (0) -> true
                | "something else" : (1) -> false

            example keptMember
                | "the members first put in" : (0) -> "1.0,2.00"

            example keptJoined
                | "the larger's" : (0) -> "1.00,2"
                | "the first's where the two are as large" : (1) -> "1.0"

            example keptShared
                | "the smaller's" : (0) -> "1.00"
                | "the second's where the two are as large" : (1) -> "1.00"

            example parities
                | "odd and even" : ([1, 2, 3, 4, 5]) -> 2

            example kept
                | "above two" : ([1, 2, 3, 4, 3]) -> [3, 4]

            example split
                | "above two and the rest" : ([1, 2, 3, 4, 3]) -> 202

            example summed
                | "each member once" : ([1, 2, 2, 3]) -> 6

            example firsts
                | "in the order they came" : ([3, 1, 3, 2, 1]) -> [3, 1, 2]

            example firstsBy
                | "the first of each remainder" : ([3, 1, 4, 6, 2, 7]) -> [3, 1, 2]

            example unique
                | "each once" : ([1, 2, 3]) -> true
                | "one twice" : ([1, 2, 1]) -> false
            """;

    private static final String MAPS = """
            module maps

            let counts (words: List<String>): Map<String, Int> =
                List.fold((acc, w) -> Map.updateOrInsert(w, 1, n -> n + 1, acc), Map.empty, words)

            behavior countOf : (words: List<String>, w: String) -> Int
            let countOf (words, w) = match Map.get(w, counts(words)) with
                | Some n -> n
                | None -> 0

            behavior sized : (words: List<String>) -> Int
            let sized (words) = Map.size(counts(words))

            behavior keysOf : (words: List<String>) -> List<String>
            let keysOf (words) = List.sort(Map.keys(counts(words)))

            behavior valuesOf : (words: List<String>) -> List<Int>
            let valuesOf (words) = List.sort(Map.values(counts(words)))

            behavior has : (words: List<String>, w: String) -> Bool
            let has (words, w) = Map.containsKey(w, counts(words))

            behavior without : (words: List<String>, w: String) -> List<String>
            let without (words, w) = List.sort(Map.keys(Map.remove(w, counts(words))))

            behavior total : (words: List<String>) -> Int
            let total (words) = Map.fold((acc, k, v) -> acc + v, 0, counts(words))

            behavior doubled : (words: List<String>) -> List<Int>
            let doubled (words) = List.sort(Map.values(Map.mapValues((k, v) -> v * 2, counts(words))))

            behavior often : (words: List<String>) -> List<String>
            let often (words) = List.sort(Map.keys(Map.filterEntries((k, v) -> v > 1, counts(words))))

            behavior same : (xs: List<String>, ys: List<String>) -> Bool
            let same (xs, ys) = counts(xs) == counts(ys)

            behavior paired : (words: List<String>) -> Int
            let paired (words) = Map.size(Map.fromList(List.map(w -> (w, 1), words)))

            behavior rebuilt : (words: List<String>) -> Bool
            let rebuilt (words) = Map.fromList(Map.toList(counts(words))) == counts(words)

            behavior alone : (w: String) -> Int
            let alone (w) = Map.size(Map.insert(w, 2, Map.singleton(w, 1)))

            behavior emptied : (words: List<String>) -> Bool
            let emptied (words) = Map.isEmpty(counts(words))

            behavior bumped : (words: List<String>, w: String) -> Int
            let bumped (words, w) = match Map.get(w, Map.updateIfPresent(w, n -> n * 10, counts(words))) with
                | Some n -> n
                | None -> -1

            behavior joined : (xs: List<String>, ys: List<String>) -> List<Int>
            let joined (xs, ys) = List.sort(Map.values(Map.union(counts(xs), counts(ys))))

            behavior shared : (xs: List<String>, ys: List<String>) -> List<String>
            let shared (xs, ys) = List.sort(Map.keys(Map.intersection(counts(xs), counts(ys))))

            behavior apart : (xs: List<String>, ys: List<String>) -> List<String>
            let apart (xs, ys) = List.sort(Map.keys(Map.difference(counts(xs), counts(ys))))

            behavior grouped : (xs: List<Int>) -> List<Int>
            let grouped (xs) =
                List.sort(List.map(g -> List.sum(g), Map.values(List.groupBy(x -> Int.floorMod(x, 3), xs))))

            let written (keys: List<Decimal>): String =
                String.join(",", List.map(k -> String.fromDecimal(k), List.sort(keys)))

            let heldAtTwoScales: Map<Decimal, Int> = Map.fromList([(1.0m, 1), (2.00m, 2), (1.00m, 3)])

            behavior keptOnInsert : (n: Int) -> String
            let keptOnInsert (n) = {
                let inserted = Map.insert(2.0m, n, heldAtTwoScales)
                match Map.get(2m, inserted) with
                    | Some v -> String.append(written(Map.keys(inserted)), String.append(":", String.fromInt(v)))
                    | None -> "none"
            }

            behavior keptFromList : (n: Int) -> String
            let keptFromList (n) = match Map.get(1m, heldAtTwoScales) with
                | Some v -> String.append(written(Map.keys(heldAtTwoScales)), String.append(":", String.fromInt(v)))
                | None -> "none"

            behavior keptOnUpdate : (n: Int) -> String
            let keptOnUpdate (n) = {
                let updated = Map.updateOrInsert(1.000m, n, v -> v + n, heldAtTwoScales)
                let bumped = Map.updateIfPresent(2.0m, v -> v * 10, updated)
                written(Map.keys(bumped))
            }

            behavior keptGrouped : (xs: List<Int>) -> String
            let keptGrouped (xs) =
                written(Map.keys(List.groupBy(x -> if x > 1 then 1.00m else 1.0m, xs)))

            behavior keptIndexed : (xs: List<Int>) -> String
            let keptIndexed (xs) =
                written(Map.keys(List.indexBy(x -> if x > 1 then 1.00m else 1.0m, xs)))

            behavior indexed : (xs: List<Int>, at: Int) -> Int
            let indexed (xs, at) = match Map.get(at, List.indexBy(x -> Int.floorMod(x, 10), xs)) with
                | Some x -> x
                | None -> -1

            example countOf
                | "twice" : (["a", "b", "a"], "a") -> 2
                | "not there" : (["a", "b", "a"], "c") -> 0

            example sized
                | "two words" : (["a", "b", "a"]) -> 2
                | "none" : ([]) -> 0

            example keysOf
                | "each once" : (["b", "a", "b"]) -> ["a", "b"]

            example valuesOf
                | "counted" : (["b", "a", "b"]) -> [1, 2]

            example has
                | "there" : (["a"], "a") -> true
                | "not there" : (["a"], "b") -> false

            example without
                | "one of them" : (["a", "b", "c"], "b") -> ["a", "c"]
                | "none of them" : (["a", "b"], "z") -> ["a", "b"]

            example total
                | "every word" : (["a", "b", "a", "c"]) -> 4

            example doubled
                | "counted twice over" : (["a", "b", "a"]) -> [2, 4]

            example often
                | "more than once" : (["a", "b", "a", "c", "c"]) -> ["a", "c"]

            example same
                | "in another order" : (["a", "b", "a"], ["b", "a", "a"]) -> true
                | "another count" : (["a", "b"], ["b", "a", "a"]) -> false

            example paired
                | "a later pair" : (["a", "a", "b"]) -> 2

            example rebuilt
                | "through its pairs" : (["a", "b", "a"]) -> true

            example alone
                | "one key" : ("a") -> 1

            example emptied
                | "empty" : ([]) -> true
                | "not" : (["a"]) -> false

            example bumped
                | "there" : (["a", "a"], "a") -> 20
                | "not there" : (["a"], "b") -> -1

            example joined
                | "the first wins" : (["a", "a"], ["a", "b"]) -> [1, 2]

            example shared
                | "keys in both" : (["a", "b"], ["b", "c"]) -> ["b"]

            example apart
                | "keys in the first alone" : (["a", "b"], ["b", "c"]) -> ["a"]

            example grouped
                | "by remainder" : ([1, 2, 3, 4, 5, 6]) -> [5, 7, 9]

            example keptOnInsert
                | "the key held, the value put" : (7) -> "1.0,2.00:7"

            example keptFromList
                | "the earlier key, the later value" : (0) -> "1.0,2.00:3"

            example keptOnUpdate
                | "updated and bumped under the keys held" : (5) -> "1.0,2.00"

            example keptGrouped
                | "the first key the group was made under" : ([1, 2, 3]) -> "1.0"
                | "only the larger" : ([2, 3]) -> "1.00"

            example keptIndexed
                | "the first key the entry was made under" : ([2, 1, 3]) -> "1.00"

            example indexed
                | "the last of each" : ([1, 11, 2], 1) -> 11
                | "none" : ([1, 11, 2], 5) -> -1
            """;

    /**
     * A set and a map answered and handed over whole, which is where each is written out and read
     * back in: a set as the array of its members in the order a boundary writes them in, and a map
     * as an object, its keys each written as the key's type is and read back as it. A set of a
     * sum's case answered as a set of the sum is the set it was.
     */
    private static final String ANSWERED = """
            module answered exposing ( Lost, Won, Stage, Sku, Holding, Shelf, texts, tallied, held,
                                       shelved, winning, amounts )

            data Lost
            data Won
            data Stage = Won | Lost
            data Sku = String
            data Holding = { tags: Set<String>, counts: Map<String, Int> }
            data Shelf = { bySku: Map<Sku, Set<Int>> }

            behavior members : (xs: List<Int>) -> Set<Int>
            let members (xs) = Set.fromList(xs)

            behavior echoed : (s: Set<Int>) -> Set<Int>
            let echoed (s) = s

            behavior sized : (s: Set<Int>) -> Int
            let sized (s) = Set.size(s)

            behavior counted : (words: List<String>) -> Map<String, Int>
            let counted (words) =
                List.fold((acc, w) -> Map.updateOrInsert(w, 1, n -> n + 1, acc), Map.empty, words)

            behavior keyed : (m: Map<String, Int>) -> Int
            let keyed (m) = Map.fold((acc, k, v) -> acc + v, 0, m)

            behavior held : (n: Int) -> Holding
            let held (n) = Holding { tags = Set.fromList(["b", "a", "b"]), counts = counted(["b", "a", "b"]) }

            behavior shelved : (n: Int) -> Shelf
            let shelved (n) = Shelf { bySku = Map.fromList([(Sku("b"), Set.fromList([n, 1])), (Sku("a"), Set.empty)]) }

            behavior winning : (n: Int) -> Set<Stage>
            let winning (n) = {
                let wins: Set<Won> = Set.singleton(Won)
                wins
            }

            behavior amounts : (n: Int) -> Set<Decimal>
            let amounts (n) = Set.fromList([2.5m, 1.0m, 1.00m])

            let words: List<String> = ["b", "｡", "a", "𐀀", "b"]

            behavior texts : (n: Int) -> Set<String>
            let texts (n) = Set.fromList(words)

            behavior tallied : (n: Int) -> Map<String, Int>
            let tallied (n) = counted(words)

            example members
                | "in the order a boundary writes" : ([3, 1, 2, 1]) -> [1, 2, 3]

            example echoed
                | "read and written again" : ([2, 1]) -> [1, 2]

            example sized
                | "read with its members once each" : ([1, 1, 2]) -> 2

            example keyed
                | "read whole" : (Map.fromList([("a", 1), ("b", 2)])) -> 3

            """;

    @Test
    void everySetAndMapAnsweredHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(ANSWERED);
    }

    /**
     * What a boundary writes is fixed by the members and not by the collection (spec
     * §collections): a set's members ascending by their own external representation, text by
     * UTF-16 code unit, and a map's members ascending by their rendered keys, at any depth. So U+FF61
     * comes after U+10000, whose first unit is a surrogate, where its UTF-8 comes before. Held as
     * text, since the order is what is asked.
     */
    @Test
    void aSetAndAMapAreWrittenInTheOrderOfWhatTheyHold() throws Exception {
        CheckedProgram program = Checked.of(List.of(ANSWERED));
        Running running = Running.of(program);
        String late = "\uFF61";
        String past = new String(Character.toChars(0x10000));
        ObservedValue none = new ObservedValue.Integer(0);

        assertThat(written(running, program, "texts", none))
                .isEqualTo("[\"a\",\"b\",\"" + past + "\",\"" + late + "\"]");
        assertThat(written(running, program, "tallied", none))
                .isEqualTo("{\"a\":1,\"b\":2,\"" + past + "\":1,\"" + late + "\":1}");
        assertThat(written(running, program, "held", none))
                .isEqualTo("{\"tags\":[\"a\",\"b\"],\"counts\":{\"a\":1,\"b\":2}}");
        // Two amounts at two scales are one member, written as its amount.
        assertThat(written(running, program, "amounts", none)).isEqualTo("[1,2.5]");
        assertThat(written(running, program, "shelved", new ObservedValue.Integer(5)))
                .isEqualTo("{\"bySku\":{\"a\":[],\"b\":[1,5]}}");
        // A set of a sum's case answered as a set of the sum, each member written as the sum's.
        assertThat(written(running, program, "winning", new ObservedValue.Integer(0)))
                .isEqualTo("[\"Won\"]");
    }

    /**
     * Read from a document, a set is its array's members once each, whatever order they came in,
     * and a map is its object's members, each key read as the key's own type is read anywhere,
     * its invariant and all. Two keys that are one once read, two spellings of one moment, are
     * refused where the second stands, as the JVM's reader refuses them, rather than have one
     * value lost to the other. Every value is read before any key is.
     */
    @Test
    void aSetAndAMapAreReadAsTheLanguageWritesThem() throws Exception {
        String harness = new Decoding()
                .type("read", "Holding").type("read", "Shelf").type("read", "Moments")
                .row("holding", "Holding", "{\"tags\":[\"b\",\"a\",\"b\"],\"counts\":{\"b\":2,\"a\":1}}")
                .row("holding not a map", "Holding", "{\"tags\":[],\"counts\":[1]}")
                .row("holding wrong value", "Holding", "{\"tags\":[1],\"counts\":{\"a\":\"x\"}}")
                .row("shelf", "Shelf", "{\"bySku\":{\"b\":[5,1,5],\"a\":[]}}")
                .row("shelf key", "Shelf", "{\"bySku\":{\"\":[1],\"a\":[2]}}")
                .row("shelf value first", "Shelf", "{\"bySku\":{\"\":[1],\"a\":[\"x\"]}}")
                .row("moments", "Moments", "{\"at\":{\"2026-01-01T00:00:00Z\":1}}")
                .row("moments twice", "Moments",
                        "{\"at\":{\"2026-01-01T00:00:00Z\":1,\"2026-01-01T09:00:00+09:00\":2}}")
                .harness();

        assertThat(AValueIsReadFromTheFormItIsWrittenInTest.run(Checked.of(List.of(READ)), harness))
                .isEqualTo("""
                        holding: value {"tags":["a","b"],"counts":{"a":1,"b":2}}
                        holding not a map: issues [@/counts type_mismatch actual=array expected=an object]
                        holding wrong value: issues [@/tags/0 type_mismatch actual=number expected=String] [@/counts/a type_mismatch actual=string expected=Int]
                        shelf: value {"bySku":{"a":[],"b":[1,5]}}
                        shelf key: issues [@/bySku/ invariant_violation module=read type=Sku clause=named]
                        shelf value first: issues [@/bySku/a/0 type_mismatch actual=string expected=Int]
                        moments: value {"at":{"2026-01-01T00:00:00Z":1}}
                        moments twice: issues [@/at/2026-01-01T09:00:00+09:00 duplicate_key]
                        """);
    }

    private static final String READ = """
            module read exposing ( Holding, Shelf, Moments, Sku )

            data Sku = String
                invariant named = String.length(value) > 0
            data Holding = { tags: Set<String>, counts: Map<String, Int> }
            data Shelf = { bySku: Map<Sku, Set<Int>> }
            data Moments = { at: Map<Instant, Int> }
            """;

    private static String written(Running running, CheckedProgram program, String name,
                                  ObservedValue... inputs) throws Exception {
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior behavior = module.behavior(new ValueName.Behavior(module.name(), name));
        return running.externalAnswer(module, behavior, List.of(inputs)).toString();
    }


    @Test
    void everySetRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(SETS);
    }

    @Test
    void everyMapRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(MAPS);
    }
}
