package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.ValueName;
import tools.jackson.databind.json.JsonMapper;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value built in one object and written out by another is written as the case it is.
 *
 * <p>Which case a value is, is the address of its declaration's token, and the linker resolves one
 * address for both objects; that is what tells the writing object which case it holds. What the
 * case is written as — its name, the tag's key, its fields — is the declaration's, which crossed
 * with the program this object was built from. The address says which case, and nothing about how
 * it is spelt.
 */
class ACaseBuiltElsewhereIsWrittenAsItsOwnCaseTest {

    private static final JsonMapper JSON = JsonMapper.builder().build();

    private static final String BUILT_BEFORE = """
            module lib.porch exposing ( Door, Open, Closed, opened, shut )

            data Closed
            data Open = { since: Int }
            data Door = Open | Closed

            behavior opened : (n: Int) -> Door
                constructs Open

            let opened (n) = Open { since = n }

            behavior shut : (n: Int) -> Door
            let shut (n) = Closed
            """;

    private static final String RELAYING = """
            module app.relay exposing ( relay )
            import lib.porch ( Door, opened, shut )

            behavior relay : (n: Int, open: Bool) -> Door
            let relay (n, open) = if open then opened(n) else shut(n)
            """;

    @Test
    void aCaseBuiltInAnotherObjectIsWrittenAsTheCaseItIs() throws Exception {
        CheckedProgram program = Checked.of(List.of(RELAYING), path());
        byte[] before = NativeArtifacts.object(Checked.of(List.of(BUILT_BEFORE)));

        Running running = Running.of(program, List.of(before));
        CheckedModule module = program.modules().getFirst();
        CheckedBehavior relay = module.behavior(new ValueName.Behavior("app.relay", "relay"));

        assertThat(running.externalAnswer(module, relay,
                List.of(new ObservedValue.Integer(5), new ObservedValue.Bool(true))))
                .isEqualTo(JSON.readTree("{\"type\":\"Open\",\"since\":5}"));
        assertThat(running.externalAnswer(module, relay,
                List.of(new ObservedValue.Integer(5), new ObservedValue.Bool(false))))
                .isEqualTo(JSON.readTree("{\"type\":\"Closed\"}"));
    }

    private static ModulePath path() {
        Map<String, ClassFileImage> published = Compiler.compile(BUILT_BEFORE);
        return ModulePath.of(published);
    }
}
