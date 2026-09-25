package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
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
 * An attempted construction of a type another build declares asks that build's object which clause
 * did not hold, and takes the arm naming it here.
 *
 * <p>The clauses call a helper the declaring module holds, which does not cross, so a build that
 * attempts the type cannot run them. What crosses is what each clause is answered under, in the
 * order that object runs them, and that is all an arm here is matched against. The two objects are
 * linked to put it to a run.
 */
class AnAttemptOfATypeAnotherBuildDeclaresIsDecidedThereTest {

    private static final String BUILT_BEFORE = """
            module lib.spans exposing ( Span )

            let gap (lo: Int, hi: Int): Int = hi - lo

            data Span = { lo: Int, hi: Int }
                invariant ordered = lo <= hi
                invariant roomy = gap(lo, hi) >= 1
                invariant gap(lo, hi) <= 100
            """;

    private static final String ATTEMPTING_IT = """
            module app.attempts exposing ( measured )
            import lib.spans ( Span )

            behavior measured : (lo: Int, hi: Int) -> Int
            let measured (lo, hi) = {
                guard Span { lo = lo, hi = hi } as s
                    else | roomy -> -2 | ordered -> -1 | _ -> -3
                s.hi - s.lo
            }
            """;

    /**
     * What each clause is answered under crosses, in the order the declaring object runs them, and
     * what they say does not.
     */
    @Test
    void theNamesOfTheClausesCrossAndTheClausesDoNot() {
        String written = ProgramWriter.written(compiled());

        int span = written.indexOf("\"name\":\"Span\"");
        String declaration = written.substring(span, written.indexOf("{\"module\":", span));
        assertThat(declaration)
                .contains("\"by\":\"onthepath\"")
                .contains("\"headers\":[{\"name\":\"ordered\"},{\"name\":\"roomy\"},{\"name\":null}]")
                .doesNotContain("\"invariants\"");
    }

    /** Every clause takes its own arm, decided by the object of the build that declared the type. */
    @Test
    void eachClauseTakesItsArmHereAndIsDecidedThere() throws Exception {
        assertThat(ran(1, 5)).isEqualTo(answered(4));
        assertThat(ran(5, 1)).isEqualTo(answered(-1));
        assertThat(ran(3, 3)).isEqualTo(answered(-2));
        assertThat(ran(0, 500)).isEqualTo(answered(-3));
    }

    private static RunOutcome ran(long lo, long hi) throws Exception {
        CheckedProgram program = compiled();
        byte[] before = NativeArtifacts.object(CheckedProgram.of(List.of(BUILT_BEFORE)));
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior measured = module.behaviors().getFirst();
        return Running.of(program, List.of(before)).answeredOrEnded(module, measured,
                List.of(new ObservedValue.Integer(lo), new ObservedValue.Integer(hi)));
    }

    private static RunOutcome answered(long value) {
        return new RunOutcome.Answered(new ObservedValue.Integer(value));
    }

    private static CheckedProgram compiled() {
        Map<String, ClassFileImage> published = Compiler.compile(BUILT_BEFORE);
        return CheckedProgram.of(List.of(ATTEMPTING_IT), ModulePath.of(published));
    }
}
