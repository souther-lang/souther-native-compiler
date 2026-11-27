package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.BindingApi;
import souther.compiler.Compiler;
import souther.compiler.command.Environment;
import souther.compiler.command.TargetCommand;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.PrintStream;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collections;
import java.util.Enumeration;
import java.util.List;
import java.util.Properties;
import java.util.ServiceLoader;
import java.util.jar.JarFile;
import java.util.zip.ZipEntry;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * {@code souther compile --target native} loads the backend jar beside its own compiler and runs the
 * one command the jar provides, and chooses the jar by what its descriptor says and by nothing else:
 * the target it answers to and the Souther it was built against, read as written. A descriptor
 * naming another Souther than the one the backend was compiled against would have the CLI load a
 * backend written against another compiler than the one it is run with, and nothing between the two
 * would say so. So the version in the descriptor is held here to the Souther compiler this build
 * compiles against, and the jar is loaded as the CLI loads it: under a loader that has Souther's
 * compiler and nothing of this build.
 */
class TheBackendJarIsWhatSoutherCompileRunsForTheNativeTargetTest {

    private static final String DESCRIPTOR = "META-INF/souther/backend.properties";

    private static final String PROVIDED = "META-INF/services/" + TargetCommand.class.getName();

    /** The packages this repository writes, which are the backend's and not the command line's. */
    private static final List<String> OWN = List.of("souther.nativecode.", "souther.bindings.");

    private static final String MONEY = """
            module shop.money exposing ( Money, double )

            data Money = Int
                invariant notNegative = value >= 0

            behavior double : (money: Money) -> Int
            let double (money) = money.value * 2
            """;

    private static Path jar() {
        return Path.of(System.getProperty("souther.backend.jar"));
    }

    @Test
    void theDescriptorNamesTheNativeTargetAndTheSoutherTheJarIsBuiltAgainst() throws Exception {
        String souther = Compiler.class.getPackage().getImplementationVersion();
        assertThat(souther).as("the Souther compiler on this build's class path says its version")
                .isNotBlank();

        try (JarFile archive = new JarFile(jar().toFile())) {
            ZipEntry entry = archive.getEntry(DESCRIPTOR);
            assertThat(entry).as(DESCRIPTOR).isNotNull();
            byte[] bytes;
            try (InputStream in = archive.getInputStream(entry)) {
                bytes = in.readAllBytes();
            }
            Properties descriptor = new Properties();
            descriptor.load(new ByteArrayInputStream(bytes));

            assertThat(descriptor.stringPropertyNames()).containsExactlyInAnyOrder("name",
                    "souther.version");
            assertThat(descriptor.getProperty("name")).isEqualTo("native");
            assertThat(descriptor.getProperty("souther.version")).isEqualTo(souther);
            assertThat(new String(bytes, StandardCharsets.UTF_8)).as("each value as the CLI compares it")
                    .isEqualTo("name=native\nsouther.version=" + souther + "\n");
        }
    }

    /**
     * Souther's compiler and the libraries it runs with are the command line's, so the jar holds no
     * class but this repository's: one it held of theirs would be a copy the command line never
     * reads, since its own comes first. And it says it is the backend's release, which is the
     * release the backend fetches its driver and its generators at.
     */
    @Test
    void theJarHoldsNoClassButThisRepositorysAndSaysItsRelease() throws Exception {
        try (JarFile archive = new JarFile(jar().toFile())) {
            List<String> classes = archive.stream().map(ZipEntry::getName)
                    .filter(name -> name.endsWith(".class"))
                    .map(name -> name.replace('/', '.')).toList();

            assertThat(classes).contains(Main.class.getName() + ".class",
                    BindingApi.class.getName() + ".class");
            assertThat(classes).allMatch(name -> OWN.stream().anyMatch(name::startsWith));
            assertThat(archive.getManifest().getMainAttributes().getValue("Implementation-Version"))
                    .isEqualTo(System.getProperty("souther.native.version"));
            assertThat(archive.getManifest().getMainAttributes().getValue("Class-Path"))
                    .as("a class path beside the jar, which the CLI refuses a backend for")
                    .isNull();
        }
    }

    /**
     * Loaded as the CLI loads it, and run: the command is the one the jar itself provides, and the
     * API a generator is loaded against comes out of the same jar, while the Souther the program is
     * read with is the one above it. A clone's build is not a release, so the generator's jar is
     * named to it, among the properties of the environment it is run in.
     *
     * <p>The command is run as far as a generator answering it, and no further: a binding asked for
     * without the option its generator wants is refused by that generator, which the command has
     * by then loaded against the jar's API and asked. What a library and a binding are built into
     * is the same under any loader, and is what the launcher's own test asks.
     */
    @Test
    void theJarLoadedBesideSouthersCompilerRunsItsCommandAsFarAsAGenerator(@TempDir Path directory)
            throws Exception {
        Path sources = Files.createDirectories(directory.resolve("src"));
        Files.writeString(sources.resolve("money.sou"), MONEY);
        ByteArrayOutputStream printed = new ByteArrayOutputStream();
        PrintStream out = new PrintStream(printed, true, StandardCharsets.UTF_8);
        Environment process = Environment.ofThisProcess();
        Environment environment = new Environment(process.variables(), process.properties(),
                directory, InputStream.nullInputStream(), out, out, false, process.today());

        Thread thread = Thread.currentThread();
        ClassLoader before = thread.getContextClassLoader();
        try (URLClassLoader loader = new URLClassLoader(new URL[]{jar().toUri().toURL()},
                new WithoutThisBuild(before))) {
            // As the CLI does it: the loader is the thread's from before the command is made.
            thread.setContextClassLoader(loader);
            int ended;
            try {
                List<ServiceLoader.Provider<TargetCommand>> provided =
                        ServiceLoader.load(TargetCommand.class, loader).stream().toList();
                assertThat(provided).as("the commands the jar provides").hasSize(1);
                TargetCommand command = provided.getFirst().get();
                assertThat(command.getClass().getName()).isEqualTo(Main.class.getName());
                assertThat(command.getClass().getClassLoader()).as("what defined the command")
                        .isSameAs(loader);
                assertThat(loader.loadClass(BindingApi.class.getName()).getClassLoader())
                        .as("what defined the API").isSameAs(loader);
                assertThat(loader.loadClass(Compiler.class.getName())).as("Souther's compiler")
                        .isSameAs(Compiler.class);

                ended = command.run(List.of("--library", "out/native", "--binding", "rust",
                        "out/rust", "src"), environment);
            } finally {
                thread.setContextClassLoader(before);
            }

            assertThat(printed.toString(StandardCharsets.UTF_8)).as("what the generator said")
                    .contains("the Rust binding wants the option crate");
            assertThat(ended).isEqualTo(2);
        }
        assertThat(directory.resolve("out")).doesNotExist();
    }

    /**
     * What the CLI has when it loads a backend: everything this build runs with but what this
     * repository writes, which is in the backend's jar and nowhere above it.
     */
    private static final class WithoutThisBuild extends ClassLoader {

        private final ClassLoader build;

        WithoutThisBuild(ClassLoader build) {
            super(null);
            this.build = build;
        }

        @Override
        protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
            if (OWN.stream().anyMatch(name::startsWith)) {
                throw new ClassNotFoundException(name);
            }
            return build.loadClass(name);
        }

        @Override
        public URL getResource(String name) {
            return name.equals(PROVIDED) ? null : build.getResource(name);
        }

        @Override
        public Enumeration<URL> getResources(String name) throws IOException {
            return name.equals(PROVIDED) ? Collections.emptyEnumeration()
                    : build.getResources(name);
        }
    }
}
