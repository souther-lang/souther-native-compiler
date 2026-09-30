package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.bindings.BindingGenerator;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A generator's code is called only through {@link Bindings.Generator}, which tells its failures
 * apart from the command's. What keeps it so is that nothing else holds a generator: no class of
 * the command but {@link Bindings} keeps one, and no method answers one, so a call into a
 * generator's code from anywhere else has no generator to call. Walked over every class the
 * command's own classes declare.
 */
class OnlyBindingsHoldsAGeneratorTest {

    @Test
    void noClassButBindingsKeepsAGeneratorAndNoMethodAnswersOne() throws Exception {
        List<String> holding = new ArrayList<>();
        for (Class<?> type : declared()) {
            boolean owner = type == Bindings.class || type == Bindings.Generator.class;
            for (Field field : type.getDeclaredFields()) {
                if (!owner && mentions(field.getGenericType())) {
                    holding.add(type.getName() + "." + field.getName());
                }
            }
            for (Method method : type.getDeclaredMethods()) {
                if (!method.isSynthetic() && mentions(method.getGenericReturnType())) {
                    holding.add(type.getName() + "." + method.getName() + "()");
                }
            }
        }
        assertThat(holding).as("what holds or answers a generator").isEmpty();
    }

    /** Whether {@code type} is a generator, or a type argument of it at any depth is. */
    private static boolean mentions(Type type) {
        return switch (type) {
            case Class<?> it -> BindingGenerator.class.isAssignableFrom(it)
                    || it.isArray() && mentions(it.getComponentType());
            case ParameterizedType it -> mentions(it.getRawType())
                    || Stream.of(it.getActualTypeArguments()).anyMatch(
                            OnlyBindingsHoldsAGeneratorTest::mentions);
            default -> false;
        };
    }

    /** Every class the command's own classes declare, nested ones among them. */
    private static List<Class<?>> declared() throws Exception {
        Path classes = Path.of(Bindings.class.getProtectionDomain().getCodeSource().getLocation()
                .toURI());
        List<Class<?>> types = new ArrayList<>();
        try (Stream<Path> files = Files.walk(classes.resolve("souther/nativecode"))) {
            for (Path file : files.filter(it -> it.toString().endsWith(".class")).sorted().toList()) {
                String name = classes.relativize(file).toString()
                        .replace(file.getFileSystem().getSeparator(), ".")
                        .replaceAll("\\.class$", "");
                types.add(Class.forName(name, false, Bindings.class.getClassLoader()));
            }
        }
        assertThat(types).contains(Main.class, Bindings.Generator.class);
        return types;
    }
}
