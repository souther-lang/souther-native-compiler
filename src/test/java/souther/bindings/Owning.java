package souther.bindings;

import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.RecordComponent;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Stream;

/**
 * Whether a record owns the collections it holds: what it was checked against where it was made is
 * what it holds afterwards, whatever becomes of the collection it was handed, and nothing it answers
 * can be changed through. A record holding its caller's list holds whatever that list becomes, and
 * what its constructor held it to holds no longer.
 *
 * <p>Asked of records a test has in hand, and kept per record type and component, so a test can
 * require every component of every such type under a package to have been asked of with something
 * in it: a collection with nothing in it shows nothing about whose it is.
 */
public final class Owning {

    /** What was found wrong, one line each. */
    public final Set<String> wrong = new TreeSet<>();

    /** Each {@code Type.component} asked of with something in it. */
    public final Set<String> asked = new TreeSet<>();

    private final Set<Object> seen = java.util.Collections.newSetFromMap(new IdentityHashMap<>());

    /** Asks every record reachable from {@code root}, through records and collections. */
    public void walk(Object root) {
        if (root == null || !seen.add(root)) {
            return;
        }
        switch (root) {
            case Record record -> {
                ask(record);
                for (RecordComponent component : record.getClass().getRecordComponents()) {
                    walk(read(record, component));
                }
            }
            case Collection<?> each -> each.forEach(this::walk);
            case Map<?, ?> each -> each.values().forEach(this::walk);
            default -> {
            }
        }
    }

    /**
     * Every {@code Type.component} of a record declared in {@code packageName} whose component is a
     * collection, as the build compiled the package.
     */
    public static Set<String> collectionsIn(String packageName) throws Exception {
        Path classes = Path.of(Manifest.class.getProtectionDomain().getCodeSource().getLocation()
                .toURI());
        Path under = classes.resolve(packageName.replace('.', '/'));
        Set<String> components = new TreeSet<>();
        try (Stream<Path> files = Files.list(under)) {
            for (Path file : files.filter(it -> it.toString().endsWith(".class")).toList()) {
                String name = packageName + "." + file.getFileName().toString()
                        .replaceAll("\\.class$", "");
                Class<?> type = Class.forName(name);
                if (!type.isRecord()) {
                    continue;
                }
                for (RecordComponent component : type.getRecordComponents()) {
                    if (isCollection(component.getType())) {
                        components.add(key(type, component));
                    }
                }
            }
        }
        return components;
    }

    private void ask(Record record) {
        RecordComponent[] components = record.getClass().getRecordComponents();
        Object[] handed = new Object[components.length];
        List<Object> mutable = new ArrayList<>();
        boolean any = false;
        for (int at = 0; at < components.length; at++) {
            Object held = read(record, components[at]);
            handed[at] = held;
            if (!isCollection(components[at].getType()) || held == null) {
                continue;
            }
            any = true;
            String key = key(record.getClass(), components[at]);
            if (changeable(held)) {
                wrong.add(key + " answers a collection it can be changed through");
            }
            if (size(held) > 0) {
                asked.add(key);
                handed[at] = mutableCopy(held);
                mutable.add(handed[at]);
            }
        }
        if (!any || mutable.isEmpty()) {
            return;
        }
        Record made = make(record.getClass(), components, handed);
        mutable.forEach(Owning::clear);
        for (RecordComponent component : components) {
            if (isCollection(component.getType())
                    && !java.util.Objects.equals(read(made, component), read(record, component))) {
                wrong.add(key(record.getClass(), component)
                        + " holds the collection it was handed, not its own");
            }
        }
    }

    private static boolean isCollection(Class<?> type) {
        return Collection.class.isAssignableFrom(type) || Map.class.isAssignableFrom(type);
    }

    private static String key(Class<?> type, RecordComponent component) {
        return type.getName() + "." + component.getName();
    }

    private static Object read(Record record, RecordComponent component) {
        try {
            Method accessor = component.getAccessor();
            accessor.setAccessible(true);
            return accessor.invoke(record);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    private static Record make(Class<?> type, RecordComponent[] components, Object[] handed) {
        Class<?>[] types = Stream.of(components).map(RecordComponent::getType)
                .toArray(Class<?>[]::new);
        try {
            Constructor<?> canonical = type.getDeclaredConstructor(types);
            canonical.setAccessible(true);
            return (Record) canonical.newInstance(handed);
        } catch (InvocationTargetException e) {
            throw new IllegalStateException(type.getName() + " refused what it held", e.getCause());
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    /** Whether {@code held} takes a change, which is undone at once where it does. */
    @SuppressWarnings("unchecked")
    private static boolean changeable(Object held) {
        try {
            switch (held) {
                case List<?> list -> {
                    ((List<Object>) list).add(null);
                    list.removeLast();
                }
                case Set<?> set -> {
                    Object probe = new Object();
                    ((Set<Object>) set).add(probe);
                    set.remove(probe);
                }
                case Map<?, ?> map -> {
                    Object probe = new Object();
                    ((Map<Object, Object>) map).put(probe, null);
                    map.remove(probe);
                }
                default -> throw new IllegalStateException("not a collection: " + held);
            }
            return true;
        } catch (UnsupportedOperationException | NullPointerException | ClassCastException none) {
            return false;
        }
    }

    private static int size(Object held) {
        return held instanceof Map<?, ?> map ? map.size() : ((Collection<?>) held).size();
    }

    private static Object mutableCopy(Object held) {
        return switch (held) {
            case List<?> list -> new ArrayList<>(list);
            case Set<?> set -> new LinkedHashSet<>(set);
            case Map<?, ?> map -> new LinkedHashMap<>(map);
            default -> throw new IllegalStateException("not a collection: " + held);
        };
    }

    private static void clear(Object mutable) {
        if (mutable instanceof Map<?, ?> map) {
            map.clear();
        } else {
            ((Collection<?>) mutable).clear();
        }
    }
}
