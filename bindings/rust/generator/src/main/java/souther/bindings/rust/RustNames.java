package souther.bindings.rust;

import souther.bindings.NotBindable;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * What a Souther name is called in Rust, and every name Rust will not take refused.
 *
 * <p>A name is written as the model spells it, and not put into Rust's own case: a binding whose
 * names were made up would be one its users could not find the model's names in, which is why the
 * PHP binding refuses rather than renames, and the generated crate allows the lints that would say
 * a field is not in snake case. A keyword is written as a raw identifier ({@code r#type}), which is
 * still the model's name; the few Rust takes as no raw identifier are refused.
 */
final class RustNames {

    private RustNames() {
    }

    /** Every keyword of the 2024 edition, strict and reserved, which a name is written raw for. */
    private static final Set<String> KEYWORDS = Set.of(
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while",
            "abstract", "become", "box", "do", "final", "macro", "override", "priv", "try",
            "typeof", "unsized", "virtual", "yield");

    /** The keywords Rust takes as no raw identifier either. */
    private static final Set<String> UNRAWABLE = Set.of("crate", "self", "Self", "super", "_");

    /**
     * Names a crate of this binding's may not be called: the ones Cargo refuses, and the crates a
     * build already names, which one of the same name would stand for.
     */
    private static final Set<String> RESERVED_CRATES = Set.of("std", "core", "alloc", "proc_macro",
            "test", "souther_binding_runtime", "raoh");

    /** What Rust calls {@code name}, where it takes it as an identifier at all, raw or not. */
    static String identifier(String name, String what) {
        if (!isIdentifier(name)) {
            throw new NotBindable(what + " is named `" + name + "`, which Rust takes for no name");
        }
        if (UNRAWABLE.contains(name)) {
            throw new NotBindable(what + " is named `" + name + "`, which Rust takes for no name,"
                    + " not even written raw");
        }
        return KEYWORDS.contains(name) ? "r#" + name : name;
    }

    /** Whether Rust takes {@code name} as an identifier, raw or not. */
    static boolean takes(String name) {
        return isIdentifier(name) && !UNRAWABLE.contains(name);
    }

    /**
     * The path of the module {@code module}, one segment for each of its dotted parts, each as
     * {@link #identifier} writes it.
     */
    static List<String> modulePath(String module) {
        return java.util.Arrays.stream(module.split("\\.", -1))
                .map(part -> identifier(part, "module `" + module + "`"))
                .toList();
    }

    /** A module segment as a file is named: without the {@code r#} it is written with. */
    static String fileName(String segment) {
        return segment.startsWith("r#") ? segment.substring(2) : segment;
    }

    /**
     * {@code name} as the name of the crate a binding is written as, which is the name its host
     * depends on it by: what Cargo takes for a package, which is then what Rust calls the library
     * with every {@code -} an {@code _}.
     */
    static String crateName(String name) {
        if (!name.matches("[A-Za-z][A-Za-z0-9_-]*")) {
            throw new NotBindable("a crate is named with a letter and then letters, digits, `_` and"
                    + " `-`, and not `" + name + "`");
        }
        String library = name.replace('-', '_');
        if (KEYWORDS.contains(library) || RESERVED_CRATES.contains(library)) {
            throw new NotBindable("a crate cannot be named `" + name + "`, which Rust or the"
                    + " binding already names");
        }
        return name;
    }

    /** {@code name} with its first letter capital, as a behavior's type is named. */
    static String capitalized(String name) {
        return name.isEmpty() ? name : Character.toUpperCase(name.charAt(0)) + name.substring(1);
    }

    private static boolean isIdentifier(String name) {
        if (name.isEmpty() || name.equals("_")) {
            return name.equals("_");
        }
        char first = name.charAt(0);
        if (!(first == '_' || Character.isLetter(first)) || first > 0x7f) {
            return false;
        }
        for (int at = 1; at < name.length(); at++) {
            char it = name.charAt(at);
            if (!(it == '_' || it < 0x80 && Character.isLetterOrDigit(it))) {
                return false;
            }
        }
        return true;
    }
}
