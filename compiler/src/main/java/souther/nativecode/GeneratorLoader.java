package souther.nativecode;

import souther.bindings.BindingGenerator;

import java.net.MalformedURLException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;

/**
 * What a generator's code resolves against: the JDK, the API it is written against, and its own jar.
 *
 * <p>Its parent is the platform class loader, so the compiler and what the compiler is written
 * against are not in its graph at all, and a generator that happened to use one of them fails here
 * and not on the day the compiler changes it. The API's own package is the one thing it is handed
 * from the compiler, always, even where the jar carries a copy, so the types a generator is handed
 * and the ones it was written against are one. A boundary of what a generator depends on, and not a
 * sandbox: code that walks from those types to the compiler's loader is not stopped.
 */
final class GeneratorLoader extends URLClassLoader {

    /** The package of the API; its subpackages are generators' own. */
    private static final String API = BindingGenerator.class.getPackageName();

    static {
        registerAsParallelCapable();
    }

    GeneratorLoader(Path jar) throws MalformedURLException {
        super("generator " + jar.getFileName(), new URL[] {jar.toUri().toURL()},
                ClassLoader.getPlatformClassLoader());
    }

    @Override
    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        int dot = name.lastIndexOf('.');
        if (dot > 0 && name.substring(0, dot).equals(API)) {
            return BindingGenerator.class.getClassLoader().loadClass(name);
        }
        return super.loadClass(name, resolve);
    }
}
