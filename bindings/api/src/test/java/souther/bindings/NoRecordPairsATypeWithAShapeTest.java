package souther.bindings;

import org.junit.jupiter.api.Test;

import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value's type and the shape it crosses in are paired once, by the command, into a
 * {@link ValueCrossing}, and a generator is handed the pair. So no other record the API offers
 * holds a type beside a shape: one that did would hand a generator the two apart, to pair its own
 * way, which is two authorities for one crossing. Walked over every record the API's classes
 * declare, so one added later is held as soon as it is added.
 */
class NoRecordPairsATypeWithAShapeTest {

    @Test
    void noRecordButAValueCrossingHoldsATypeBesideAShape() throws Exception {
        List<String> pairing = new ArrayList<>();
        for (Class<?> type : declared()) {
            if (!type.isRecord() || ValueCrossing.class.isAssignableFrom(type)) {
                continue;
            }
            boolean typed = false;
            boolean shaped = false;
            for (RecordComponent component : type.getRecordComponents()) {
                typed |= mentions(component.getGenericType(), Manifest.Type.class);
                shaped |= mentions(component.getGenericType(), Manifest.Shape.class)
                        || mentions(component.getGenericType(), Manifest.Signature.class);
            }
            if (typed && shaped) {
                pairing.add(type.getName());
            }
        }
        assertThat(pairing).as("records holding a type beside a shape").isEmpty();
    }

    /** Whether {@code type} is {@code of}, or a type argument of it at any depth is. */
    private static boolean mentions(Type type, Class<?> of) {
        return switch (type) {
            case Class<?> it -> of.isAssignableFrom(it);
            case ParameterizedType it -> Stream.of(it.getActualTypeArguments())
                    .anyMatch(argument -> mentions(argument, of));
            default -> false;
        };
    }

    /** Every class the API's own classes declare, nested ones among them. */
    private static List<Class<?>> declared() throws Exception {
        Path classes = Classes.of(Manifest.class);
        List<Class<?>> types = new ArrayList<>();
        try (Stream<Path> files = Files.walk(classes.resolve("souther/bindings"))) {
            for (Path file : files.filter(it -> it.toString().endsWith(".class")).sorted().toList()) {
                String name = Classes.nameOf(classes, file);
                types.add(Class.forName(name));
            }
        }
        assertThat(types).contains(Manifest.Call.class, ValueCrossing.Listed.class);
        return types;
    }
}
