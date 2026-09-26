package souther.bindings;

import org.junit.jupiter.api.Test;
import souther.bindings.CrossingShape.Listed;
import souther.bindings.CrossingShape.Present;
import souther.bindings.CrossingShape.Told;
import souther.bindings.CrossingShape.Whole;
import souther.bindings.Manifest.Answer;
import souther.bindings.Manifest.Case;
import souther.bindings.Manifest.Element;
import souther.bindings.Manifest.Function;
import souther.bindings.Manifest.ListCrossing;
import souther.bindings.Manifest.Module;
import souther.bindings.Manifest.Parameter;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.UnionAnswer;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * How a value of a model type crosses, as the library's ABI has it and whatever host it crosses
 * to: the words, the list functions, and which way it crosses at all. Asked of a module made here,
 * since nothing about a shape depends on the program that would have written it.
 */
class AValueCrossesInTheShapeTheAbiGivesItTest {

    private static final Type INT = new Type.Primitive("Int");
    private static final Type.Declared ITEM = new Type.Declared("m", "Item");
    private static final Type.Union EITHER = new Type.Union(List.of(
            new Case.Declared("m", "Item"), new Case.Declared("m", "Other")));

    /** A module saying a list for each of {@code elements}, with functions of the shape it promises. */
    private static Module module(Element... elements) {
        List<ListCrossing> lists = new ArrayList<>();
        for (Element element : elements) {
            List<Parameter> built = new ArrayList<>(List.of(Parameter.given(Word.COUNT)));
            element.words().forEach(word -> built.add(Parameter.slice(word)));
            List<Parameter> at = new ArrayList<>(
                    List.of(Parameter.given(Word.LIST), Parameter.given(Word.COUNT)));
            element.words().forEach(word -> at.add(Parameter.room(word)));
            lists.add(new ListCrossing(element,
                    new Function("construct_" + lists.size(), built, Word.LIST),
                    new Function("length_" + lists.size(), List.of(Parameter.given(Word.LIST)),
                            Word.COUNT),
                    new Function("at_" + lists.size(), at, Word.BOOL)));
        }
        return new Module("m", List.of(), List.of(), List.of(), List.of(), List.of(), lists);
    }

    @Test
    void aPrimitiveADeclaredTypeAndAnOptionalCrossAsTheirWords() {
        Module module = module();

        assertThat(CrossingShape.given(module, INT)).isEqualTo(new Whole(Word.INT, INT));
        assertThat(CrossingShape.received(module, ITEM)).isEqualTo(new Whole(Word.VALUE, ITEM));
        assertThat(CrossingShape.given(module, new Type.Option(INT)))
                .isEqualTo(new Present(new Whole(Word.INT, INT)))
                .extracting(CrossingShape::words).isEqualTo(List.of(Word.BOOL, Word.INT));
    }

    /**
     * What the ABI hands over no way is answered as no way, the same for every host: a primitive
     * it names no word for, an optional inside an optional, and a type no host has a
     * representation for yet.
     */
    @Test
    void whatTheAbiHasNoWordForDoesNotCross() {
        Module module = module();

        assertThat(CrossingShape.given(module, new Type.Primitive("Decimal"))).isNull();
        assertThat(CrossingShape.given(module, new Type.Option(new Type.Option(INT)))).isNull();
        assertThat(CrossingShape.given(module, new Type.Unrepresented("tuple"))).isNull();
        assertThat(CrossingShape.given(module, new Type.Union(List.of(
                new Case.Declared("m", "Item"), new Case.Other("primitive", "Int"))))).isNull();
    }

    /** A list crosses as one word, through the functions the module says for its element. */
    @Test
    void aListCrossesThroughTheFunctionsForItsElement() {
        Element optionalInt = new Element(true, Word.INT);
        Module module = module(new Element(false, Word.VALUE), optionalInt);

        CrossingShape shape = CrossingShape.given(module, new Type.ListOf(new Type.Option(INT)));

        assertThat(shape).isInstanceOf(Listed.class);
        Listed listed = (Listed) shape;
        assertThat(listed.words()).containsExactly(Word.LIST);
        assertThat(listed.element()).isEqualTo(new Present(new Whole(Word.INT, INT)));
        assertThat(listed.crossing().element()).isEqualTo(optionalInt);
    }

    /**
     * A function of a module handing a list across with nothing in the module to build one through
     * is the manifest disagreeing with itself, and is refused rather than taken for a list no host
     * can reach.
     */
    @Test
    void aListWithNothingToBuildItThroughIsRefused() {
        Module module = module(new Element(false, Word.VALUE));

        assertThatThrownBy(() -> CrossingShape.given(module, new Type.ListOf(INT)))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("module `m` nothing to build a list of");
    }

    /**
     * A union no declaration names is handed over as the value, which already is its case, and is
     * handed to a host no way anywhere nothing says which case it is: not alone, not in an
     * optional, and not in a list.
     */
    @Test
    void aUnionCrossesFromAHostAndNotToOne() {
        Module module = module(new Element(false, Word.VALUE));

        assertThat(CrossingShape.given(module, EITHER)).isEqualTo(new Whole(Word.VALUE, EITHER));
        assertThat(CrossingShape.given(module, new Type.Option(EITHER)))
                .isEqualTo(new Present(new Whole(Word.VALUE, EITHER)));
        assertThat(CrossingShape.received(module, EITHER)).isNull();
        assertThat(CrossingShape.received(module, new Type.Option(EITHER))).isNull();
        assertThat(CrossingShape.received(module, new Type.ListOf(EITHER))).isNull();
    }

    /** What a behavior answers says which case a union is, where it names what says so. */
    @Test
    void aUnionABehaviorAnswersIsToldItsCase() {
        Module module = module();
        Function which = new Function("which", List.of(Parameter.given(Word.VALUE)), Word.CASE);

        CrossingShape told = CrossingShape.received(module,
                new Answer(EITHER, new UnionAnswer(EITHER.cases(), which)));

        assertThat(told).isEqualTo(new Told(EITHER, EITHER.cases(), which));
        assertThat(told.words()).containsExactly(Word.VALUE);
        assertThat(CrossingShape.received(module,
                new Answer(EITHER, new UnionAnswer(EITHER.cases(), null)))).isNull();
        assertThat(CrossingShape.received(module, new Answer(EITHER, null))).isNull();
    }

    /** A {@code which} that is not what tells cases apart is two readings of one thing disagreeing. */
    @Test
    void aWhichOfAnotherShapeIsRefused() {
        Function which = new Function("which", List.of(Parameter.given(Word.VALUE)), Word.INT);

        assertThatThrownBy(() -> CrossingShape.received(module(),
                new Answer(EITHER, new UnionAnswer(EITHER.cases(), which))))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("the manifest says which takes");
    }

    /**
     * A behavior's function takes what it was constructed with first where it requires anything,
     * then each parameter's words, and writes its answer through room.
     */
    @Test
    void aCallTakesItsRequirementsThenItsParameters() {
        Whole item = new Whole(Word.VALUE, ITEM);
        Present optional = new Present(new Whole(Word.INT, INT));
        Function call = new Function("call", List.of(Parameter.given(Word.REQUIREMENTS),
                Parameter.given(Word.VALUE), Parameter.given(Word.BOOL), Parameter.given(Word.INT),
                Parameter.room(Word.BOOL), Parameter.room(Word.INT)), Word.STATUS);

        CrossingShape.agreesAsCall(call, true, List.of(item, optional), optional);
        assertThatThrownBy(() -> CrossingShape.agreesAsCall(call, false, List.of(item, optional),
                optional))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("this generator would call it with");
    }
}
