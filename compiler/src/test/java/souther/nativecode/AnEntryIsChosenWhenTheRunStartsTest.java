package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.StandsIn;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What is built of a program is built once, however many entries are run and however many
 * {@link Running}s ask.
 *
 * <p>Held as a count of what was built and not as a time, so that a slow machine cannot make it
 * pass and a fast one cannot make a regression look harmless. The program is one no other test
 * builds, so the counts are its own.
 */
class AnEntryIsChosenWhenTheRunStartsTest {

    private static final String SOURCE = """
            module entriesChosenLate exposing ( first, second )

            behavior first : (a: Int) -> Int
            let first (a) = a + 1

            behavior second : (a: Int, b: Int) -> Int
            let second (a, b) = a * b

            example first
                | "one" : (1) -> 2
                | "two" : (2) -> 3

            example second
                | "six" : (2, 3) -> 6
            """;

    @Test
    void anyNumberOfEntriesOfOneProgramIsOneCompileAndOneLink() throws Exception {
        CheckedProgram program = Checked.of(List.of(SOURCE));
        CheckedModule module = program.modules().stream()
                .filter(it -> it.name().equals("entriesChosenLate")).findFirst().orElseThrow();

        Running first = Running.of(program);
        assertThat(first.answering(module, behavior(module, "first"),
                List.of(new ObservedValue.Integer(1)))).isEqualTo(new ObservedValue.Integer(2));
        assertThat(first.rowAnswering(module, behavior(module, "first"), 0, List.of()))
                .isEqualTo(new ObservedValue.Integer(2));
        assertThat(first.rowAnswering(module, behavior(module, "first"), 1, List.of()))
                .isEqualTo(new ObservedValue.Integer(3));

        // Another Running of the same program, so that what is kept is not kept by the first.
        // and of a program checked again from the same source, so that what is kept is kept by
        // what the program says and not by which object happens to hold it.
        CheckedProgram again = Checked.of(List.of(SOURCE));
        CheckedModule moduleAgain = again.modules().stream()
                .filter(it -> it.name().equals("entriesChosenLate")).findFirst().orElseThrow();
        module = moduleAgain;
        Running second = Running.of(again);
        assertThat(second.answering(module, behavior(module, "second"),
                List.of(new ObservedValue.Integer(4), new ObservedValue.Integer(5))))
                .isEqualTo(new ObservedValue.Integer(20));
        assertThat(second.rowAnswering(module, behavior(module, "second"), 0, List.of()))
                .isEqualTo(new ObservedValue.Integer(6));

        assertThat(NativeArtifacts.compilationsOf(program)).isEqualTo(1);
        assertThat(NativeArtifacts.linksOf(program)).isEqualTo(1);
    }

    @Test
    void whatIsKeptIsNotChangedByAnArrayAnyoneStillHolds() throws Exception {
        CheckedProgram program = Checked.of(List.of(SOURCE.replace(
                "entriesChosenLate", "entriesKeptApart")));

        byte[] handedOut = NativeArtifacts.object(program);
        byte[] before = handedOut.clone();
        java.util.Arrays.fill(handedOut, (byte) 0);
        assertThat(NativeArtifacts.object(program)).isEqualTo(before);

        // An array given to Running is copied when it is given, so the key and the link are of
        // the same bytes even if the caller writes to it afterwards.
        byte[] beside = before.clone();
        Running running = Running.of(program, List.of(beside));
        java.util.Arrays.fill(beside, (byte) 0);
        assertThat(running).isNotNull();
        assertThat(NativeArtifacts.object(program)).isEqualTo(before);
    }

    private static CheckedBehavior behavior(CheckedModule module, String name) {
        return module.behaviors().stream().filter(it -> it.name().name().equals(name))
                .findFirst().orElseThrow();
    }
}
