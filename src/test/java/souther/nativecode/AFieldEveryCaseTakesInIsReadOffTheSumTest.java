package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.ValueName;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A field every case of a sum takes in by spread, read off a value of the sum without first
 * telling which case it is.
 *
 * <p>The sum lays out no fields of its own, and each case lays out its own. So the read is of
 * whichever case the value is, where that case lays the field out, and each case is run here, a
 * case of a sum the sum takes in included. The checker lays a field taken in by spread first in
 * every case, and the read does not rest on that: a field standing at a different place in each
 * case is run from a document written by hand (`native/crates/compiler/tests/restating.rs`).
 */
class AFieldEveryCaseTakesInIsReadOffTheSumTest {

    private static final String SOURCE = """
            module shapes exposing ( measured, Sized, Round, Square, Tri, Angular, Shape )

            data Sized = { size: Int }
            data Round = { ...Sized, across: Int }
            data Square = { side: Int, ...Sized }
            data Tri = { flat: Bool, pointed: Bool, ...Sized }
            data Angular = Square | Tri
            data Shape = Round | Angular

            behavior measured : (kind: Int, n: Int) -> Int
            let measured (kind, n) = {
                let shape: Shape = if kind == 0 then Round { size = n, across = 1 }
                    else if kind == 1 then Square { side = 2, size = n + 1 }
                    else Tri { flat = true, pointed = false, size = n + 2 }
                shape.size
            }
            """;

    @Test
    void theFieldIsReadWhereTheCaseTheValueIsLaysItOut() throws Exception {
        CheckedProgram program = Checked.of(List.of(SOURCE));
        Running running = Running.of(program);
        CheckedModule module = program.modules().getFirst();
        var measured = module.behavior(new ValueName.Behavior("shapes", "measured"));

        for (long kind = 0; kind < 3; kind++) {
            assertThat(running.answering(module, measured,
                    List.of(new ObservedValue.Integer(kind), new ObservedValue.Integer(10))))
                    .as("the case numbered %d", kind)
                    .isEqualTo(new ObservedValue.Integer(10 + kind));
        }
    }
}
