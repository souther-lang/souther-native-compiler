package souther.nativecode;

import java.util.regex.Pattern;

/**
 * Which generator a jar may say it is, by what asked for it.
 *
 * <p>Ids are two namespaces. An unqualified id, one word with no dot ({@code php}, {@code rust},
 * {@code swift}), is reserved for the bindings this project ships, including the ones it has not
 * shipped yet. A qualified id, with at least one dot ({@code com.acme.kotlin}), is anyone else's. The
 * split is what keeps a generator of someone else's valid as the catalog grows: an id is what owns a
 * binding's directory, so a rule that later refused an id already in use would break every directory
 * written under it. Who owns a reverse-DNS name is not checked.
 */
sealed interface IdRule {

    /** An id the catalog names: a word of lowercase letters, digits and hyphens. */
    Pattern RESERVED = Pattern.compile("[a-z][a-z0-9-]*");

    /** An id of anyone else's: dotted, of parts each such a word. */
    Pattern QUALIFIED = Pattern.compile("[a-z][a-z0-9-]*(\\.[a-z][a-z0-9-]*)+");

    /** Refuses {@code id}, said by {@code jar}, where it is not one this allows. */
    void check(String id, Object jar) throws NotAGenerator;

    /** The catalog's binding {@code id}, and no other. */
    record Exactly(String id) implements IdRule {

        public Exactly {
            if (!RESERVED.matcher(id).matches()) {
                throw new IllegalArgumentException("the catalog names \"" + id + "\", which is not a"
                        + " reserved id");
            }
        }

        @Override
        public void check(String said, Object jar) throws NotAGenerator {
            if (!said.equals(id)) {
                throw new NotAGenerator(jar + " says it is the generator \"" + said + "\", and is to"
                        + " be \"" + id + "\"");
            }
        }
    }

    /** A generator named with {@code --binding}: any qualified id. */
    record External() implements IdRule {
        @Override
        public void check(String said, Object jar) throws NotAGenerator {
            if (!QUALIFIED.matcher(said).matches()) {
                throw new NotAGenerator(jar + " says it is the generator \"" + said + "\": a"
                        + " generator named with --binding has a qualified id, with at least one dot"
                        + " (com.acme.kotlin), and an id with none is reserved for the bindings this"
                        + " command ships");
            }
        }
    }
}
