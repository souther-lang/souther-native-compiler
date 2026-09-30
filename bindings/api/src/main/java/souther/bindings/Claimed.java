package souther.bindings;

import java.util.HashMap;
import java.util.Map;

/**
 * The names taken in one place a host's language reads names in, so that two things given one name
 * there are refused with both of them named, rather than written as two a compiler refuses.
 *
 * <p>Names are told apart as written. A language that folds case where it reads names, as PHP does
 * a class's, tells them apart its own way.
 */
public final class Claimed {

    private final String where;
    private final Map<String, String> taken = new HashMap<>();

    /** Names taken in {@code where}, as a refusal words the place. */
    public Claimed(String where) {
        this.where = where;
    }

    /**
     * Takes {@code name} for {@code what}, answering it.
     *
     * @throws NotBindable where something else has taken it
     */
    public String claim(String name, String what) {
        String before = taken.putIfAbsent(name, what);
        if (before != null) {
            throw new NotBindable(what + " and " + before + " are both `" + name + "` in " + where);
        }
        return name;
    }

    /** Whether {@code name} is taken. */
    public boolean has(String name) {
        return taken.containsKey(name);
    }
}
