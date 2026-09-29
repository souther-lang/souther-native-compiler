package souther.bindings.go;

import souther.bindings.Manifest.Declaration;

/**
 * A declared type, and where the binding writes it.
 *
 * @param module     the module of the model it is declared in
 * @param importPath the import path of the package that module is written as
 * @param name       what Go calls it
 */
record Declared(String module, Declaration declaration, String importPath, String name) {

    String key() {
        return module + "." + declaration.name();
    }
}
