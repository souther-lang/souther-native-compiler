package souther.nativecode.php;

import java.util.Collection;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * What a Souther name is called in PHP, and every name PHP will not take refused.
 *
 * <p>One place for all of it: which names PHP reserves, that PHP compares class and method names
 * without regard to case, and what a generated name may not collide with. A name PHP will not take
 * is refused with the name and the reason rather than turned into another name: a binding whose
 * names were made up would be one its users could not find the model's names in, and a model
 * renaming one thing is easier than a binding learning a rule for spelling it otherwise.
 */
final class PhpNames {

    private PhpNames() {
    }

    /** Words PHP reserves, which no class, interface or namespace may be called. */
    private static final Set<String> RESERVED = Set.of(
            "__halt_compiler", "abstract", "and", "array", "as", "break", "callable", "case",
            "catch", "class", "clone", "const", "continue", "declare", "default", "die", "do",
            "echo", "else", "elseif", "empty", "enddeclare", "endfor", "endforeach", "endif",
            "endswitch", "endwhile", "eval", "exit", "extends", "final", "finally", "fn",
            "for", "foreach", "function", "global", "goto", "if", "implements", "include",
            "include_once", "instanceof", "insteadof", "interface", "isset", "list", "match",
            "namespace", "new", "or", "print", "private", "protected", "public", "readonly",
            "require", "require_once", "return", "static", "switch", "throw", "trait", "try",
            "unset", "use", "var", "while", "xor", "yield",
            // Reserved as the names of classes and nothing else.
            "bool", "false", "float", "int", "iterable", "mixed", "never", "null", "object",
            "parent", "self", "string", "true", "void",
            // Soft-reserved ones, below.
            "enum", "numeric", "resource");

    /**
     * Words PHP's manual reserves for later and PHP accepts as a class's name today. Refused all the
     * same, so that a binding keeps loading on the PHP that starts refusing them.
     */
    private static final Set<String> SOFT_RESERVED = Set.of("enum", "numeric", "resource");

    /**
     * Names PHP takes for no parameter: `$this`, and every superglobal, which a function cannot
     * name a parameter after. As spelt: PHP reads a variable's name with its case.
     */
    private static final Set<String> UNNAMEABLE_PARAMETERS = Set.of("this", "GLOBALS", "_SERVER",
            "_GET", "_POST", "_FILES", "_COOKIE", "_SESSION", "_REQUEST", "_ENV");

    /** The one word PHP reserves that no method may be called either. */
    private static final String HALT = "__halt_compiler";

    /** The words refused as a class's name, for a test holding them to what PHP refuses. */
    static Set<String> reserved() {
        return RESERVED;
    }

    /** The words refused as a class's name that PHP takes today, for the same test. */
    static Set<String> softReserved() {
        return SOFT_RESERVED;
    }

    /** The names refused as a parameter's, for the same test. */
    static Set<String> unnameableParameters() {
        return UNNAMEABLE_PARAMETERS;
    }

    /** Refused where {@code name} is not a name PHP takes for a class, interface or namespace. */
    static String typeName(String name, String what) {
        identifier(name, what);
        if (RESERVED.contains(name.toLowerCase(Locale.ROOT))) {
            throw new PhpBindings.NotBindable(what + " `" + name + "` is a word PHP reserves,"
                    + " which no class, interface or namespace may be called");
        }
        return name;
    }

    /** Refused where {@code name} is not a name PHP takes for a method or a parameter. */
    static String memberName(String name, String what) {
        identifier(name, what);
        if (name.equalsIgnoreCase(HALT)) {
            throw new PhpBindings.NotBindable(what + " `" + name + "` is a word PHP reserves even"
                    + " for a method");
        }
        return name;
    }

    /** The name of a parameter. PHP takes every identifier but one. */
    static String parameterName(String name, String what) {
        identifier(name, what);
        if (UNNAMEABLE_PARAMETERS.contains(name)) {
            throw new PhpBindings.NotBindable(
                    what + " is called `" + name + "`, which PHP takes for no parameter");
        }
        return name;
    }

    /**
     * The namespace a module's declarations stand in: the root the binding was generated under,
     * then each part of the module's name with its first letter made capital.
     */
    static String moduleNamespace(String root, String module) {
        StringBuilder namespace = new StringBuilder(root);
        for (String part : module.split("\\.", -1)) {
            String capital = part.isEmpty() ? part
                    : part.substring(0, 1).toUpperCase(Locale.ROOT) + part.substring(1);
            namespace.append('\\').append(typeName(capital, "module `" + module + "`"));
        }
        return namespace.toString();
    }

    /** Refused where {@code root} is not a namespace. */
    static String rootNamespace(String root) {
        if (root.isEmpty()) {
            throw new PhpBindings.NotBindable(
                    "a binding is generated under a namespace of its own, and none was named");
        }
        for (String part : root.split("\\\\", -1)) {
            typeName(part, "the namespace `" + root + "`");
        }
        return root;
    }

    /**
     * Names claimed in one place PHP looks names up in: a namespace's classes, a class's methods,
     * or a function's parameters. A second claim of a name is refused, including, where PHP does not
     * tell the case of letters apart, one differing only in that.
     */
    static final class Claimed {

        private final String where;
        private final boolean foldsCase;
        private final Map<String, String> held = new HashMap<>();

        private Claimed(String where, boolean foldsCase) {
            this.where = where;
            this.foldsCase = foldsCase;
        }

        /** The classes of a namespace, or the methods of a class: PHP reads these without case. */
        static Claimed members(String where) {
            return new Claimed(where, true);
        }

        /** The parameters of one function: PHP reads a variable's name as it is spelt. */
        static Claimed parameters(String where) {
            return new Claimed(where, false);
        }

        /** Claims {@code name} for {@code what}, answering the name. */
        String claim(String name, String what) {
            String before = held.putIfAbsent(foldsCase ? name.toLowerCase(Locale.ROOT) : name, what);
            if (before != null) {
                throw new PhpBindings.NotBindable(before + " and " + what + " in " + where
                        + (foldsCase ? " are one name to PHP, which does not tell the case of"
                        + " letters apart there" : " are one name, which PHP takes for one"
                        + " parameter of a function"));
            }
            return name;
        }
    }

    /** {@code wanted}, or it with underscores after it until it is none of {@code taken}. */
    static String freeOf(String wanted, Collection<String> taken) {
        String name = wanted;
        while (taken.contains(name)) {
            name = name + "_";
        }
        return name;
    }

    /** What a composition's inputs are called: by their place, since nothing names them. */
    static List<String> positional(int count) {
        return java.util.stream.IntStream.range(0, count).mapToObj(at -> "input" + at).toList();
    }

    /**
     * Letters, digits and underscores, not starting with a digit, where a letter is any character
     * past ASCII too: PHP reads a name as bytes and takes every byte above 0x7f as a letter.
     */
    private static void identifier(String name, String what) {
        boolean first = true;
        for (int at = 0; at < name.length(); at++) {
            char held = name.charAt(at);
            boolean letter = held == '_' || held > 0x7f
                    || (held >= 'a' && held <= 'z') || (held >= 'A' && held <= 'Z');
            boolean digit = held >= '0' && held <= '9';
            if (!(letter || (!first && digit))) {
                throw new PhpBindings.NotBindable(what + " `" + name + "` is not a name PHP takes");
            }
            first = false;
        }
        if (first) {
            throw new PhpBindings.NotBindable(what + " has an empty name");
        }
    }
}
