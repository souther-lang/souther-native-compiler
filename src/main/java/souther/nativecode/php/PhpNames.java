package souther.nativecode.php;

import java.text.Normalizer;
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
        if (RESERVED.contains(asciiLower(name))) {
            throw new PhpBindings.NotBindable(what + " `" + name + "` is a word PHP reserves,"
                    + " which no class, interface or namespace may be called");
        }
        return name;
    }

    /** Refused where {@code name} is not a name PHP takes for a method or a parameter. */
    static String memberName(String name, String what) {
        identifier(name, what);
        if (asciiLower(name).equals(HALT)) {
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
            // The first letter made capital where it is an ASCII one, as PHP would: a letter past
            // ASCII is left as it is, rather than made one Java's rules make it.
            String capital = part.isEmpty() || part.charAt(0) < 'a' || part.charAt(0) > 'z' ? part
                    : (char) (part.charAt(0) - ('a' - 'A')) + part.substring(1);
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
     * Names claimed in one place a name is looked up in: a namespace's classes, a class's methods,
     * or a function's parameters. A second claim of a name is refused where it is the same name to
     * whatever looks it up there.
     */
    static final class Claimed {

        /** What makes two names one, where each kind of name is looked up. */
        private enum Sameness {
            /**
             * A class's or a namespace's name, which is a file's or a directory's too: one to PHP
             * where they differ in the case of ASCII letters, and one to a file system that does
             * not tell case apart (macOS's and Windows' by default) where they differ in the case
             * of any letter, or in how an accented letter is composed.
             */
            FILE,
            /** A method's: one to PHP where they differ in the case of ASCII letters, and no other. */
            METHOD,
            /** A parameter's: PHP reads a variable's name as it is spelt. */
            SPELLING
        }

        private final String where;
        private final Sameness sameness;
        private final Map<String, String> held = new HashMap<>();

        private Claimed(String where, Sameness sameness) {
            this.where = where;
            this.sameness = sameness;
        }

        /** The classes of a namespace, or the namespaces under another. */
        static Claimed classes(String where) {
            return new Claimed(where, Sameness.FILE);
        }

        /** The methods of a class. */
        static Claimed methods(String where) {
            return new Claimed(where, Sameness.METHOD);
        }

        /** The parameters of one function. */
        static Claimed parameters(String where) {
            return new Claimed(where, Sameness.SPELLING);
        }

        /** Claims {@code name} for {@code what}, answering the name. */
        String claim(String name, String what) {
            String key = switch (sameness) {
                case FILE -> asAFile(name);
                case METHOD -> asciiLower(name);
                case SPELLING -> name;
            };
            String before = held.putIfAbsent(key, what);
            if (before != null) {
                throw new PhpBindings.NotBindable(before + " and " + what + " in " + where
                        + switch (sameness) {
                            case FILE -> " are one name to PHP or to a file system that does not"
                                    + " tell case apart, and each is a file of its own";
                            case METHOD -> " are one name to PHP, which does not tell the case of"
                                    + " ASCII letters apart there";
                            case SPELLING -> " are one name, which PHP takes for one parameter of"
                                    + " a function";
                        });
            }
            return name;
        }
    }

    /**
     * {@code name} with its ASCII capitals made small, and nothing else: how PHP compares the names
     * of classes, functions and methods, and the only case it changes (since 8.2, not by locale).
     */
    static String asciiLower(String name) {
        StringBuilder lowered = new StringBuilder(name.length());
        for (int at = 0; at < name.length(); at++) {
            char held = name.charAt(at);
            lowered.append(held >= 'A' && held <= 'Z' ? (char) (held + ('a' - 'A')) : held);
        }
        return lowered.toString();
    }

    /**
     * {@code name} as a file system that does not tell case apart compares it: every letter's case
     * folded, and composed the one way. The two such systems a binding is most often written to
     * fold more than ASCII, so this does too.
     */
    static String asAFile(String name) {
        return Normalizer.normalize(
                name.toUpperCase(Locale.ROOT).toLowerCase(Locale.ROOT), Normalizer.Form.NFC);
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
