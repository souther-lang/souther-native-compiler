package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.ValueName;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a native run writes an answer as, held against what the language says it is written as.
 *
 * <p>Compared as JSON values and not as text. Which members an object has, what they hold, and
 * whether a key is there at all is what the language decides; where whitespace goes and how a
 * character is escaped is not something it states, and a comparison of bytes would make it one.
 */
class AnAnswerCrossesInTheFormItsPositionDeclaresTest {

    private static final JsonMapper JSON = JsonMapper.builder().build();

    private static final String DOORS = """
            module doors exposing ( closedAlone, holding, doorOf, phaseOf, porchOf, customer,
                                    placeOrder, noteOf, flagOf, chainOf, rankOf, bill, lookUp, echoInt, echoBool,
                                    echoText, doorsOf, linksOf, lengthOf, flaggedOf, namedOf,
                                    doubledLength : Int | NotFound,
                                    Closed, Open, Door, Phase, Pending, Holder, Porch, CustomerId,
                                    Order, Noted, Flagged, Chain, Links, Manager, Staff, Rank, Issued,
                                    UnknownSku, Missing, NotFound )

            data Closed
            data Open = { since: Int }
            data Door = Open | Closed
            data Phase = Pending | Closed
            data Holder = { state: Closed }
            data Porch = { door: Door, phase: Phase }

            data CustomerId = String
            data Order = { id: Int, paid: Bool, note: String }
            data Noted = { note: String?, count: Int }
            data Flagged = { on: Bool? }
            data Chain = { n: Int, next: Chain? }
            data Links = { chains: List<Chain>, count: Int }

            data Manager = Int
            data Staff
            data Rank = Manager | Staff

            data Issued = { amount: Int }

            behavior closedAlone : (n: Int) -> Closed
            let closedAlone (n) = Closed

            behavior holding : (n: Int) -> Holder
                constructs Holder

            let holding (n) = Holder { state = Closed }

            behavior doorOf : (n: Int) -> Door
                constructs Open

            let doorOf (n) = if n > 0 then Open { since = n } else Closed

            behavior phaseOf : (n: Int) -> Phase
            let phaseOf (n) = if n > 0 then Pending else Closed

            behavior porchOf : (n: Int) -> Porch
                constructs Porch, Open

            let porchOf (n) = Porch { door = Open { since = n }, phase = Pending }

            behavior customer : (s: String) -> CustomerId
                constructs CustomerId

            let customer (s) = CustomerId(s)

            behavior placeOrder : (id: Int, paid: Bool, note: String) -> Order
                constructs Order

            let placeOrder (id, paid, note) = Order { id = id, paid = paid, note = note }

            behavior noteOf : (has: Bool, note: String) -> Noted
                constructs Noted

            let noteOf (has, note) =
                if has then Noted { note = note, count = 1 } else Noted { note = None, count = 0 }

            behavior flagOf : (has: Bool, on: Bool) -> Flagged
                constructs Flagged

            let flagOf (has, on) = if has then Flagged { on = on } else Flagged { on = None }

            behavior chainOf : (n: Int) -> Chain
                constructs Chain

            let chainOf (n) = Chain { n = n, next = Chain { n = n + 1, next = None } }

            behavior doorsOf : (n: Int) -> List<Door>
                constructs Open

            let doorsOf (n) = [Open { since = n }, Closed, Open { since = n + 1 }]

            behavior linksOf : (n: Int) -> Links
                constructs Links, Chain

            let linksOf (n) = Links {
                chains = [Chain { n = n, next = Chain { n = n + 1, next = None } }, Chain { n = 0, next = None }],
                count = 2
            }

            behavior rankOf : (level: Int) -> Rank
                constructs Manager

            let rankOf (level) = if level > 0 then Manager(level) else Staff

            behavior bill : (n: Int) -> Issued | UnknownSku
                constructs Issued

            let bill (n) = if n > 0 then Issued { amount = n } else UnknownSku

            behavior lookUp : (n: Int) -> Door | Missing
                constructs Open

            let lookUp (n) = if n > 1 then Open { since = n } else if n > 0 then Closed else Missing

            data NotFound

            behavior lengthOf : (n: Int) -> Int | NotFound
            let lengthOf (n) = if n > 0 then n else NotFound

            behavior doubled : (n: Int) -> Int
            let doubled (n) = n * 2

            behavior doubledLength = lengthOf >-> doubled

            behavior flaggedOf : (n: Int) -> Bool | NotFound
            let flaggedOf (n) = if n > 0 then n > 1 else NotFound

            behavior namedOf : (s: String) -> String | NotFound
            let namedOf (s) = if s == "" then NotFound else s

            behavior echoInt : (n: Int) -> Int
            let echoInt (n) = n

            behavior echoBool : (b: Bool) -> Bool
            let echoBool (b) = b

            behavior echoText : (s: String) -> String
            let echoText (s) = s
            """;

    /**
     * One unit, four forms, and the form is never the unit's to decide.
     *
     * <p>{@code Closed} is the same native value in every one of these: a token and nothing else.
     * What it is written as is decided by where it stands — on its own, as a field, as a case of a
     * sum where another case carries fields, as a case of a sum where every case is a unit — and
     * each of those is a decision the checker made and this backend carried.
     */
    @Test
    void oneUnitIsWrittenFourWaysByThePositionItStandsIn() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "closedAlone", integer(0)))
                .isEqualTo(json("{}"));
        assertThat(answer(running, program, "holding", integer(0)))
                .isEqualTo(json("{\"state\":{}}"));
        assertThat(answer(running, program, "doorOf", integer(0)))
                .isEqualTo(json("{\"type\":\"Closed\"}"));
        assertThat(answer(running, program, "phaseOf", integer(0)))
                .isEqualTo(json("\"Closed\""));
    }

    @Test
    void aProductCaseTakesTheTagBesideItsOwnFields() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "doorOf", integer(7)))
                .isEqualTo(json("{\"type\":\"Open\",\"since\":7}"));
        assertThat(answer(running, program, "phaseOf", integer(1)))
                .isEqualTo(json("\"Pending\""));
    }

    /** A field of a sum's type is written as that sum is, whichever of its forms it has. */
    @Test
    void aFieldIsWrittenAsItsOwnDeclarationIs() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "porchOf", integer(3)))
                .isEqualTo(json("{\"door\":{\"type\":\"Open\",\"since\":3},\"phase\":\"Pending\"}"));
    }

    @Test
    void aNewtypeIsWrittenAsWhatItWraps() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "customer", text("c-42")))
                .isEqualTo(json("\"c-42\""));
    }

    /** A truth held in a field is a slot wide, and is written as a truth and not as the slot. */
    @Test
    void aProductIsAnObjectOfItsFields() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "placeOrder",
                integer(12), new ObservedValue.Bool(true), text("rush")))
                .isEqualTo(json("{\"id\":12,\"paid\":true,\"note\":\"rush\"}"));
        assertThat(answer(running, program, "placeOrder",
                integer(-1), new ObservedValue.Bool(false), text("")))
                .isEqualTo(json("{\"id\":-1,\"paid\":false,\"note\":\"\"}"));
    }

    /** A field holding nothing is not there at all, and one holding something is what it holds. */
    @Test
    void anAbsentOptionalFieldIsLeftOut() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "noteOf", new ObservedValue.Bool(false), text("x")))
                .isEqualTo(json("{\"count\":0}"));
        assertThat(answer(running, program, "noteOf", new ObservedValue.Bool(true), text("x")))
                .isEqualTo(json("{\"note\":\"x\",\"count\":1}"));
    }

    /**
     * A truth held under an optional is a slot wide in the box that holds it, and is read back to a
     * truth before it is written: {@code true} and {@code false}, never the slot.
     */
    @Test
    void aTruthHeldUnderAnOptionalFieldIsWrittenAsATruth() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "flagOf",
                new ObservedValue.Bool(true), new ObservedValue.Bool(true)))
                .isEqualTo(json("{\"on\":true}"));
        assertThat(answer(running, program, "flagOf",
                new ObservedValue.Bool(true), new ObservedValue.Bool(false)))
                .isEqualTo(json("{\"on\":false}"));
        assertThat(answer(running, program, "flagOf",
                new ObservedValue.Bool(false), new ObservedValue.Bool(true)))
                .isEqualTo(json("{}"));
    }

    /** A declaration that holds itself is written by the one encoder, as deep as the value is. */
    @Test
    void aDeclarationThatHoldsItselfIsWrittenAsDeepAsTheValueIs() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "chainOf", integer(1)))
                .isEqualTo(json("{\"n\":1,\"next\":{\"n\":2}}"));
    }

    /**
     * An answer that is a list of a declared type is an array of each element written as that
     * type, in the order the list holds them, with nothing of one element standing in another's.
     */
    @Test
    void aListOfADeclaredTypeIsWrittenElementByElementInOrder() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "doorsOf", integer(4)))
                .isEqualTo(json("[{\"type\":\"Open\",\"since\":4},{\"type\":\"Closed\"},"
                        + "{\"type\":\"Open\",\"since\":5}]"));
    }

    /**
     * A field holding a list of a declared type is put once every element is in the array, and a
     * field laid out after it is put after it.
     */
    @Test
    void aFieldAfterAListOfADeclaredTypeIsPutAfterIt() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        JsonNode written = answer(running, program, "linksOf", integer(1));
        assertThat(written).isEqualTo(json("{\"chains\":[{\"n\":1,\"next\":{\"n\":2}},{\"n\":0}],"
                + "\"count\":2}"));
        assertThat(written.propertyNames()).containsExactly("chains", "count");
    }

    /**
     * A newtype case cannot hold the tag in a value of its own, so it stands under the contents key
     * beside it; a unit case of the same sum takes the tag into its empty object.
     */
    @Test
    void aNewtypeCaseIsWrappedBesideTheTag() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "rankOf", integer(3)))
                .isEqualTo(json("{\"type\":\"Manager\",\"value\":3}"));
        assertThat(answer(running, program, "rankOf", integer(0)))
                .isEqualTo(json("{\"type\":\"Staff\"}"));
    }

    @Test
    void anAnswerNobodyNamedIsDiscriminatedLikeASumOverTheSameCases() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "bill", integer(200)))
                .isEqualTo(json("{\"type\":\"Issued\",\"amount\":200}"));
        assertThat(answer(running, program, "bill", integer(0)))
                .isEqualTo(json("{\"type\":\"UnknownSku\"}"));
    }

    /**
     * A sum standing as a member of an answer nobody named is walked into: its cases are the
     * answer's cases, each tagged with its own name, and the sum's name is written nowhere.
     */
    @Test
    void aSumAmongAnAnswersMembersIsWrittenAsItsOwnCases() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "lookUp", integer(4)))
                .isEqualTo(json("{\"type\":\"Open\",\"since\":4}"));
        assertThat(answer(running, program, "lookUp", integer(1)))
                .isEqualTo(json("{\"type\":\"Closed\"}"));
        assertThat(answer(running, program, "lookUp", integer(0)))
                .isEqualTo(json("{\"type\":\"Missing\"}"));
    }

    /**
     * A primitive standing as a member of an answer has no object of its own to take the tag into,
     * so it stands under the contents key beside it, the way a newtype case does, and is written as
     * the primitive it is.
     */
    @Test
    void aPrimitiveMemberIsWrittenUnderTheContentsKeyBesideItsName() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "lengthOf", integer(4)))
                .isEqualTo(json("{\"type\":\"Int\",\"value\":4}"));
        assertThat(answer(running, program, "lengthOf", integer(0)))
                .isEqualTo(json("{\"type\":\"NotFound\"}"));
        assertThat(answer(running, program, "flaggedOf", integer(1)))
                .isEqualTo(json("{\"type\":\"Bool\",\"value\":false}"));
        assertThat(answer(running, program, "flaggedOf", integer(2)))
                .isEqualTo(json("{\"type\":\"Bool\",\"value\":true}"));
        assertThat(answer(running, program, "namedOf", text("n")))
                .isEqualTo(json("{\"type\":\"String\",\"value\":\"n\"}"));
        assertThat(answer(running, program, "namedOf", text("")))
                .isEqualTo(json("{\"type\":\"NotFound\"}"));
    }

    /**
     * A stage accepting the primitive case of what runs is handed it read back out of what carries
     * it, and what it answers is carried again as the composition's answer; the case it does not
     * accept leaves as it came.
     */
    @Test
    void aStageAcceptingAPrimitiveCaseIsHandedThePrimitive() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "doubledLength", integer(21)))
                .isEqualTo(json("{\"type\":\"Int\",\"value\":42}"));
        assertThat(answer(running, program, "doubledLength", integer(0)))
                .isEqualTo(json("{\"type\":\"NotFound\"}"));
    }

    @Test
    void theEndsOfAnIntAreWrittenWhole() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        assertThat(answer(running, program, "echoInt", integer(Long.MIN_VALUE)))
                .isEqualTo(json(Long.toString(Long.MIN_VALUE)));
        assertThat(answer(running, program, "echoInt", integer(Long.MAX_VALUE)))
                .isEqualTo(json(Long.toString(Long.MAX_VALUE)));
        assertThat(answer(running, program, "echoBool", new ObservedValue.Bool(true)))
                .isEqualTo(json("true"));
    }

    /** Whatever a string holds reads back as that string once the JSON is read as JSON. */
    @Test
    void aStringIsWrittenSoThatItReadsBackAsItself() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of(DOORS));
        Running running = Running.of(program);
        for (String said : List.of("", "a\"b\\c", "line\nbreak\ttab\rreturn", "\u0001\u001f",
                "𠮷￥", "nought\u0000after")) {
            assertThat(answer(running, program, "echoText", text(said)).stringValue())
                    .as("the string %s", said)
                    .isEqualTo(said);
        }
    }

    private static JsonNode answer(Running running, CheckedProgram program, String name,
                                   ObservedValue... inputs) throws Exception {
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior behavior = module.behavior(new ValueName.Behavior(module.name(), name));
        return running.externalAnswer(module, behavior, List.of(inputs));
    }

    private static JsonNode json(String written) {
        return JSON.readTree(written);
    }

    private static ObservedValue integer(long value) {
        return new ObservedValue.Integer(value);
    }

    private static ObservedValue text(String value) {
        return new ObservedValue.Text(value);
    }
}
