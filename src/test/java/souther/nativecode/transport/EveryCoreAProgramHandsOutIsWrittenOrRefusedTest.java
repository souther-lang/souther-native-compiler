package souther.nativecode.transport;

import org.junit.jupiter.api.Test;
import souther.compiler.core.Core;
import souther.compiler.program.CheckedProgram;

import java.lang.reflect.Method;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.lang.reflect.WildcardType;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every place a checked program hands out {@code Core} is one this writer carries across or
 * refuses the program over.
 *
 * <p>{@code Core} a program hands out is code some output runs: a body, a clause a value is held
 * to, a rule an answer is held to. A writer that reads the places it was written against and none
 * other compiles a program that declares a new one as though it did not, and the object answers
 * without running it. That happened once already, to the rules an answer is held to. So the places
 * are read off the program's types here, and each has to be one {@link ProgramWriter} says what it
 * does with.
 */
class EveryCoreAProgramHandsOutIsWrittenOrRefusedTest {

    /** Every public method reachable from a checked program whose answer holds a {@code Core}, and
     *  what {@link ProgramWriter} does with it. */
    private static final Map<String, String> HANDLED = Map.of(
            "souther.compiler.program.CheckedImplementation$Body#body", "written as a definition",
            "souther.compiler.program.CheckedHelper#body", "written as a helper",
            "souther.compiler.program.CheckedValue#body", "written as a value",
            "souther.compiler.program.CheckedValueEntry#body", "written as an entry",
            "souther.compiler.core.ValueShape$Invariant#condition",
            "written as the clause of a declaration this build builds, and built by the declaring"
                    + " build's object for one another build declares",
            "souther.compiler.core.Contract$Rule#condition",
            "refused: a behavior whose answer is held to a rule is not written yet");

    @Test
    void everyPlaceAProgramHandsOutCoreIsOneTheWriterAnswersFor() {
        assertThat(surfacesReachableFrom(CheckedProgram.class))
                .as("a place a checked program hands out Core that ProgramWriter neither writes nor"
                        + " refuses a program over, or one the program no longer has")
                .isEqualTo(new TreeSet<>(HANDLED.keySet()));
    }

    /** Every public method reachable from {@code from} whose answer names a {@code Core}. */
    private static Set<String> surfacesReachableFrom(Class<?> from) {
        Set<String> surfaces = new TreeSet<>();
        Set<Class<?>> seen = new HashSet<>();
        Deque<Class<?>> pending = new ArrayDeque<>();
        pending.add(from);
        while (!pending.isEmpty()) {
            Class<?> here = pending.poll();
            // What is under a Core is the Core's own, and written as nodes rather than a surface.
            if (!seen.add(here) || Core.class.isAssignableFrom(here)) {
                continue;
            }
            Class<?>[] arms = here.getPermittedSubclasses();
            if (arms != null) {
                pending.addAll(List.of(arms));
            }
            for (Method method : here.getMethods()) {
                if (method.getDeclaringClass() == Object.class) {
                    continue;
                }
                Set<Class<?>> named = new LinkedHashSet<>();
                namedIn(method.getGenericReturnType(), named);
                for (Class<?> each : named) {
                    if (Core.class.isAssignableFrom(each)) {
                        surfaces.add(method.getDeclaringClass().getName() + "#" + method.getName());
                    }
                    pending.add(each);
                }
            }
        }
        return surfaces;
    }

    private static void namedIn(Type type, Set<Class<?>> into) {
        switch (type) {
            case Class<?> c -> {
                Class<?> element = c;
                while (element.isArray()) {
                    element = element.getComponentType();
                }
                if (element.getName().startsWith("souther.")) {
                    into.add(element);
                }
            }
            case ParameterizedType p -> {
                namedIn(p.getRawType(), into);
                for (Type argument : p.getActualTypeArguments()) {
                    namedIn(argument, into);
                }
            }
            case WildcardType w -> {
                for (Type bound : w.getUpperBounds()) {
                    namedIn(bound, into);
                }
            }
            default -> { }
        }
    }
}
