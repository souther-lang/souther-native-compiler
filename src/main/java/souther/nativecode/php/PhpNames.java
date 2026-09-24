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
            "endswitch", "endwhile", "enum", "eval", "exit", "extends", "final", "finally", "fn",
            "for", "foreach", "function", "global", "goto", "if", "implements", "include",
            "include_once", "instanceof", "insteadof", "interface", "isset", "list", "match",
            "namespace", "new", "or", "print", "private", "protected", "public", "readonly",
            "require", "require_once", "return", "static", "switch", "throw", "trait", "try",
            "unset", "use", "var", "while", "xor", "yield",
            // Reserved as the names of classes and nothing else.
            "bool", "false", "float", "int", "iterable", "mixed", "never", "null", "numeric",
            "object", "parent", "resource", "self", "string", "true", "void");

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
        return name;
    }

    /** The name of a parameter. PHP takes every identifier but one. */
    static String parameterName(String name, String what) {
        identifier(name, what);
        if (name.equals("this")) {
            throw new PhpBindings.NotBindable(
                    what + " is called `this`, which PHP takes for no parameter");
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
     * Names claimed in one place PHP looks names up in: a namespace's classes, or a class's
     * methods. A second claim of a name is refused, including one differing only in the case of its
     * letters, which PHP does not tell apart there.
     */
    static final class Claimed {

        private final String where;
        private final Map<String, String> held = new HashMap<>();

        Claimed(String where) {
            this.where = where;
        }

        /** Claims {@code name} for {@code what}, answering the name. */
        String claim(String name, String what) {
            String before = held.putIfAbsent(name.toLowerCase(Locale.ROOT), what);
            if (before != null) {
                throw new PhpBindings.NotBindable(before + " and " + what + " in " + where
                        + " are one name to PHP, which does not tell the case of letters apart"
                        + " there");
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
