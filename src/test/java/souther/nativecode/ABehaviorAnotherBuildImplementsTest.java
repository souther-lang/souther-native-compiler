package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.program.CheckedImplementation;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.ValueName;
import souther.nativecode.transport.ProgramWriter;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A behavior this program reaches and no module of it declares.
 *
 * <p>The module holding it was built before and is read off the path, so it is not among the
 * modules this compile emits. What a call can reach is therefore wider than what is emitted, and a
 * document taking one list for both leaves a call reaching a name nothing in it says anything
 * about.
 */
class ABehaviorAnotherBuildImplementsTest {

    private static final String BUILT_BEFORE = """
            module lib.rates exposing ( Rate, spin, tally, twice )

            data Rate = Int

            behavior spin : (of: Int) -> Rate
            let spin (of) = Rate(of)

            behavior tally : (r: Rate) -> Int
            let tally (r) = r.value

            behavior twice : (a: Int) -> Int
            let twice (a) = a * 2
            """;

    private static final String REACHING_IT = """
            module app.uses
            import lib.rates ( Rate, spin, tally )

            behavior counted : (base: Int) -> Int
            let counted (base) = tally(spin(base))
            """;

    /** What the setup rests on, asserted rather than assumed. */
    @Test
    void theModuleHoldingItIsNotOneThisCompileEmits() {
        CheckedProgram program = compiled();

        assertThat(program.modules().stream().map(CheckedModule::name).toList())
                .containsExactly("app.uses");
        assertThat(program.behavior(new ValueName.Behavior("lib.rates", "spin")).implementation())
                .isInstanceOf(CheckedImplementation.ImplementedElsewhere.class);
    }

    @Test
    void itCrossesWithTheSignatureACallerReachesItBy() {
        String written = ProgramWriter.written(compiled());

        assertThat(written).contains("\"module\":\"lib.rates\",\"name\":\"spin\",\"is\":\"elsewhere\"");
        assertThat(written).contains("\"module\":\"lib.rates\",\"name\":\"tally\",\"is\":\"elsewhere\"");
        // And what it takes and answers, which is a declaration no module here holds either.
        assertThat(written).contains("\"declared\":\"lib.rates.Rate\"");
    }

    /**
     * What it takes and answers is a value of a declared type, whose number for that type this
     * object counted. Two objects exchanging one would compare numbers that were never about each
     * other, both valid and neither complaining, so the call is refused until a declared type has
     * an identity a linker settles.
     */
    @Test
    void aValueOfADeclaredTypeDoesNotCrossOutOfTheObjectItWasBuiltIn() {
        assertThatThrownBy(() -> NativeCompiler.compile(compiled()))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("lib.rates.Rate");
    }

    /**
     * What does cross is what is nobody's count. The object names the behavior and defines nothing
     * for it, so what answers it is settled by whoever links the two objects.
     */
    @Test
    void aBehaviorAnotherBuildImplementsOverNumbersAloneIsReached() throws Exception {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module app.plainly
                import lib.rates ( twice )

                behavior fourTimes : (base: Int) -> Int
                let fourTimes (base) = twice(twice(base))
                """), path());

        assertThat(ProgramWriter.written(program))
                .contains("\"module\":\"lib.rates\",\"name\":\"twice\",\"is\":\"elsewhere\"");
        assertThat(NativeCompiler.compile(program)).isNotEmpty();
    }

    private static CheckedProgram compiled() {
        return CheckedProgram.of(List.of(REACHING_IT), path());
    }

    private static ModulePath path() {
        Map<String, ClassFileImage> published = Compiler.compile(BUILT_BEFORE);
        return ModulePath.of(published);
    }
}
