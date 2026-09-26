package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every kernel of {@code Int} and {@code String}, and those of {@code List} and {@code Option}
 * this backend lowers, held to what the JVM answers for the same program.
 *
 * <p>The rows are the oracle. A row that ran and whose answer kept it is one the JVM answered, so a
 * native run put to the same row is held to the JVM's answer without this file writing down what
 * either should say (#3). The rows are where the two carriers could part: the ends of the
 * {@code Int} range, text past the basic plane, a mark that composes with what it is joined to,
 * whitespace and digits outside ASCII, a Greek sigma at the end of a word, and a pattern over
 * characters a JVM string writes as two units.
 *
 * <p>A row states a value and not a run that ends, so where a kernel ends the run the reason is
 * asked of the native run and held to what the kernel's contract says it ends for.
 */
class AKernelAnswersWhatTheJvmAnswersTest {

    private static final String INT = """
            module ints

            behavior less : (a: Int, b: Int) -> Int
            let less (a, b) = Int.subtract(a, b)

            behavior times : (a: Int, b: Int) -> Int
            let times (a, b) = Int.multiply(a, b)

            behavior ordered : (a: Int, b: Int) -> Int
            let ordered (a, b) = Int.compare(a, b)

            behavior floored : (a: Int, b: Int) -> Int
            let floored (a, b) = Int.floorMod(a, b)

            example less
                | "below nought" : (5, 7) -> -2
                | "from the smallest but one" : (-9223372036854775807, 0) -> -9223372036854775807
                | "to the largest" : (9223372036854775806, -1) -> 9223372036854775807

            example times
                | "of two signs" : (3, -4) -> -12
                | "by nought" : (0, 9223372036854775807) -> 0
                | "to the largest but one" : (4611686018427387903, 2) -> 9223372036854775806

            example ordered
                | "below" : (1, 2) -> -1
                | "at" : (2, 2) -> 0
                | "above" : (3, 2) -> 1
                | "further apart than an Int holds" : (-9223372036854775807, 9223372036854775807) -> -1
                | "the other way" : (9223372036854775807, -9223372036854775807) -> 1

            example floored
                | "both above nought" : (7, 3) -> 1
                | "a dividend below nought" : (-7, 3) -> 2
                | "a divisor below nought" : (7, -3) -> -2
                | "both below nought" : (-7, -3) -> -1
                | "a multiple" : (-6, 3) -> 0
                | "by minus one" : (-9223372036854775807, -1) -> 0
                | "by the largest" : (-1, 9223372036854775807) -> 9223372036854775806
            """;

    private static final String TEXT = """
            module texts

            behavior parsed : (s: String) -> Int
            let parsed (s) = match String.toInt(s) with
                | Int as n -> n
                | NotANumber -> 424242

            behavior parsedPastTheSmallest : (s: String) -> Int
            let parsedPastTheSmallest (s) = match String.toInt(s) with
                | Int as n -> n + 1
                | NotANumber -> 424242

            behavior written : (n: Int) -> String
            let written (n) = String.fromInt(n)

            behavior trimmed : (s: String) -> String
            let trimmed (s) = String.trim(s)

            behavior lower : (s: String) -> String
            let lower (s) = String.lowercase(s)

            behavior upper : (s: String) -> String
            let upper (s) = String.uppercase(s)

            behavior holds : (sub: String, s: String) -> Bool
            let holds (sub, s) = String.contains(sub, s)

            behavior begins : (prefix: String, s: String) -> Bool
            let begins (prefix, s) = String.startsWith(prefix, s)

            behavior ends : (suffix: String, s: String) -> Bool
            let ends (suffix, s) = String.endsWith(suffix, s)

            behavior sliced : (from: Int, to: Int, s: String) -> String
            let sliced (from, to, s) = String.slice(from, to, s)

            behavior appended : (a: String, b: String) -> String
            let appended (a, b) = String.append(a, b)

            behavior split : (sep: String, s: String) -> List<String>
            let split (sep, s) = String.split(sep, s)

            behavior joined : (sep: String, xs: List<String>) -> String
            let joined (sep, xs) = String.join(sep, xs)

            behavior concatenated : (xs: List<String>) -> String
            let concatenated (xs) = String.concat(xs)

            behavior replaced : (target: String, replacement: String, s: String) -> String
            let replaced (target, replacement, s) = String.replace(target, replacement, s)

            behavior worded : (s: String) -> List<String>
            let worded (s) = String.words(s)

            behavior lined : (s: String) -> List<String>
            let lined (s) = String.lines(s)

            behavior reversed : (s: String) -> String
            let reversed (s) = String.reverse(s)

            behavior repeated : (n: Int, s: String) -> String
            let repeated (n, s) = String.repeat(n, s)

            behavior paddedLeft : (width: Int, pad: String, s: String) -> String
            let paddedLeft (width, pad, s) = String.padLeft(width, pad, s)

            behavior paddedRight : (width: Int, pad: String, s: String) -> String
            let paddedRight (width, pad, s) = String.padRight(width, pad, s)

            behavior characters : (s: String) -> List<String>
            let characters (s) = String.characters(s)

            behavior points : (s: String) -> List<Int>
            let points (s) = String.codePoints(s)

            example parsed
                | "leading zeros" : ("007") -> 7
                | "a plus and leading zeros" : ("+007") -> 7
                | "a minus and leading zeros" : ("-007") -> -7
                | "minus nought" : ("-0") -> 0
                | "the largest" : ("9223372036854775807") -> 9223372036854775807
                | "one past the largest" : ("9223372036854775808") -> 424242
                | "one below the smallest" : ("-9223372036854775809") -> 424242
                | "nothing" : ("") -> 424242
                | "a sign alone" : ("-") -> 424242
                | "whitespace before" : (" 5") -> 424242
                | "whitespace after" : ("5 ") -> 424242
                | "full-width digits" : ("１２３") -> 424242
                | "an Arabic-Indic digit" : ("٣") -> 424242
                | "a letter after" : ("12x") -> 424242

            example parsedPastTheSmallest
                | "the smallest" : ("-9223372036854775808") -> -9223372036854775807

            example written
                | "nought" : (0) -> "0"
                | "below nought" : (-42) -> "-42"
                | "the largest" : (9223372036854775807) -> "9223372036854775807"

            example trimmed
                | "an ideographic space and a no-break space" : ("　 a b \\n") -> "a b"
                | "nothing but whitespace" : ("   ") -> ""
                | "an em space and a medium mathematical space" : (" x ") -> "x"
                | "a zero-width space is not whitespace" : ("​x") -> "​x"

            example lower
                | "a sigma at the end of a word" : ("ΟΣ") -> "ος"
                | "a sigma inside a word" : ("ΟΣΑ") -> "οσα"
                | "a sharp s's capitals" : ("STRASSE") -> "strasse"
                | "a capital I with a dot" : ("İ") -> "i̇"
                | "text with no case" : ("日本 123") -> "日本 123"

            example upper
                | "a sharp s" : ("straße") -> "STRASSE"
                | "a Turkish-looking i" : ("i") -> "I"
                | "a dotless i" : ("ı") -> "I"
                | "a ligature" : ("ﬁ") -> "FI"
                | "a j with a caron" : ("ǰ") -> "J̌"

            example holds
                | "nothing is in everything" : ("", "abc") -> true
                | "past the basic plane" : ("𠮷", "a𠮷b") -> true
                | "not there" : ("d", "abc") -> false

            example begins
                | "a prefix" : ("ab", "abc") -> true
                | "longer than it" : ("abcd", "abc") -> false

            example ends
                | "a suffix" : ("語", "日本語") -> true
                | "not a suffix" : ("日", "日本語") -> false

            example sliced
                | "code points and not units" : (1, 3, "a𠮷b日") -> "𠮷b"
                | "the whole" : (0, 4, "a𠮷b日") -> "a𠮷b日"
                | "nothing at the end" : (4, 4, "a𠮷b日") -> ""

            example appended
                | "a mark composes with the letter it follows" : ("e", "́") -> "é"
                | "two texts" : ("日", "本") -> "日本"

            example split
                | "empty pieces kept" : (",", "a,,b") -> ["a", "", "b"]
                | "an empty separator" : ("", "abc") -> ["abc"]
                | "at the end" : ("日", "a日b日") -> ["a", "b", ""]

            example joined
                | "with a separator" : ("-", ["a", "b", "c"]) -> "a-b-c"
                | "a mark joined to a letter" : ("", ["e", "́"]) -> "é"
                | "nothing" : ("-", []) -> ""

            example concatenated
                | "past the basic plane" : (["a", "日", "𠮷"]) -> "a日𠮷"

            example replaced
                | "none overlapping" : ("aa", "b", "aaa") -> "ba"
                | "an empty target" : ("", "-", "abc") -> "abc"
                | "a mark replacing a letter" : ("x", "́", "ex") -> "é"

            example worded
                | "the language's whitespace" : ("  a　b c ") -> ["a", "b", "c"]
                | "nothing but whitespace" : (" \\t ") -> []

            example lined
                | "both newlines" : ("a\\nb\\r\\nc") -> ["a", "b", "c"]
                | "a newline at the end" : ("a\\n") -> ["a", ""]
                | "a return alone" : ("a\\rb") -> ["a\\rb"]

            example reversed
                | "past the basic plane" : ("a𠮷b") -> "b𠮷a"
                | "a mark reversed onto a letter" : ("́e") -> "é"

            example repeated
                | "three" : (3, "ab") -> "ababab"
                | "nought" : (0, "ab") -> ""
                | "below nought" : (-1, "ab") -> ""
                | "past any string, of nothing" : (3000000000, "") -> ""

            example paddedLeft
                | "digits" : (5, "0", "42") -> "00042"
                | "a pad cut to fit" : (4, "xy", "a") -> "xyxa"
                | "wide enough already" : (2, "0", "123") -> "123"
                | "an empty pad" : (9, "", "a") -> "a"

            example paddedRight
                | "a pad cut to fit" : (5, "xy", "a") -> "axyxy"
                | "a pad that composes" : (2, "́", "e") -> "é́"

            example characters
                | "one code point each" : ("a𠮷🇯🇵") -> ["a", "𠮷", "🇯", "🇵"]
                | "nothing" : ("") -> []

            example points
                | "as numbers" : ("a𠮷") -> [97, 134071]
            """;

    private static final String MATCHING = """
            module matching

            behavior postal : (s: String) -> Bool
            let postal (s) = String.matches("[0-9]{3}-[0-9]{4}", s)

            behavior chosen : (s: String) -> Bool
            let chosen (s) = String.matches("(ab|𠮷)*c?", s)

            behavior dotted : (s: String) -> Bool
            let dotted (s) = String.matches("a.b", s)

            behavior classed : (s: String) -> Bool
            let classed (s) = String.matches("\\\\w+\\\\s\\\\d", s)

            behavior ranged : (s: String) -> Bool
            let ranged (s) = String.matches("[^぀-ゟ]{2,3}", s)

            behavior emptied : (s: String) -> Bool
            let emptied (s) = String.matches("(){1048576}", s)

            behavior emptiedUpTo : (s: String) -> Bool
            let emptiedUpTo (s) = String.matches("(){0,1048576}", s)

            behavior emptiedFrom : (s: String) -> Bool
            let emptiedFrom (s) = String.matches("(){1048576,}", s)

            behavior counted : (s: String) -> Bool
            let counted (s) = String.matches("[ab]{300}", s)

            example postal
                | "a postal code" : ("123-4567") -> true
                | "one digit short" : ("123-456") -> false
                | "full-width digits" : ("１２３-４５６７") -> false
                | "something before it" : ("x123-4567") -> false

            example chosen
                | "nothing" : ("") -> true
                | "each arm and the end" : ("ab𠮷c") -> true
                | "half an arm" : ("a") -> false

            example dotted
                | "past the basic plane" : ("a𠮷b") -> true
                | "a newline" : ("a\\nb") -> false

            example classed
                | "a word, a space and a digit" : ("ab_9 7") -> true
                | "an ideographic space" : ("ab　7") -> false

            example emptied
                | "nothing" : ("") -> true
                | "something" : ("a") -> false

            example emptiedFrom
                | "nothing" : ("") -> true
                | "something" : ("a") -> false

            example emptiedUpTo
                | "nothing" : ("") -> true
                | "something" : ("a") -> false

            example counted
                | "exactly as many" : ("%1$s") -> true
                | "one fewer" : ("%2$s") -> false

            example ranged
                | "outside the hiragana" : ("ア𠮷") -> true
                | "one of them hiragana" : ("アあ") -> false
                | "too many" : ("abcd") -> false
            """.formatted("ab".repeat(150), "ab".repeat(150).substring(1));

    private static final String LISTS = """
            module lists

            data Lost
            data Won
            data Qualified
            data Prospecting
            data Open = Prospecting | Qualified
            data Stage = Open | Won | Lost
            data Sku = String

            let stage (n: Int): Stage =
                if n == 0 then Lost
                else if n == 1 then Won
                else if n == 2 then Qualified
                else Prospecting

            let named (s: Stage): Int = match s with
                | Lost -> 0
                | Won -> 1
                | Qualified -> 2
                | Prospecting -> 3

            behavior firstAbove : (xs: List<Int>, floor: Int) -> Int
            let firstAbove (xs, floor) = match List.find(x -> x > floor, xs) with
                | Some x -> x
                | None -> -1

            behavior sorted : (xs: List<Int>) -> List<Int>
            let sorted (xs) = List.sort(xs)

            behavior sortedTexts : (xs: List<String>) -> List<String>
            let sortedTexts (xs) = List.sort(xs)

            behavior sortedStages : (ns: List<Int>) -> List<Int>
            let sortedStages (ns) = List.map(s -> named(s), List.sort(List.map(n -> stage(n), ns)))

            behavior byLength : (xs: List<String>) -> List<String>
            let byLength (xs) = List.sortBy(s -> String.length(s), xs)

            behavior bySku : (xs: List<Int>) -> List<Int>
            let bySku (xs) = List.sortBy(x -> Sku(String.fromInt(x)), xs)

            behavior greatest : (xs: List<Int>) -> Int
            let greatest (xs) = match List.max(xs) with
                | Some x -> x
                | None -> -1

            behavior least : (xs: List<Int>) -> Int
            let least (xs) = match List.min(xs) with
                | Some x -> x
                | None -> -1

            behavior leastText : (xs: List<String>) -> String
            let leastText (xs) = match List.min(xs) with
                | Some x -> x
                | None -> "none"

            behavior greatestStage : (ns: List<Int>) -> Int
            let greatestStage (ns) = match List.max(List.map(n -> stage(n), ns)) with
                | Some s -> named(s)
                | None -> -1

            behavior reversed : (xs: List<Int>) -> List<Int>
            let reversed (xs) = List.reverse(xs)

            behavior total : (xs: List<Int>) -> Int
            let total (xs) = List.sum(xs)

            behavior multiplied : (xs: List<Int>) -> Int
            let multiplied (xs) = List.product(xs)

            behavior ranged : (from: Int, to: Int) -> List<Int>
            let ranged (from, to) = List.rangeInclusive(from, to)

            behavior foldedRight : (xs: List<Int>) -> Int
            let foldedRight (xs) = List.foldRight((x, acc) -> x - acc, 0, xs)

            behavior doubled : (a: Int, at: Int) -> Int
            let doubled (a, at) = match Option.map(x -> x * 2, List.get(at, [a])) with
                | Some x -> x
                | None -> -1

            behavior written : (a: Int, at: Int) -> String
            let written (a, at) = match Option.map(x -> String.fromInt(x), List.get(at, [a])) with
                | Some x -> x
                | None -> "none"

            behavior ofNothing : (a: Int) -> Int
            let ofNothing (a) =
                List.length(List.sort([])) + List.length(List.sortBy(x -> x, []))
                    + List.length(List.reverse([])) + a

            behavior summedNothing : (a: Int) -> Int
            let summedNothing (a) = List.sum([])

            behavior mappedNothing : (a: Int) -> Int
            let mappedNothing (a) = match Option.map(x -> 1, List.get(0, [])) with
                | Some x -> x
                | None -> a

            behavior totalPast : (a: Int) -> Int
            let totalPast (a) = List.sum([a, 1])

            behavior multipliedPast : (a: Int) -> Int
            let multipliedPast (a) = List.product([a, 2])

            behavior foundPast : (a: Int) -> Int
            let foundPast (a) = match List.find(x -> x * 2 > 0, [a, 1]) with
                | Some x -> x
                | None -> -1

            behavior foundBefore : (a: Int) -> Int
            let foundBefore (a) = match List.find(x -> x * 2 > 0, [1, a]) with
                | Some x -> x
                | None -> -1

            behavior sortedPast : (a: Int) -> List<Int>
            let sortedPast (a) = List.sortBy(x -> x * 2, [1, a])

            example firstAbove
                | "the first of two" : ([3, 12, 9, 20], 10) -> 12
                | "none above" : ([1, 2], 5) -> -1
                | "nothing to look at" : ([], 0) -> -1

            example sorted
                | "equal and below nought" : ([5, -3, 9, -3, 0]) -> [-3, -3, 0, 5, 9]
                | "the ends of the range" : ([9223372036854775807, -9223372036854775807, 0]) -> [-9223372036854775807, 0, 9223372036854775807]
                | "one" : ([4]) -> [4]
                | "none" : ([]) -> []

            example sortedTexts
                | "by code point" : (["b", "𠮷", "a", "ｚ", "B"]) -> ["B", "a", "b", "ｚ", "𠮷"]

            example sortedStages
                | "as the enumeration lists them" : ([0, 3, 1, 2, 3]) -> [3, 3, 2, 1, 0]

            example byLength
                | "equal keys in the order they came" : (["dd", "a", "ccc", "b", "ee", "c", "ff", "gg", "h", "iii", "j"]) -> ["a", "b", "c", "h", "j", "dd", "ee", "ff", "gg", "ccc", "iii"]
                | "none" : ([]) -> []

            example bySku
                | "by the text a newtype wraps" : ([10, 9, 100, 1]) -> [1, 10, 100, 9]

            example greatest
                | "among three" : ([3, 9, 2]) -> 9
                | "none" : ([]) -> -1

            example least
                | "below nought" : ([3, -9, 2]) -> -9
                | "none" : ([]) -> -1

            example leastText
                | "by code point" : (["b", "a", "c"]) -> "a"
                | "none" : ([]) -> "none"

            example greatestStage
                | "as the enumeration lists them" : ([1, 3, 2]) -> 1
                | "the last it lists" : ([1, 0, 2]) -> 0
                | "none" : ([]) -> -1

            example reversed
                | "three" : ([1, 2, 3]) -> [3, 2, 1]
                | "none" : ([]) -> []

            example total
                | "three" : ([1, 2, 3]) -> 6
                | "to the largest but one" : ([9223372036854775807, -1]) -> 9223372036854775806
                | "none" : ([]) -> 0

            example multiplied
                | "three" : ([2, 3, 4]) -> 24
                | "none" : ([]) -> 1

            example ranged
                | "four" : (1, 4) -> [1, 2, 3, 4]
                | "one" : (3, 3) -> [3]
                | "backwards" : (4, 1) -> []
                | "at the top of the range" : (9223372036854775806, 9223372036854775807) -> [9223372036854775806, 9223372036854775807]
                | "at the bottom of the range" : (-9223372036854775807, -9223372036854775806) -> [-9223372036854775807, -9223372036854775806]
                | "furthest backwards" : (9223372036854775807, -9223372036854775807) -> []

            example foldedRight
                | "from the end" : ([1, 2, 3]) -> 2

            example doubled
                | "held" : (21, 0) -> 42
                | "not held" : (21, 1) -> -1

            example ofNothing
                | "an empty list of what has no value" : (7) -> 7

            example summedNothing
                | "the seed the position states" : (7) -> 0

            example mappedNothing
                | "an optional of what has no value" : (7) -> 7

            example foundBefore
                | "asks nothing past the first it holds for" : (4611686018427387904) -> 1

            example written
                | "held" : (21, 0) -> "21"
                | "not held" : (21, 1) -> "none"
            """;

    @Test
    void everyIntKernelRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(INT);
    }

    @Test
    void everyStringKernelRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(TEXT);
    }

    /**
     * A pattern is run as what the checker read it as, over scalar values: a character past the
     * basic plane is one character to a class, to {@code .} and to a repetition.
     */
    @Test
    void everyPatternRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(MATCHING);
    }

    @Test
    void everyListAndOptionKernelRowHolds() throws Exception {
        ARowHoldsWhereverItIsRunTest.assertEveryRowHolds(LISTS);
    }

    /**
     * A sum or a product no {@code Int} holds ends the run as {@code +} and {@code *} do, and so
     * does a span longer than a list holds, which is the JVM's longest list. A function a kernel
     * calls that ends the run ends the call with it, as it would where a body applied it.
     */
    @Test
    void aListOrOptionKernelEndsTheRunWhereItsContractOrItsFunctionSays() throws Exception {
        Asked lists = new Asked(LISTS);
        ObservedValue largest = integer(9223372036854775807L);
        ObservedValue past = integer(4611686018427387904L);
        assertThat(lists.outcome("totalPast", largest))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(lists.outcome("multipliedPast", past))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(lists.outcome("ranged", integer(0), integer(2147483647L)))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(lists.outcome("ranged", integer(-9223372036854775808L), largest))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(lists.outcome("doubled", past, integer(0)))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(lists.outcome("foundPast", past))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(lists.outcome("sortedPast", past))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
    }

    /** A difference or a product no {@code Int} holds ends the run, and so does a zero divisor. */
    @Test
    void anIntKernelEndsTheRunWhereItsContractSays() throws Exception {
        Asked ints = new Asked(INT);
        assertThat(ints.outcome("less", integer(-9223372036854775807L), integer(2)))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(ints.outcome("times", integer(4611686018427387904L), integer(2)))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(ints.outcome("floored", integer(1), integer(0)))
                .isEqualTo(ended(AbortKind.DIVISION_BY_ZERO));
    }

    /**
     * An index the string has not got ends a slice, and a count or a width past what any string
     * holds ends a repeat or a pad. Which count that is, is the JVM's
     * (souther-lang/souther#1986).
     */
    @Test
    void aStringKernelEndsTheRunWhereItsContractSays() throws Exception {
        Asked texts = new Asked(TEXT);
        assertThat(texts.outcome("sliced", integer(0), integer(5), text("abcd")))
                .isEqualTo(ended(AbortKind.INVALID_BOUNDS));
        assertThat(texts.outcome("sliced", integer(-1), integer(1), text("a")))
                .isEqualTo(ended(AbortKind.INVALID_BOUNDS));
        assertThat(texts.outcome("sliced", integer(2), integer(1), text("abc")))
                .isEqualTo(ended(AbortKind.INVALID_BOUNDS));
        assertThat(texts.outcome("repeated", integer(3000000000L), text("a")))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(texts.outcome("paddedLeft", integer(3000000000L), text("0"), text("a")))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
        assertThat(texts.outcome("paddedRight", integer(3000000000L), text("0"), text("a")))
                .isEqualTo(ended(AbortKind.REQUIRED_FORM_HAS_NO_PLACE));
    }

    private static ObservedValue integer(long value) {
        return new ObservedValue.Integer(value);
    }

    private static ObservedValue text(String value) {
        return new ObservedValue.Text(value);
    }

    private static RunOutcome ended(AbortKind kind) {
        return new RunOutcome.Aborted(kind);
    }

    /**
     * A program checked once and asked as many questions as a test has: checking one runs every
     * row it states on the JVM, which is paid for each program and not for each question.
     */
    private static final class Asked {

        private final CheckedProgram program;
        private final Running running;

        Asked(String source) {
            this.program = Checked.of(List.of(source));
            this.running = Running.of(program);
        }

        RunOutcome outcome(String behavior, ObservedValue... handed) throws Exception {
            CheckedModule module = program.modules().getFirst();
            CheckedBehavior reached = module.behaviors().stream()
                    .filter(it -> it.name().name().equals(behavior))
                    .findFirst()
                    .orElseThrow(() -> new AssertionError("no behavior " + behavior));
            return running.answeredOrEnded(module, reached, List.of(handed));
        }
    }
}
