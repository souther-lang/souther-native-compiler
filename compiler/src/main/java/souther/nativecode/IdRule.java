package souther.nativecode;

import java.util.Set;
import java.util.TreeSet;

/**
 * Which generator a jar may say it is, by what asked for it: a binding of the catalog asks for
 * exactly its own, and a jar named on the command line may be any but one the catalog names.
 */
sealed interface IdRule {

    /** Refuses {@code id}, said by {@code jar}, where it is not one this allows. */
    void check(String id, Object jar) throws NotAGenerator;

    record Exactly(String id) implements IdRule {
        @Override
        public void check(String said, Object jar) throws NotAGenerator {
            if (!said.equals(id)) {
                throw new NotAGenerator(jar + " says it is the generator \"" + said + "\", and is to"
                        + " be \"" + id + "\"");
            }
        }
    }

    record NotOneOf(Set<String> ids) implements IdRule {

        public NotOneOf {
            ids = Set.copyOf(ids);
        }

        @Override
        public void check(String said, Object jar) throws NotAGenerator {
            if (ids.contains(said)) {
                throw new NotAGenerator(jar + " says it is the generator \"" + said + "\", which is"
                        + " one this command ships; a generator named with --binding is one of "
                        + "its own, and none of " + new TreeSet<>(ids));
            }
        }
    }
}
