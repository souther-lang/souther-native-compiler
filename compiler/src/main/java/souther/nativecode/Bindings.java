package souther.nativecode;

import souther.bindings.BindingGenerator;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.ServiceLoader;

/** The generators a command can use, each by the id the catalog names it by. */
public final class Bindings {

    private final Map<String, BindingGenerator> byId = new HashMap<>();

    private Bindings(Iterable<BindingGenerator> generators) {
        for (BindingGenerator generator : generators) {
            BindingGenerator earlier = byId.putIfAbsent(generator.id(), generator);
            if (earlier != null) {
                throw new IllegalStateException("two generators answer to \"" + generator.id()
                        + "\": " + earlier.getClass().getName() + " and "
                        + generator.getClass().getName());
            }
        }
    }

    /** The generators on the class path. */
    static Bindings installed() {
        return new Bindings(ServiceLoader.load(BindingGenerator.class));
    }

    /** Exactly these generators, for a caller that has chosen them. */
    static Bindings of(List<BindingGenerator> generators) {
        return new Bindings(generators);
    }

    BindingGenerator generatorFor(KnownBindings.Kind kind) throws BindingUnavailable {
        BindingGenerator generator = byId.get(kind.id());
        if (generator == null) {
            throw new BindingUnavailable(kind);
        }
        return generator;
    }
}
