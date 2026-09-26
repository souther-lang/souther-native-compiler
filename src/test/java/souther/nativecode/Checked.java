package souther.nativecode;

import souther.compiler.meta.ModulePath;
import souther.compiler.program.CheckedProgram;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * A program checked once however many questions a test asks of it.
 *
 * <p>Checking one runs every row it states on the JVM, which costs more than anything a question
 * then asks of it; a test asking several questions of one program checked it again for each, in
 * helpers written one per test class. So every test checks a program through here, and a program
 * written alike is checked once while it is still being asked about. What is kept is the last few
 * programs, and not every one the tests write: questions about one program are asked together.
 *
 * <p>A program that does not check throws each time it is asked for, as it does from
 * {@link CheckedProgram#of}, and is kept nowhere.
 */
public final class Checked {

    private static final int KEPT = 16;

    private static final Map<List<String>, CheckedProgram> CHECKED =
            new LinkedHashMap<>(KEPT, 0.75f, true) {
                @Override
                protected boolean removeEldestEntry(Map.Entry<List<String>, CheckedProgram> eldest) {
                    return size() > KEPT;
                }
            };

    private Checked() {}

    /** These sources checked, as {@link CheckedProgram#of} checks them. */
    public static CheckedProgram of(List<String> sources) {
        List<String> key = List.copyOf(sources);
        synchronized (CHECKED) {
            CheckedProgram kept = CHECKED.get(key);
            if (kept != null) {
                return kept;
            }
        }
        CheckedProgram checked = CheckedProgram.of(key);
        synchronized (CHECKED) {
            CHECKED.put(key, checked);
        }
        return checked;
    }

    /**
     * These sources checked against the builds `path` holds. Not kept: what a program checks to
     * depends on those builds as well, and a path is not a value two tests write alike.
     */
    public static CheckedProgram of(List<String> sources, ModulePath path) {
        return CheckedProgram.of(sources, path);
    }
}
