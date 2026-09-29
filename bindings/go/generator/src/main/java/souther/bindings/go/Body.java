package souther.bindings.go;

import souther.bindings.Manifest.Function;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * The statements of one Go function as they are written, and the imports the file they are in
 * has to have for them.
 */
final class Body {

    /**
     * The imports of one generated file: each is noted where the code that needs it is written, so
     * that a file has exactly the imports it uses, which Go holds it to.
     */
    static final class Imports {

        static final String SOUTHER = "github.com/souther-lang/souther-native-compiler/bindings/go/runtime";

        private final String root;
        private boolean unsafe;
        private boolean souther;
        private boolean lib;
        private final Map<String, String> modules = new LinkedHashMap<>();
        private final String own;

        /**
         * @param root the import path of the package the binding is written as
         * @param own  the import path of the package this file is in
         */
        Imports(String root, String own) {
            this.root = root;
            this.own = own;
        }

        String unsafe() {
            unsafe = true;
            return "unsafe";
        }

        String souther() {
            souther = true;
            return "souther";
        }

        String lib() {
            lib = true;
            return "lib";
        }

        /** How this file names a package of the binding: unqualified where it is this one. */
        String module(String importPath) {
            if (importPath.equals(own)) {
                return "";
            }
            return modules.computeIfAbsent(importPath, it -> "m" + modules.size()) + ".";
        }

        /** The import declarations, and nothing where the file imports nothing. */
        String written() {
            StringBuilder out = new StringBuilder();
            // Each group in the order of its paths, as gofmt writes one.
            TreeMap<String, String> others = new TreeMap<>();
            if (souther) {
                others.put(SOUTHER, "\tsouther \"" + SOUTHER + "\"\n");
            }
            if (lib) {
                others.put(root, "\tlib \"" + root + "\"\n");
            }
            modules.forEach((path, alias) -> others.put(path, "\t" + alias + " \"" + path + "\"\n"));
            if (!unsafe && others.isEmpty()) {
                return "";
            }
            out.append("import (\n");
            if (unsafe) {
                out.append("\t\"unsafe\"\n");
            }
            if (unsafe && !others.isEmpty()) {
                out.append("\n");
            }
            others.values().forEach(out::append);
            return out.append(")\n").toString();
        }
    }

    /** What calls a function of the library through its address: the C shim of it, noted to be written. */
    interface Shims {
        String call(Function function);
    }

    private final StringBuilder out = new StringBuilder();
    private int depth;
    private int temps;
    private boolean direct;

    final Imports imports;
    private final Shims shims;

    /** The Go expression of the run the function works in. */
    final String run;

    /** The statement that ends the function with the failure {@code err} holds. */
    final String fail;

    Body(Imports imports, Shims shims, String run, String fail, int depth) {
        this.imports = imports;
        this.shims = shims;
        this.run = run;
        this.fail = fail;
        this.depth = depth;
    }

    /** One statement, indented. */
    Body line(String statement) {
        out.append("\t".repeat(depth)).append(statement).append("\n");
        return this;
    }

    /** Statements already written at the depth they belong at, as they are. */
    Body raw(String written) {
        out.append(written);
        return this;
    }

    /** A block's head, and what is written until {@link #close} is inside it. */
    Body open(String head) {
        line(head + " {");
        depth++;
        return this;
    }

    Body close() {
        depth--;
        return line("}");
    }

    /** A label of the switch this is inside, as gofmt writes it: level with the switch. */
    Body label(String head) {
        depth--;
        line(head);
        depth++;
        return this;
    }

    /**
     * The call of {@code function}, a Go expression, which is made here and not through {@code
     * souther.Call}: what the library answers is a number or an address and no status, and it is
     * made in the run the function works in. Noted, so the function begins by asking the run
     * whether something may be made through it.
     */
    String call(Function function, List<String> arguments) {
        direct = true;
        List<String> handed = new java.util.ArrayList<>(List.of(run + ".Library().Symbol(\""
                + function.name() + "\")"));
        handed.addAll(arguments);
        return shims.call(function) + "(" + String.join(", ", handed) + ")";
    }

    /** Whether something was made through the run by a call written here, before the function's own. */
    boolean makes() {
        return direct;
    }

    /** A name no other of this function has. */
    String temp(String base) {
        return base + temps++;
    }

    /** {@code if err != nil { fail }}. */
    Body checked() {
        return open("if err != nil").line(fail).close();
    }

    @Override
    public String toString() {
        return out.toString();
    }
}
