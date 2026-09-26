package souther.bindings;

/**
 * A model a host's binding cannot be generated for as it stands, and why: a name the host's
 * language will not take, or a directory the binding would replace that holds what no generation
 * wrote.
 *
 * <p>Apart from a manifest this does not read, which {@link Manifest#read} refuses with an
 * {@link IllegalArgumentException} of its own: the manifest a build wrote beside its library is
 * one the generators read, and one they do not is this compiler disagreeing with itself, not a
 * model a host cannot be given.
 */
public final class NotBindable extends IllegalArgumentException {

    public NotBindable(String why) {
        super(why);
    }
}
