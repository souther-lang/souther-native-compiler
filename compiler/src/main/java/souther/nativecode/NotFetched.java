package souther.nativecode;

import java.io.IOException;

/**
 * Something the command needs that it could not have, and why: it may not fetch, there is no
 * release to fetch it from, what came did not match what it was to be, or the fetch failed.
 *
 * <p>Nothing that failed is kept, so the next command asks again rather than trusting what an
 * earlier one left half done.
 */
public final class NotFetched extends IOException {

    NotFetched(String why) {
        super(why);
    }

    NotFetched(String why, Throwable cause) {
        super(why, cause);
    }
}
