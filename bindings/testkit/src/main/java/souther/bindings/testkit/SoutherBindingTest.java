package souther.bindings.testkit;

import souther.bindings.BindingInput;
import souther.bindings.Declarations;
import souther.compiler.diag.CompileException;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.ManifestReader;
import souther.nativecode.NativeCompiler;
import souther.nativecode.NotLowered;

import java.io.IOException;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Souther sources built into a library, for a test of a binding generator: the input the command
 * would hand the generator, and the library and header a host of the binding loads.
 *
 * <p>Built by the compiler and the driver of the release this testkit belongs to, as the command of
 * that release would build them, so a generator tested here is tested against what it will be given.
 * The test says where the library is written; the testkit keeps no directory of its own.
 *
 * <pre>{@code
 * @Test
 * void aBindingIsWritten(@TempDir Path into) throws Exception {
 *     TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), """
 *             module shop exposing ( total )
 *             ...
 *             """);
 *     new MyGenerator().generate(library.bindingInput(), into.resolve("binding"), Map.of(...));
 * }
 * }</pre>
 */
public final class SoutherBindingTest {

    /**
     * The programs checked most recently, since checking one runs every row it states and a test
     * class often builds one program several times.
     */
    private static final Map<List<String>, CheckedProgram> CHECKED =
            new LinkedHashMap<>(16, 0.75f, true) {
                @Override
                protected boolean removeEldestEntry(Map.Entry<List<String>, CheckedProgram> eldest) {
                    return size() > 16;
                }
            };

    private SoutherBindingTest() {
    }

    /**
     * {@code sources}, one module each, built into a library in {@code into}.
     *
     * @throws IllegalArgumentException where the sources are not a program the language takes, or
     *                                  one the native backend does not write yet
     * @throws IOException              where the driver could not be had or could not build it
     */
    public static TestLibrary compile(Path into, String... sources) throws IOException {
        if (sources.length == 0) {
            throw new IllegalArgumentException("a library is built from at least one module");
        }
        CheckedProgram program = checked(List.of(sources));
        NativeCompiler.Library built;
        try {
            built = NativeCompiler.library(program, List.of(), into, NativeToolchain.driver());
        } catch (NotLowered e) {
            throw new IllegalArgumentException("the native backend does not write this yet: "
                    + e.getMessage(), e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("interrupted while the driver was building " + into, e);
        }
        return new TestLibrary(new BindingInput(ManifestReader.read(built.manifest()),
                Declarations.at(built.declarations())), built.library(), built.header());
    }

    private static CheckedProgram checked(List<String> sources) {
        synchronized (CHECKED) {
            CheckedProgram kept = CHECKED.get(sources);
            if (kept != null) {
                return kept;
            }
        }
        CheckedProgram checked;
        try {
            checked = CheckedProgram.of(sources);
        } catch (CompileException e) {
            throw new IllegalArgumentException("the sources are not a program the language takes: "
                    + e.getMessage(), e);
        }
        synchronized (CHECKED) {
            CHECKED.put(sources, checked);
        }
        return checked;
    }
}
