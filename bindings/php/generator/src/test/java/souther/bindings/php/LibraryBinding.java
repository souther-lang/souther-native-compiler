package souther.bindings.php;

import souther.bindings.BindingInput;
import souther.bindings.Declarations;
import souther.bindings.testkit.TestLibrary;
import souther.nativecode.BindingDirectory;
import souther.nativecode.Generated;
import souther.nativecode.ManifestReader;
import souther.nativecode.NativeCompiler;

import java.io.IOException;
import java.nio.file.Path;

/** The PHP binding of a library a test built, generated from what the build wrote beside it. */
final class LibraryBinding {

    private LibraryBinding() {
    }

    static Generated generated(TestLibrary library, Path into, String namespace)
            throws IOException {
        return fromInput(library.bindingInput(), into, namespace);
    }

    /**
     * From a library built by the compiler itself, for a test that reads or changes the manifest the
     * build wrote, or builds from a document no checked program writes: neither is anything the
     * testkit hands over, since the manifest's form is the compiler's own.
     */
    static Generated generated(NativeCompiler.Library library, Path into, String namespace)
            throws IOException {
        return generated(library.manifest(), library.declarations(), into, namespace);
    }

    /** From a manifest that is not the one the library wrote, as a test that changes one does. */
    static Generated generated(Path manifest, Path declarations, Path into, String namespace)
            throws IOException {
        return fromInput(new BindingInput(ManifestReader.read(manifest),
                Declarations.at(declarations)), into, namespace);
    }

    /**
     * From {@code input}, written and put in place as the command does: replacing a binding of this
     * generator that was there, and nothing else. The mark says the artifact of no jar, since none
     * was loaded.
     */
    private static Generated fromInput(BindingInput input, Path into, String namespace)
            throws IOException {
        return Generated.of(BindingDirectory.written(into, new BindingDirectory.Mark("php",
                        new BindingDirectory.Artifact.Local("0".repeat(64))),
                staging -> PhpBindings.generate(input, staging, namespace)));
    }
}
