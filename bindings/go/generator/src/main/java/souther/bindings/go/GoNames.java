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
     * The identifiers Go declares before any package does (its universe block, which the language
     * fixes): a generated body writes some of them (a type, {@code nil}, {@code len}, {@code make}),
     * so a name of the model's that was one would shadow it there.
     */
    private static final Set<String> UNIVERSE = Set.of("any", "bool", "byte", "comparable", "complex64",
            "complex128", "error", "float32", "float64", "int", "int8", "int16", "int32", "int64",
            "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr", "true",
            "false", "iota", "nil", "append", "cap", "clear", "close", "complex", "copy", "delete",
            "imag", "len", "make", "max", "min", "new", "panic", "print", "println", "real",
            "recover");

    /** What a generated signature calls the run and the receiver of a method, which a caller reads. */
    static final String RUN = "r";
    static final Set<String> SIGNATURE = Set.of(RUN, "v", "b", "f");

    /** What a generated file calls what it imports, apart from the packages of the binding. */
    static final Set<String> ALIASES = Set.of("souther", "lib", "raoh", "unsafe", "C");

    /**
     * What a generated file calls each package of the binding it imports: this and a number. It is
     * the one place that says so, and {@link Body.Imports} makes the names from it.
     */
    static final String MODULE_ALIAS = "m";

    /**
     * Whether {@code name} means something already where a generated function is written: a word of
     * Go, an identifier Go declares, what a signature calls the run and a receiver, or what a file
     * calls an import. Each is a closed set defined once; nothing else is reserved, since what the
     * generator writes beyond them is claimed by {@link Names} around the model's names.
     */
    static boolean reserved(String name) {
        return KEYWORDS.contains(name) || UNIVERSE.contains(name) || SIGNATURE.contains(name)
                || ALIASES.contains(name) || isModuleAlias(name);
    }

    /** Whether {@code name} is what a generated file calls a package of the binding. */
    static boolean isModuleAlias(String name) {
        return name.matches(MODULE_ALIAS + "[0-9]+");
    }

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

    /**
     * What Go calls a parameter named {@code name}: as it is, or with an underscore after it where
     * the name means something already ({@link #reserved}). A caller reads what a function takes by
     * the types and never by these names.
     */
    static String local(String name, String what) {
        if (!isIdentifier(name) || name.equals("_")) {
            throw new NotBindable(what + " is named `" + name + "`, which Go takes for no name");
        }
        return reserved(name) ? name + "_" : name;
    }

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
     * What a function the library calls back is exported from Go as, which is a name of the whole
     * program: the C symbol of an {@code //export} is global in the binary, so two bindings in one
     * program, or two modules of one, must never share one however their names are spelled.
     *
     * <p>It is made of what says which one it is, each part encoded so that the whole is read back
     * as the same parts and no other ({@link #encoded}), which a replacement of what Go does not take
     * in a name is not: {@code a-b} and {@code a_b} would come to one.
     *
     * @param kind       {@code i} for a behavior a host implements and {@code f} for a function type
     * @param importPath the import path of the binding, which no two bindings in a program share
     * @param module     the module of the model the callback is of
     * @param name       the behavior or the function type in that module
     */
    static String hostSymbol(char kind, String importPath, String module, String name) {
        return "souther_host_z" + kind + "_z" + encoded(importPath) + "_z" + encoded(module) + "_z"
                + encoded(name);
    }

    /**
     * {@code text} as letters and digits and {@code _}, and as nothing else: a letter or a digit is
     * itself, {@code _} is {@code __}, and any other byte of its UTF-8 is {@code _u} and two
     * hexadecimal digits and {@code _}. Inside a name an {@code _} is then followed only by
     * {@code _} or {@code u}, so {@code _z} is a separator no name holds, and two texts have one
     * spelling only if they are one text.
     */
    static String encoded(String text) {
        StringBuilder out = new StringBuilder();
        for (byte each : text.getBytes(java.nio.charset.StandardCharsets.UTF_8)) {
            char it = (char) (each & 0xff);
            if (it < 0x80 && Character.isLetterOrDigit(it)) {
                out.append(it);
            } else if (it == '_') {
                out.append("__");
            } else {
                out.append("_u").append(String.format("%02x", each & 0xff)).append('_');
            }
        }
        return out.toString();
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
