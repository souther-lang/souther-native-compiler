package souther.bindings;

/**
 * A binding that cannot be written as it was asked for, and why: an option the host's language
 * will not take, a name in the model it will not take, or a directory the binding would replace
 * that holds what no generation wrote.
 *
 * <p>Apart from a manifest the command does not read, which it refuses with an
 * {@link IllegalArgumentException} of its own: the manifest a build wrote beside its library is one
 * the command reads, and one it does not is this compiler disagreeing with itself, not a model a
 * host cannot be given.
 */
public final class NotBindable extends IllegalArgumentException {

    public NotBindable(String why) {
        super(why);
    }
}
