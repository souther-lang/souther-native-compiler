package souther.nativecode;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.BindingApi;
import souther.bindings.BindingGenerator;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.jar.JarEntry;
import java.util.jar.JarInputStream;
import java.util.jar.JarOutputStream;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A generator's jar is run in a fixed order, and each step runs nothing the one before did not allow:
 * its bytes are held to their digest before anything it says of itself is read, what it says is
 * checked before any of its code runs, and its providers are counted before any is constructed.
 *
 * <p>What the generator then resolves is the JDK, the API, and its own jar: the compiler is not in
 * its loader's graph, the API is always the compiler's own copy, and the thread it is called on
 * resolves against its loader. And the bytes loaded are a copy only this command wrote, which it
 * deletes once it is done.
 */
class AGeneratorJarIsCheckedBeforeAnyOfItsCodeRunsTest {

    @BeforeEach
    @AfterEach
    void nothingAsked() {
        System.clearProperty(TestGenerators.ASKED);
    }

    /**
     * A jar whose manifest does not say it is a generator this command runs, whatever it does not
     * say, is refused with its provider neither initialized nor made.
     */
    @Test
    void aJarThatSaysOtherwiseOrNothingRunsNone(@TempDir Path into) throws Exception {
        Map<String, GeneratorJar> refused = new LinkedHashMap<>();
        refused.put("another major", watched().saying(BindingApi.API,
                String.valueOf(BindingApi.MAJOR + 1)));
        refused.put("another generation", watched().saying(BindingApi.ABI_GENERATIONS,
                String.valueOf(ManifestReader.ABI + 1)));
        refused.put("no id", watched().saying(BindingApi.ID, null));
        refused.put("no major", watched().saying(BindingApi.API, null));
        refused.put("no generations", watched().saying(BindingApi.ABI_GENERATIONS, null));
        refused.put("a range of generations", watched().saying(BindingApi.ABI_GENERATIONS,
                ManifestReader.ABI + "-" + (ManifestReader.ABI + 1)));
        refused.put("a generation twice", watched().saying(BindingApi.ABI_GENERATIONS,
                ManifestReader.ABI + "," + ManifestReader.ABI));
        refused.put("an id with a space", watched().saying(BindingApi.ID, "acme kotlin"));
        refused.put("no manifest", watched().withNoManifest());
        for (Map.Entry<String, GeneratorJar> each : refused.entrySet()) {
            Path jar = each.getValue().writtenTo(into.resolve(each.getKey() + ".jar"));

            assertThatThrownBy(() -> load(into, jar)).as(each.getKey())
                    .isInstanceOf(NotAGenerator.class);
            assertThat(asked()).as(each.getKey()).isEmpty();
        }
    }

    @Test
    void aJarThatSaysItWritesForSeveralGenerationsAmongThemThisOneRuns(@TempDir Path into)
            throws Exception {
        Path jar = watched().saying(BindingApi.ABI_GENERATIONS,
                (ManifestReader.ABI - 1) + "," + ManifestReader.ABI).writtenTo(into.resolve("g.jar"));

        try (Bindings.Generator generator = load(into, jar)) {
            assertThat(generator.id()).isEqualTo("acme.watched");
            assertThat(asked()).containsExactly("initialized Watched", "constructed Watched");
        }
    }

    /** A jar naming no provider, or two, has neither made: they are counted first. */
    @Test
    void aJarWithNoProviderOrTwoMakesNone(@TempDir Path into) throws Exception {
        Path none = GeneratorJar.providing("acme.none").holding(TestGenerators.Watched.class)
                .writtenTo(into.resolve("none.jar"));
        Path two = watched().provider(TestGenerators.AlsoWatched.class)
                .writtenTo(into.resolve("two.jar"));

        assertThatThrownBy(() -> load(into, none)).isInstanceOf(NotAGenerator.class)
                .hasMessageContaining("provides 0 generators");
        assertThatThrownBy(() -> load(into, two)).isInstanceOf(NotAGenerator.class)
                .hasMessageContaining("provides 2 generators");
        assertThat(asked()).as("neither initialized nor made").isEmpty();
    }

    /**
     * A jar fetched from a repository that is not the one its digest names has nothing it says read:
     * one whose manifest is not even a generator's is refused for its digest, and not kept.
     */
    @Test
    void aJarWhoseDigestDoesNotMatchHasNothingItSaysRead(@TempDir Path into) throws Exception {
        byte[] served = watched().withNoManifest().bytes();
        MavenCoordinate coordinate = MavenCoordinate.parse("com.acme:souther-binding-kotlin:1.2.0");
        Fetching fetching = new Fetching(into.resolve("cache"), false,
                java.net.URI.create("http://repository.invalid/maven"),
                java.net.URI.create("http://repository.invalid/releases"), "1.0.0", Map.of(),
                address -> served);
        GeneratorSpec spec = new GeneratorSpec(new GeneratorRef.Maven(coordinate, "0".repeat(64)),
                new IdRule.NotOneOf(GeneratorSpec.catalogIds()), "the generator");

        assertThatThrownBy(() -> Bindings.load(fetching, spec)).isInstanceOf(NotFetched.class)
                .hasMessageContaining("does not match the SHA-256");
        assertThat(into.resolve("cache/generators")).doesNotExist();
        assertThat(asked()).isEmpty();
    }

    /**
     * The generator resolves the API, and not the compiler; the API it gets is the compiler's copy
     * even where its jar carries one; and the thread it is called on resolves against its loader.
     */
    @Test
    void aGeneratorResolvesTheApiAndItsOwnJarAndNotTheCompiler(@TempDir Path into)
            throws Exception {
        byte[] api;
        String entry = BindingGenerator.class.getName().replace('.', '/') + ".class";
        try (InputStream in = BindingGenerator.class.getClassLoader().getResourceAsStream(entry)) {
            api = in.readAllBytes();
        }
        Path jar = withEntry(GeneratorJar.of("acme.looking", TestGenerators.Looking.class).bytes(),
                entry, api, into.resolve("looking.jar"));

        try (Bindings.Generator generator = load(into, jar)) {
            generator.preflight(Map.of());

            assertThat(asked()).containsExactly("context is own true",
                    "api is own false", "compiler does not resolve");
            assertThat(generator.loader().loadClass(BindingGenerator.class.getName()))
                    .isSameAs(BindingGenerator.class);
            assertThatThrownBy(() -> Class.forName(Main.class.getName(), false, generator.loader()))
                    .isInstanceOf(ClassNotFoundException.class);
            assertThat(Thread.currentThread().getContextClassLoader())
                    .as("the thread's own loader is given back").isNotSameAs(generator.loader());
        }
    }

    /**
     * A generator carrying a JSpecify annotation, whose class its loader cannot resolve, loads and
     * runs: an annotation is metadata, and not something a class links against.
     */
    @Test
    void aGeneratorAnnotatedWithWhatItsLoaderCannotResolveRuns(@TempDir Path into)
            throws Exception {
        Path jar = GeneratorJar.of("acme.annotated", FetchedGenerator.class)
                .writtenTo(into.resolve("annotated.jar"));

        try (Bindings.Generator generator = load(into, jar)) {
            assertThatThrownBy(() -> Class.forName("org.jspecify.annotations.Nullable", false,
                    generator.loader())).isInstanceOf(ClassNotFoundException.class);
            generator.preflight(Map.of());
            assertThat(generator.implementation()).isEqualTo(FetchedGenerator.class.getName());
        }
    }

    /**
     * What is loaded is a copy only this command wrote: the jar it was pointed at, replaced after it
     * was hashed, is not what runs; and the copy is gone once the command is done with it.
     */
    @Test
    void aLocalJarRunsFromACopyThatIsGoneOnceItIsClosed(@TempDir Path into) throws Exception {
        Path jar = GeneratorJar.of("acme.looking", TestGenerators.Looking.class)
                .writtenTo(into.resolve("looking.jar"));

        Path copy;
        try (Bindings.Generator generator = load(into, jar)) {
            copy = generator.jar();
            Files.write(jar, "not a jar any more".getBytes());

            assertThat(copy).isNotEqualTo(jar).exists();
            generator.preflight(Map.of());
            assertThat(asked()).contains("compiler does not resolve");
        }
        assertThat(copy).doesNotExist();
        assertThat(copy.getParent()).doesNotExist();
    }

    /** A jar refused after it was copied leaves no copy behind. */
    @Test
    void aJarRefusedLeavesNoCopy(@TempDir Path into) throws Exception {
        Path jar = watched().saying(BindingApi.API, "99").writtenTo(into.resolve("refused.jar"));
        Path temporary = Path.of(System.getProperty("java.io.tmpdir"));
        List<Path> before = copies(temporary);

        assertThatThrownBy(() -> load(into, jar)).isInstanceOf(NotAGenerator.class);
        assertThat(copies(temporary)).isEqualTo(before);
    }

    private static GeneratorJar watched() {
        return GeneratorJar.of("acme.watched", TestGenerators.Watched.class);
    }

    private static Bindings.Generator load(Path into, Path jar) throws Exception {
        return Bindings.load(TheCommandFindsABindingThroughItsGeneratorTest.unreleased(
                into.resolve("cache")), new GeneratorSpec(new GeneratorRef.Local(jar),
                new IdRule.NotOneOf(GeneratorSpec.catalogIds()), "the generator " + jar));
    }

    private static List<String> asked() {
        String said = System.getProperty(TestGenerators.ASKED, "");
        return said.isEmpty() ? List.of() : said.lines().toList();
    }

    private static List<Path> copies(Path temporary) throws IOException {
        try (var held = Files.list(temporary)) {
            return held.filter(it -> it.getFileName().toString().startsWith("souther-generator-"))
                    .sorted().toList();
        }
    }

    /** {@code jar} with {@code bytes} added at {@code entry}, written to {@code file}. */
    private static Path withEntry(byte[] jar, String entry, byte[] bytes, Path file)
            throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        try (JarInputStream in = new JarInputStream(new java.io.ByteArrayInputStream(jar));
             JarOutputStream written = new JarOutputStream(out, in.getManifest())) {
            for (JarEntry each = in.getNextJarEntry(); each != null; each = in.getNextJarEntry()) {
                written.putNextEntry(new JarEntry(each.getName()));
                written.write(in.readAllBytes());
                written.closeEntry();
            }
            written.putNextEntry(new JarEntry(entry));
            written.write(bytes);
            written.closeEntry();
        }
        Files.write(file, out.toByteArray());
        return file;
    }
}
