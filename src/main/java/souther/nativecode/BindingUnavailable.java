package souther.nativecode;

/**
 * A binding the command asked for and this compiler writes, whose generator is not among those it
 * can find.
 *
 * <p>Not a command that is wrong: the same command is right where the generator is installed, so it
 * is apart from {@link Main.NotACommand}, and it is what a caller that can fetch the generator
 * catches.
 */
public final class BindingUnavailable extends Exception {

    private final KnownBindings.Kind kind;

    BindingUnavailable(KnownBindings.Kind kind) {
        super("the " + kind.display() + " binding is not installed: " + kind.flag()
                + " needs the generator " + kind.artifact());
        this.kind = kind;
    }

    /** The binding that has no generator. */
    public KnownBindings.Kind kind() {
        return kind;
    }
}
