package souther.bindings.php;

import souther.nativecode.NativeCompiler;

import java.io.IOException;
import java.nio.file.Path;

/** The PHP binding of a library a test built, generated from what the build wrote beside it. */
final class LibraryBinding {

    private LibraryBinding() {
    }

    static PhpBindings.Generated generated(NativeCompiler.Library library, Path into,
                                           String namespace) throws IOException {
        return PhpBindings.generate(library.manifest(), library.declarations(), into, namespace);
    }
}
