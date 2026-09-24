package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.abort.AbortKind;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value of a type another build declares is built by that build's object, which runs the type's
 * clauses with everything they were checked against.
 *
 * <p>The clause here calls a recursive helper the declaring module holds. That helper is the
 * declaring build's own: nothing of it crosses to a build that imports the type, so a construction
 * there that ran a copy of the clause would be running it without what it calls. It reaches the
 * constructor the declaring object defines instead, and the two objects are linked here to put
 * that to the run.
 */
class AValueOfATypeAnotherBuildDeclaresIsBuiltThereTest {

    private static final String BUILT_BEFORE = """
            module lib.depths exposing ( Peano, Zero, Succ, Shallow, depth )

            data Zero
            data Succ = { pred: Peano }
            data Peano = Zero | Succ

            let depth (p: Peano): Int = match p with
                | Zero -> 0
                | Succ as s -> 1 + depth(s.pred)

            data Shallow = { p: Peano }
                invariant shallow = depth(p) <= 2
            """;

    private static final String BUILDING_IT = """
            module app.builds exposing ( held )
            import lib.depths ( Peano, Zero, Succ, Shallow )

            behavior held : (deep: Bool) -> Int
            let held (deep) = {
                let p: Peano = if deep
                    then Succ { pred = Succ { pred = Succ { pred = Zero } } }
                    else Succ { pred = Zero }
                let s = Shallow { p = p }
                match s.p with
                    | Zero -> 0
                    | Succ -> 1
            }
            """;

    /** The clauses of a type this compile did not declare are not what this compile runs. */
    @Test
    void theClausesOfATypeAnotherBuildDeclaresDoNotCross() {
        String written = ProgramWriter.written(compiled());

        assertThat(written).contains("\"name\":\"Shallow\",\"by\":\"onthepath\"");
        int shallow = written.indexOf("\"name\":\"Shallow\"");
        String declaration = written.substring(shallow, written.indexOf("{\"module\":", shallow));
        assertThat(declaration).doesNotContain("\"invariants\"");
        assertThat(ProgramWriter.written(builtBefore()))
                .contains("\"name\":\"Shallow\",\"by\":\"amodule\"")
                .contains("\"invariants\":[{\"name\":\"shallow\"");
    }

    /**
     * Built here by the constructor the declaring object defines: a value the clause holds of is
     * answered, and one it does not ends the run the way it does where the type is declared.
     */
    @Test
    void aConstructionReachesTheConstructorOfTheBuildThatDeclaresTheType() throws Exception {
        CheckedProgram program = compiled();
        byte[] before = NativeCompiler.compile(builtBefore());
        try (Running running = Running.of(program, List.of(before))) {
            CheckedModule module = program.modules().getFirst();
            CheckedBehavior held = module.behaviors().getFirst();

            assertThat(running.answeredOrEnded(module, held,
                    List.of(new ObservedValue.Bool(false))))
                    .isEqualTo(new RunOutcome.Answered(new ObservedValue.Integer(1)));
            assertThat(running.answeredOrEnded(module, held,
                    List.of(new ObservedValue.Bool(true))))
                    .isEqualTo(new RunOutcome.Aborted(AbortKind.INVARIANT_NOT_HELD));
        }
    }

    private static CheckedProgram compiled() {
        Map<String, ClassFileImage> published = Compiler.compile(BUILT_BEFORE);
        return CheckedProgram.of(List.of(BUILDING_IT), ModulePath.of(published));
    }

    private static CheckedProgram builtBefore() {
        return CheckedProgram.of(List.of(BUILT_BEFORE));
    }
}
