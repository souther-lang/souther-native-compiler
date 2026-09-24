package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedImplementation;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.ValueName;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value published by a module this compile never checked — one this backend read off the path,
 * built in an object of its own, and links against — reached the way {@link
 * ABehaviorAnotherBuildImplementsTest} reaches a behavior another build implements.
 *
 * <p>Every earlier test of a value ({@code AValueThatHandsOverAnotherValueCrossesTest},
 * {@code APublishedValueCrossesAsItsOwnReachTest}, and the Rust side's {@code values.rs}) put the
 * declaring and the reading module in one document, one object. `reachable.is_published` then
 * finds the entry already declared `Export` by that same object's own declare phase, and the
 * `Linkage::Import` branch this backend added for a genuinely foreign published value — the one
 * that answers issue #10's "a publisher whose value names something a reader could not reach is
 * refused there" for a *reader* built later, from a different document — never runs. This is the
 * one that puts it through: {@code publisher} is compiled to its own object first, and {@code
 * reader} is checked against it off the path and compiled to a second object that imports
 * {@code publisher}'s entry, with nothing about {@code publisher} in {@code reader}'s own
 * document.
 */
class AValuePublishedByAnotherBuildIsReachedTest {

    private static final String PUBLISHER = """
            module publisher exposing ( Box, ys )

            data Box = { n: Int }

            let ks = Box { n = 42 }

            let ys = ks
            """;

    private static final String READER = """
            module reader exposing ( g )
            import publisher ( Box, ys )

            behavior g : () -> Int
            let g = ys.n
            """;

    /** What the setup rests on: `reader`'s own document never checked `publisher`, so nothing of
     *  it is among what this compile emits — the same premise
     *  {@link ABehaviorAnotherBuildImplementsTest#theModuleHoldingItIsNotOneThisCompileEmits}
     *  states for a behavior. */
    @Test
    void theModulePublishingItIsNotOneThisCompileEmits() {
        CheckedProgram program = compiledReader();

        assertThat(program.modules().stream().map(CheckedModule::name).toList())
                .containsExactly("reader");
        assertThat(program.module("reader")
                .behavior(new ValueName.Behavior("reader", "g"))
                .implementation())
                .isInstanceOf(CheckedImplementation.Body.class);
    }

    /**
     * `publisher` is built to an object of its own, `reader` to a second that names `publisher`'s
     * value and defines nothing for it, and the linker is handed both — the only way either object
     * could answer the question by itself is by holding a copy of the other, which a value has
     * exactly one executable home to rule out (ADR-0074).
     */
    @Test
    void aValuePublishedByAnotherBuildIsReachedThroughItsEntry() throws Exception {
        CheckedProgram publisherProgram = CheckedProgram.of(List.of(PUBLISHER));
        byte[] publisherObject = NativeArtifacts.object(publisherProgram);
        CheckedProgram readerProgram = compiledReader();

        Running running = Running.of(readerProgram, List.of(publisherObject));
        CheckedModule reader = readerProgram.module("reader");
        var g = reader.behavior(new ValueName.Behavior("reader", "g"));

        assertThat(running.answering(reader, g, List.of()))
                .isEqualTo(new ObservedValue.Integer(42));
    }

    private static CheckedProgram compiledReader() {
        Map<String, ClassFileImage> published = Compiler.compile(PUBLISHER);
        return CheckedProgram.of(List.of(READER), ModulePath.of(published));
    }
}
