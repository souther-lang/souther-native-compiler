package souther.bindings.php;

import souther.bindings.BindingInput;
import souther.bindings.Declarations;
import souther.nativecode.BindingDirectory;
import souther.nativecode.Generated;
import souther.bindings.Manifest;
import souther.nativecode.ManifestReader;
import souther.nativecode.NativeCompiler;

import java.io.IOException;
import java.nio.file.Path;

/** The PHP binding of a library a test built, generated from what the build wrote beside it. */
final class LibraryBinding {

    private LibraryBinding() {
    }

    static Generated generated(NativeCompiler.Library library, Path into, String namespace)
            throws IOException {
        return generated(library.manifest(), library.declarations(), into, namespace);
    }

    /** From a manifest that is not the one the library wrote, as a test that changes one does. */
    static Generated generated(Path manifest, Path declarations, Path into, String namespace)
            throws IOException {
        BindingInput input =
                new BindingInput(ManifestReader.read(manifest), Declarations.at(declarations));
        // Written and put in place as the command does.
        return Generated.of(BindingDirectory.written(into, "php",
                staging -> PhpBindings.generate(input, staging, namespace)));
    }
}
