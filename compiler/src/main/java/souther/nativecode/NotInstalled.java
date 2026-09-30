package souther.nativecode;

/**
 * The generators installed with the command cannot be told apart: one does not load, answers to no
 * id, or throws when asked it, or two answer to one. The installation's failure, and no program's.
 */
final class NotInstalled extends Exception {

    NotInstalled(String why, Throwable cause) {
        super(why, cause);
    }
}
