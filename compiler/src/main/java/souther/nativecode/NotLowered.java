package souther.nativecode;

/**
 * Something the language admits and this backend does not write yet.
 *
 * <p>Told apart from a program the language refuses, which arrives as a {@code CompileException}
 * and is the program's author's to fix. This one is nobody's fault but this project's, and saying
 * so with the same word would make a reader of either read the wrong thing about their program.
 */
public final class NotLowered extends RuntimeException {

    public NotLowered(String what) {
        super(what);
    }
}
