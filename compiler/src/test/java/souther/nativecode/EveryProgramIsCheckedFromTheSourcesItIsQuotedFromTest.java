package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;

import java.io.IOException;
import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassModel;
import java.lang.classfile.CodeModel;
import java.lang.classfile.MethodModel;
import java.lang.classfile.instruction.InvokeInstruction;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What this compiler checks a program from is the sources it read, held as Souther holds them.
 *
 * <p>A program checked from bare texts leaves whoever says why it was refused to work out which text
 * each diagnostic is about and what to call it, and a command that worked it out again was a command
 * that said a refusal as one line with no place (#154), and refused a lone file with no header that
 * {@code souther compile} takes. Kept here over the compiled classes, so that a new way into the
 * compiler is held to it whatever its source looks like.
 *
 * <p>Only the call is held here, which is what a class file can say. Whether a refusal is quoted
 * from the sources it was checked from is a matter of which value reaches the renderer, and that is
 * asked of the command itself in {@link ARefusedProgramIsReportedWhereItIsWrongTest}.
 */
class EveryProgramIsCheckedFromTheSourcesItIsQuotedFromTest {

    private static final String CHECKED_PROGRAM = CheckedProgram.class.getName().replace('.', '/');

    @Test
    void noProgramIsCheckedFromBareTexts() throws Exception {
        List<String> found = new ArrayList<>();
        for (Owned method : methods()) {
            for (InvokeInstruction call : calls(method.code())) {
                if (call.owner().asInternalName().equals(CHECKED_PROGRAM)
                        && call.name().equalsString("of")
                        && call.type().stringValue().startsWith("(Ljava/util/List;")) {
                    found.add(method.named());
                }
            }
        }

        assertThat(found).as("checked from texts with no names, in").isEmpty();
    }

    private record Owned(String named, CodeModel code) {
    }

    private static List<InvokeInstruction> calls(CodeModel code) {
        return code.elementStream().filter(InvokeInstruction.class::isInstance)
                .map(InvokeInstruction.class::cast).toList();
    }

    private static List<Owned> methods() throws IOException {
        Path classes = souther.bindings.Classes.of(Main.class);
        List<Owned> methods = new ArrayList<>();
        try (Stream<Path> walked = Files.walk(classes)) {
            for (Path file : walked.filter(it -> it.toString().endsWith(".class")).toList()) {
                ClassModel model = ClassFile.of().parse(file);
                for (MethodModel method : model.methods()) {
                    method.code().ifPresent(code -> methods.add(new Owned(
                            model.thisClass().asInternalName().replace('/', '.') + "."
                                    + method.methodName().stringValue(), code)));
                }
            }
        }
        assertThat(methods).as("methods under " + classes).hasSizeGreaterThan(100);
        return methods;
    }
}
