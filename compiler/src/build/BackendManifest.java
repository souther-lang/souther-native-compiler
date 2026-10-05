import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.SortedMap;
import java.util.TreeMap;
import java.util.jar.Attributes;
import java.util.jar.JarFile;
import java.util.jar.Manifest;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;

/**
 * Gives every package in the backend jar the identity of the jar it came from, as a section of the
 * manifest of its own.
 *
 * <p>A class asks what it is through its package ({@code Package.getImplementationVersion()} and the
 * rest), and the JVM answers from the manifest of the jar the class was loaded from: the section
 * named for the package's directory first, the main section where that has none. Shading puts every
 * dependency into one jar with one manifest, so without this every package would answer with the
 * backend's own identity: Souther's compiler would say it is the backend's release, and the check
 * that the compiler and the API beside it are of one release would compare a manifest with itself.
 *
 * <p>So the main section keeps no identity at all, and each package has a section with exactly what
 * the main section of its own jar says ({@link #IDENTITY}), which is nothing where that jar says
 * nothing. A package split across jars (Souther's {@code souther.compiler} is in its compiler's jar
 * and its syntax's) is defined by the first of them on the class path, which is what the JVM does
 * without shading, and in the order the shade plugin reads them. Split across jars of different
 * releases, or a class in the backend jar whose package came from none of the jars, refuses the
 * build, since either would leave a package answering with what is not its own.
 *
 * <p>Run by the build on the jar the shade plugin wrote, from source, since nothing of it is the
 * compiler's: {@code java BackendManifest.java <backend jar> <the compiler's jar> <class path file>}.
 * The entries keep their order and their times, so the same sources still build the same jar.
 */
public class BackendManifest {

    /** What a package says of itself, read from its jar's main section. */
    static final List<Attributes.Name> IDENTITY = List.of(
            Attributes.Name.IMPLEMENTATION_TITLE, Attributes.Name.IMPLEMENTATION_VERSION,
            Attributes.Name.IMPLEMENTATION_VENDOR, Attributes.Name.SPECIFICATION_TITLE,
            Attributes.Name.SPECIFICATION_VERSION, Attributes.Name.SPECIFICATION_VENDOR,
            new Attributes.Name("Implementation-Vendor-Id"), new Attributes.Name("Implementation-Revision"),
            Attributes.Name.SEALED);

    private static final String VERSION = Attributes.Name.IMPLEMENTATION_VERSION.toString();

    public static void main(String[] args) throws IOException {
        if (args.length != 3) {
            System.err.println("usage: java BackendManifest.java <backend jar> <compiler jar> <class path file>");
            System.exit(2);
        }
        Path backend = Path.of(args[0]);
        List<Path> jars = new ArrayList<>();
        jars.add(Path.of(args[1]));
        for (String entry : Files.readString(Path.of(args[2])).trim().split(java.io.File.pathSeparator)) {
            if (!entry.isEmpty()) {
                jars.add(Path.of(entry));
            }
        }

        SortedMap<String, SortedMap<String, String>> sections = new TreeMap<>();
        Map<String, Path> from = new TreeMap<>();
        for (Path jar : jars) {
            if (!Files.isRegularFile(jar)) {
                throw new IllegalStateException(jar + " is not a jar: a package's identity is read from"
                        + " the manifest of the jar it is shaded from");
            }
            try (JarFile archive = new JarFile(jar.toFile())) {
                SortedMap<String, String> identity = identity(archive.getManifest());
                for (String pkg : packages(archive)) {
                    SortedMap<String, String> had = sections.putIfAbsent(pkg, identity);
                    if (had != null && !java.util.Objects.equals(had.get(VERSION), identity.get(VERSION))) {
                        throw new IllegalStateException("the package " + pkg + " is in " + from.get(pkg)
                                + " and in " + jar + ", which are of different releases");
                    }
                    from.putIfAbsent(pkg, jar);
                }
            }
        }

        try (ZipFile archive = new ZipFile(backend.toFile())) {
            for (String pkg : packages(archive)) {
                if (!sections.containsKey(pkg)) {
                    throw new IllegalStateException("the package " + pkg + " of " + backend
                            + " is in none of the jars it was shaded from, so it has no identity of its own");
                }
            }
            sections.keySet().retainAll(packages(archive));
            Manifest manifest;
            try (InputStream in = archive.getInputStream(archive.getEntry(JarFile.MANIFEST_NAME))) {
                manifest = new Manifest(in);
            }
            for (Attributes.Name name : IDENTITY) {
                manifest.getMainAttributes().remove(name);
            }
            rewrite(backend, archive, write(manifest.getMainAttributes(), sections));
        }
    }

    /** What a jar's main section says of what it is, under the names {@link #IDENTITY} gives. */
    static SortedMap<String, String> identity(Manifest manifest) {
        SortedMap<String, String> identity = new TreeMap<>();
        if (manifest != null) {
            for (Attributes.Name name : IDENTITY) {
                String value = manifest.getMainAttributes().getValue(name);
                if (value != null) {
                    identity.put(name.toString(), value);
                }
            }
        }
        return Collections.unmodifiableSortedMap(identity);
    }

    /** The directories of the classes in a jar, a multi-release one's versions read as the base. */
    static List<String> packages(ZipFile archive) {
        return archive.stream().map(ZipEntry::getName)
                .filter(name -> name.endsWith(".class") && !name.endsWith("module-info.class"))
                .map(name -> name.replaceFirst("^META-INF/versions/[0-9]+/", ""))
                .filter(name -> name.contains("/") && !name.startsWith("META-INF/"))
                .map(name -> name.substring(0, name.lastIndexOf('/') + 1))
                .distinct().sorted().toList();
    }

    /** The manifest as bytes, its sections in the order of their names. */
    static byte[] write(Attributes main, SortedMap<String, SortedMap<String, String>> sections)
            throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        // The main section as the JDK writes it, which puts Manifest-Version first.
        Manifest head = new Manifest();
        head.getMainAttributes().putAll(main);
        head.write(out);
        for (Map.Entry<String, SortedMap<String, String>> section : sections.entrySet()) {
            line(out, "Name: " + section.getKey());
            for (Map.Entry<String, String> attribute : section.getValue().entrySet()) {
                line(out, attribute.getKey() + ": " + attribute.getValue());
            }
            out.write("\r\n".getBytes(StandardCharsets.UTF_8));
        }
        return out.toByteArray();
    }

    /** One header, at most 72 bytes to a line and each continuation begun with a space. */
    static void line(OutputStream out, String header) throws IOException {
        byte[] bytes = header.getBytes(StandardCharsets.UTF_8);
        int at = 0;
        int room = 72;
        while (bytes.length - at > room) {
            int end = at + room;
            while ((bytes[end] & 0xC0) == 0x80) {
                end--;
            }
            out.write(bytes, at, end - at);
            out.write("\r\n ".getBytes(StandardCharsets.UTF_8));
            at = end;
            room = 71;
        }
        out.write(bytes, at, bytes.length - at);
        out.write("\r\n".getBytes(StandardCharsets.UTF_8));
    }

    /** The jar again, with the manifest replaced, each entry in its place and at its time. */
    static void rewrite(Path backend, ZipFile archive, byte[] manifest) throws IOException {
        Path written = Files.createTempFile(backend.getParent(), backend.getFileName().toString(), ".tmp");
        try (ZipOutputStream out = new ZipOutputStream(Files.newOutputStream(written))) {
            for (ZipEntry entry : Collections.list(archive.entries())) {
                ZipEntry copy = new ZipEntry(entry.getName());
                copy.setTime(entry.getTime());
                out.putNextEntry(copy);
                if (entry.getName().equals(JarFile.MANIFEST_NAME)) {
                    out.write(manifest);
                } else if (!entry.isDirectory()) {
                    try (InputStream in = archive.getInputStream(entry)) {
                        in.transferTo(out);
                    }
                }
                out.closeEntry();
            }
        }
        Files.move(written, backend, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
    }
}
