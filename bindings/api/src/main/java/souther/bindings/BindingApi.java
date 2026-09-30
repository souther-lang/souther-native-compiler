package souther.bindings;

/**
 * What a generator's jar says of itself, and the version of this API it says it was written against.
 *
 * <p>A generator is a jar the command was pointed at, and the command learns everything it decides
 * on before running any of its code from the jar's {@code META-INF/MANIFEST.MF}: which generator it
 * is ({@value #ID}), the major of this API it was compiled against ({@value #API}), and the ABI
 * generations the code it writes, and the host runtime that code calls, are built for
 * ({@value #ABI_GENERATIONS}). The command loads a jar only where its major is {@link #MAJOR} and its
 * generations hold the one the command writes; anything else is refused before the library is built.
 *
 * <p>{@link #MAJOR} is a promise: a command of major N runs every generator already compiled against
 * major N, without recompiling it, with the same meaning. Linking is not enough for that. A
 * generator switching over every case of a sealed type still links once a case is added, and no
 * longer means what it did when it is handed the new one. So the major moves when a case is added to
 * a sealed type or a constant to an enum, when a record's components change, when a member that is
 * there is removed or its signature changes (its declared JSpecify nullness among it), when a type's
 * supertypes change, or when the abstract methods of an interface or class nothing seals change. An
 * added type, or an added member that asks nothing of what implements this API, does not move it.
 *
 * <p>The surface each major promises is recorded in {@code bindings/api/generations/<major>.txt},
 * and a test fails when a line recorded for {@link #MAJOR} is no longer true of the classes. That
 * test sees structure and declared nullness. A method that keeps both and comes to answer something
 * else has changed what it means without the test seeing it, and moves the major all the same: that
 * part of the promise is kept by whoever changes it.
 */
public final class BindingApi {

    /** The major of this API: see the rule above. */
    public static final int MAJOR = 1;

    /** The attribute naming the generator a jar is, the same across its versions. */
    public static final String ID = "Souther-Binding-Id";

    /** The attribute naming the major of this API a jar was compiled against. */
    public static final String API = "Souther-Binding-Api";

    /** The attribute naming, comma-separated, each ABI generation a jar writes code for. */
    public static final String ABI_GENERATIONS = "Souther-Abi-Generations";

    private BindingApi() {
    }
}
