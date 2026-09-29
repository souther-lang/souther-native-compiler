package souther.bindings.go;

import java.util.Collection;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/**
 * Every identifier of one generated function, claimed from one place.
 *
 * <p>A function of the binding holds names of two kinds: the ones a model chose (a parameter of a
 * behavior, a field of a constructor) and the ones the generator needs (a local that holds an
 * address, a temporary that counts). They are one scope in Go, so a generated name that was also
 * a model's would redeclare or shadow it. The model's names are claimed first, and every name the
 * generator makes is then claimed from what is left, so a generated name yields to a model's and
 * never the other way; nothing is listed of what the generator happens to write, so what it comes
 * to write next cannot collide.
 *
 * <p>What is reserved before either is what Go and the file already mean by a name, which are
 * closed sets: the words of the language, the identifiers Go declares before any package does, and
 * the names of the imports a generated file has ({@link GoNames#reserved}). A model's name that is
 * one of them is written with an underscore after it ({@link GoNames#local}), which is a change to
 * a parameter's name and to nothing a caller sees.
 */
final class Names {

    private final Set<String> taken = new HashSet<>();
    private final Map<String, String> fixed = new HashMap<>();
    private int counter;

    /** A scope in which {@code modelNames}, which are the model's own, are taken. */
    Names(Collection<String> modelNames) {
        taken.addAll(modelNames);
    }

    /**
     * The name for something the generator writes once in a function and refers to by that name
     * again: {@code base} where nothing has it, and otherwise the first of {@code base_1},
     * {@code base_2}, ... that nothing has. The same base is the same name every time it is asked.
     */
    String fixed(String base) {
        return fixed.computeIfAbsent(base, it -> unused(it));
    }

    /** A name of its own for one more thing of a kind: {@code base} and a number no name has. */
    String temp(String base) {
        String name;
        do {
            name = base + counter++;
        } while (taken.contains(name) || GoNames.reserved(name));
        taken.add(name);
        return name;
    }

    private String unused(String base) {
        String name = base;
        for (int at = 1; taken.contains(name) || GoNames.reserved(name) && !base.equals(name); at++) {
            name = base + "_" + at;
        }
        taken.add(name);
        return name;
    }
}
