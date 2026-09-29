package souther.bindings.go;

import souther.bindings.NotBindable;

import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * What a Souther name is called in Go, and every name Go will not take refused.
 *
 * <p>A name is written as the model spells it, apart from the first letter: Go exports what starts
 * with a capital, so a type, a behavior and a field's reader are written with theirs made one, and
 * a name whose first letter has no capital (a digit, an underscore, a letter of a script without
 * case) is one Go cannot export and is refused. What a binding calls by a name a user did not
 * choose is refused the same way as one a user chose, and a name is never made up: two names that
 * come to one are refused with both of them named, and a parameter that is a keyword or a name the
 * generated code uses is written with an underscore after it.
 */
final class GoNames {

    private GoNames() {
    }

    /** Every keyword of Go. */
    private static final Set<String> KEYWORDS = Set.of("break", "case", "chan", "const",
            "continue", "default", "defer", "else", "fallthrough", "for", "func", "go", "goto", "if",
            "import", "interface", "map", "package", "range", "return", "select", "struct",
            "switch", "type", "var");

    /**
     * What a parameter or a local of the generated code may not be called, since it is written
     * beside them: the keywords, what Go declares before any package does that the generated code
     * uses, and what the generated code names itself.
     */
    private static final Set<String> TAKEN = Set.of("error", "string", "int64", "bool", "nil",
            "true", "false", "byte", "any", "len", "make", "append", "panic", "recover", "uint8",
            "uint32", "int32", "float64", "iota", "unsafe", "souther", "lib", "raoh", "r", "v", "fn",
            "err", "json", "value", "run", "C", "b", "hosted", "impl", "userdata", "requirements", "failed", "reading", "made", "input");

    /** Names a top module of the model may not be, since a directory of that name means more. */
    private static final Set<String> DIRECTORIES = Set.of("internal", "vendor", "testdata");

    /** What Go calls {@code name}, as a package member: the first letter a capital. */
    static String exported(String name, String what) {
        if (!isIdentifier(name)) {
            throw new NotBindable(what + " is named `" + name + "`, which Go takes for no name");
        }
        char first = name.charAt(0);
        if (!Character.isLetter(first) || first > 0x7f) {
            throw new NotBindable(what + " is named `" + name + "`, which Go cannot export: what"
                    + " starts with anything but a letter of the alphabet has no capital to start"
                    + " with");
        }
        String exported = Character.toUpperCase(first) + name.substring(1);
        if (exported.equals("C")) {
            throw new NotBindable(what + " would be `C`, which a file that calls C reads as C");
        }
        return exported;
    }

    /** What Go calls a parameter named {@code name}: as it is, or with an underscore after it. */
    static String local(String name, String what) {
        if (!isIdentifier(name) || name.equals("_")) {
            throw new NotBindable(what + " is named `" + name + "`, which Go takes for no name");
        }
        return KEYWORDS.contains(name) || TAKEN.contains(name) || GENERATED.matcher(name).matches()
                ? name + "_" : name;
    }

    /** The locals the generated code makes of its own, each a base and a number. */
    private static final java.util.regex.Pattern GENERATED =
            java.util.regex.Pattern.compile("(g|a|text|at|held|option)[0-9]+");

    /**
     * The directories a module is written in, one for each of its dotted parts, each a name Go
     * takes as the name of a package.
     */
    static List<String> modulePath(String module) {
        return Arrays.stream(module.split("\\.", -1)).map(part -> {
            if (!isIdentifier(part) || part.startsWith("_") || part.equals("_")
                    || part.charAt(0) > 0x7f) {
                throw new NotBindable("module `" + module + "` has a part `" + part + "`, which"
                        + " Go takes for no package's name");
            }
            if (KEYWORDS.contains(part) || part.equals("main") || DIRECTORIES.contains(part)) {
                throw new NotBindable("module `" + module + "` has a part `" + part + "`, which"
                        + " Go cannot name a package, or reads as more than a name");
            }
            return part;
        }).toList();
    }

    /**
     * {@code path} as the import path of the package a binding is written as, which is the name its
     * host depends on it by.
     */
    static String importPath(String path) {
        if (path.isEmpty() || path.startsWith("/") || path.endsWith("/") || path.contains("//")
                || !path.matches("[A-Za-z0-9._~/-]+")
                || Arrays.stream(path.split("/")).anyMatch(part -> part.equals(".")
                        || part.equals("..") || part.startsWith("."))) {
            throw new NotBindable("a package is named by an import path of letters, digits and"
                    + " `._~-` in parts between `/`, and not `" + path + "`");
        }
        if (!path.split("/")[0].contains(".")) {
            throw new NotBindable("a package another module depends on has a `.` in the first part of"
                    + " its import path, as Go asks of it, and `" + path + "` has none");
        }
        packageName(path);
        return path;
    }

    /** What the last part of {@code importPath} is called as a package's name. */
    static String packageName(String importPath) {
        String last = importPath.substring(importPath.lastIndexOf('/') + 1);
        String name = last.replaceAll("[^A-Za-z0-9_]", "_");
        if (name.matches("v[0-9]+") && importPath.contains("/")) {
            // A major version suffix is not a name: the one before it is.
            String[] parts = importPath.split("/");
            name = parts[parts.length - 2].replaceAll("[^A-Za-z0-9_]", "_");
        }
        if (!isIdentifier(name) || Character.isDigit(name.charAt(0))
                || KEYWORDS.contains(name) || name.equals("main") || name.equals("_")) {
            throw new NotBindable("the package `" + importPath + "` is named `" + name
                    + "`, which Go cannot name a package");
        }
        return name;
    }

    private static boolean isIdentifier(String name) {
        if (name.isEmpty()) {
            return false;
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

    /**
     * The names taken in one place Go reads names in, so that two things given one name there are
     * refused with both of them named, rather than written as two declarations Go refuses.
     */
    static final class Claimed {

        private final String where;
        private final Map<String, String> taken = new HashMap<>();

        Claimed(String where) {
            this.where = where;
        }

        /** Takes {@code name} for {@code what}, answering it. */
        String claim(String name, String what) {
            String before = taken.putIfAbsent(name, what);
            if (before != null) {
                throw new NotBindable(what + " and " + before + " are both `" + name + "` in "
                        + where);
            }
            return name;
        }

        boolean has(String name) {
            return taken.containsKey(name);
        }
    }
}
