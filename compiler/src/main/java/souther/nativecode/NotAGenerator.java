package souther.nativecode;

import java.io.IOException;

/**
 * A jar whose bytes matched, and that is not a generator this command runs: its manifest does not
 * say which generator it is, or says one the binding asked for may not be, or says it was written
 * for another major of the API or other ABI generations; or it provides no generator, or more than
 * one. Found before any of its code runs, and before the library is built.
 */
final class NotAGenerator extends IOException {

    NotAGenerator(String why) {
        super(why);
    }

    NotAGenerator(String why, Throwable cause) {
        super(why, cause);
    }
}
