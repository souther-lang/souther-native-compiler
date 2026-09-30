package souther.bindings.testkit;

import souther.bindings.BindingInput;

import java.nio.file.Path;

/** A library {@link SoutherBindingTest#compile} built: what a generator is handed, and what a host loads. */
public final class TestLibrary {

    private final BindingInput bindingInput;
    private final Path library;
    private final Path header;

    TestLibrary(BindingInput bindingInput, Path library, Path header) {
        this.bindingInput = bindingInput;
        this.library = library;
        this.header = header;
    }

    /** What the command hands a generator for this library. */
    public BindingInput bindingInput() {
        return bindingInput;
    }

    /** The shared library a host of the binding loads. */
    public Path library() {
        return library;
    }

    /** The C header of the library. */
    public Path header() {
        return header;
    }
}
