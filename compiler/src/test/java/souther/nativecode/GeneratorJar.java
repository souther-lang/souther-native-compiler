package souther.nativecode;

import souther.bindings.BindingApi;
import souther.bindings.BindingGenerator;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.jar.Attributes;
import java.util.jar.JarEntry;
import java.util.jar.JarOutputStream;
import java.util.jar.Manifest;

/**
 * A generator's jar as a test writes one: the attributes its manifest says, the providers its
 * service file names, and the classes it holds, taken from what the tests were compiled to. By
 * default it says it is compatible with this command, as a generator built for it would.
 */
final class GeneratorJar {

    private final Map<String, String> attributes = new LinkedHashMap<>();
    private final List<String> providers = new ArrayList<>();
    private final Set<Class<?>> classes = new LinkedHashSet<>();
    private boolean manifest = true;

    private GeneratorJar(String id) {
        attributes.put(BindingApi.ID, id);
        attributes.put(BindingApi.API, String.valueOf(BindingApi.MAJOR));
        attributes.put(BindingApi.ABI_GENERATIONS, String.valueOf(ManifestReader.ABI));
        classes.add(TestGenerators.class);
    }

    /** A jar that says it is the generator {@code id}, providing {@code provider}. */
    static GeneratorJar of(String id, Class<? extends BindingGenerator> provider) {
        return new GeneratorJar(id).provider(provider);
    }

    /** A jar that says it is the generator {@code id}, and provides nothing yet. */
    static GeneratorJar providing(String id) {
        return new GeneratorJar(id);
    }

    /** Names {@code provider} in the service file too, and holds it. */
    GeneratorJar provider(Class<? extends BindingGenerator> provider) {
        providers.add(provider.getName());
        return holding(provider);
    }

    /** Holds {@code type}, and each of its superclasses the tests compiled. */
    GeneratorJar holding(Class<?> type) {
        for (Class<?> at = type; at != null && at != Object.class; at = at.getSuperclass()) {
            classes.add(at);
        }
        return this;
    }

    /** Says {@code value} for {@code name}, or nothing for it where {@code value} is null. */
    GeneratorJar saying(String name, String value) {
        if (value == null) {
            attributes.remove(name);
        } else {
            attributes.put(name, value);
        }
        return this;
    }

    /** A jar with no {@code META-INF/MANIFEST.MF} at all. */
    GeneratorJar withNoManifest() {
        manifest = false;
        return this;
    }

    byte[] bytes() throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        Manifest written = new Manifest();
        written.getMainAttributes().put(Attributes.Name.MANIFEST_VERSION, "1.0");
        attributes.forEach((name, value) -> written.getMainAttributes().putValue(name, value));
        try (JarOutputStream jar = manifest ? new JarOutputStream(out, written)
                : new JarOutputStream(out)) {
            if (!providers.isEmpty()) {
                jar.putNextEntry(new JarEntry("META-INF/services/" + BindingGenerator.class.getName()));
                jar.write((String.join("\n", providers) + "\n").getBytes(StandardCharsets.UTF_8));
                jar.closeEntry();
            }
            for (Class<?> type : classes) {
                String entry = type.getName().replace('.', '/') + ".class";
                jar.putNextEntry(new JarEntry(entry));
                try (InputStream in = type.getClassLoader().getResourceAsStream(entry)) {
                    jar.write(in.readAllBytes());
                }
                jar.closeEntry();
            }
        }
        return out.toByteArray();
    }

    /** Writes the jar to {@code file}, and answers where. */
    Path writtenTo(Path file) throws IOException {
        Files.createDirectories(file.getParent());
        Files.write(file, bytes());
        return file;
    }
}
