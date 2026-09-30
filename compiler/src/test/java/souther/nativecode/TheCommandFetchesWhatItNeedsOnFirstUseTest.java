package souther.nativecode;

import com.sun.net.httpserver.HttpServer;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.PrintStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.jar.JarEntry;
import java.util.jar.JarOutputStream;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A released compiler fetches what it does not have when a command first needs it, and only that:
 * the generator of a binding that was asked for, at this compiler's own version, and the driver for
 * this platform. What it fetched is kept, so the next command fetches nothing, and it fetches
 * nothing where it is told not to.
 *
 * <p>A jar and a bundle are held to the checksum this compiler was released with, and to nothing that
 * what serves them says of itself: an asset of a GitHub release can be replaced, and a Maven
 * repository can be a mirror. A bundle holds three files and no others. What does not match, or is
 * not what it is to be, is refused and not kept.
 *
 * <p>Read over HTTP from a server of the test's own, so that what is asked and what is kept are what
 * a real fetch would do.
 */
class TheCommandFetchesWhatItNeedsOnFirstUseTest {

    private static final String VERSION = "1.2.3";
    private static final String JAR = "/maven/org/souther-lang/souther-binding-php/" + VERSION
            + "/souther-binding-php-" + VERSION + ".jar";

    private static final String MONEY = """
            module shop.money exposing ( Money )

            data Money = Int
                invariant notNegative = value >= 0
            """;

    /** A server holding files by path, and counting what was asked of it. */
    private static final class Served implements AutoCloseable {
        final Map<String, byte[]> files = new ConcurrentHashMap<>();
        final Map<String, Integer> asked = new ConcurrentHashMap<>();
        private final HttpServer server;

        Served() throws IOException {
            server = HttpServer.create(new InetSocketAddress(InetAddress.getLoopbackAddress(), 0), 0);
            server.createContext("/", exchange -> {
                String path = exchange.getRequestURI().getPath();
                asked.merge(path, 1, Integer::sum);
                byte[] body = files.get(path);
                if (body == null) {
                    exchange.sendResponseHeaders(404, -1);
                } else {
                    exchange.sendResponseHeaders(200, body.length);
                    exchange.getResponseBody().write(body);
                }
                exchange.close();
            });
            server.start();
        }

        URI at(String path) {
            return URI.create("http://127.0.0.1:" + server.getAddress().getPort() + path);
        }

        /** Serves the generator's jar, and answers the checksum the release carries for it. */
        Map<String, String> serveGenerator() throws IOException {
            byte[] jar = jar();
            files.put(JAR, jar);
            return Map.of(ReleaseChecksums.generator("php"), Fetching.sha256(jar));
        }

        @Override
        public void close() {
            server.stop(0);
        }
    }

    private record Ran(int ended, String printed, String said) {
    }

    private static Fetching fetching(Served served, Path cache, String version,
                                     Map<String, String> checksums) {
        return new Fetching(cache, false, served.at("/maven"), served.at("/releases"), version,
                checksums, Downloads.http());
    }

    @Test
    void aGeneratorThatIsNotThereIsFetchedOnceAndThenKept(@TempDir Path into) throws Exception {
        try (Served served = new Served()) {
            Map<String, String> checksums = served.serveGenerator();
            Fetching fetching = fetching(served, into.resolve("cache"), VERSION, checksums);

            Ran first = build(into, "first", fetching);
            Ran second = build(into, "second", fetching);

            assertThat(first.ended()).as(first.said()).isZero();
            assertThat(first.said()).contains("fetching " + served.at(JAR));
            assertThat(into.resolve("first/out/fetched.txt")).exists();
            assertThat(second.ended()).as(second.said()).isZero();
            assertThat(into.resolve("second/out/fetched.txt")).exists();
            assertThat(served.asked).as("the jar is asked for once, and nothing beside it")
                    .containsOnlyKeys(JAR).containsEntry(JAR, 1);
            assertThat(into.resolve("cache/generators/souther-binding-php-" + VERSION + ".jar"))
                    .exists();
        }
    }

    /**
     * A jar the release named that provides no generator for the id it was fetched for is not what
     * the catalog said it was: the fetch's refusal, and no generator's failure, since no generator
     * of it was asked anything.
     */
    @Test
    void aJarThatProvidesNoGeneratorIsTheFetchesRefusalAndNoGeneratorsFailure(@TempDir Path into)
            throws Exception {
        try (Served served = new Served()) {
            ByteArrayOutputStream empty = new ByteArrayOutputStream();
            new JarOutputStream(empty).close();
            byte[] jar = empty.toByteArray();
            served.files.put(JAR, jar);
            Map<String, String> checksums =
                    Map.of(ReleaseChecksums.generator("php"), Fetching.sha256(jar));

            Ran ran = build(into, "built", fetching(served, into.resolve("cache"), VERSION, checksums));

            assertThat(ran.ended()).as(ran.said()).isEqualTo(2);
            assertThat(ran.said()).contains("provides no generator for \"php\"")
                    .doesNotContain("generator failed");
        }
    }

    /**
     * A repository that serves another jar, and a checksum beside it that agrees, is not believed:
     * what a jar is held to is what the compiler was released with.
     */
    @Test
    void aJarThatIsNotTheOneTheReleaseNamedIsRefusedWhateverIsServedBesideIt(@TempDir Path into)
            throws Exception {
        try (Served served = new Served()) {
            Map<String, String> checksums = served.serveGenerator();
            byte[] replaced = bytes("another jar");
            served.files.put(JAR, replaced);
            served.files.put(JAR + ".sha256", bytes(Fetching.sha256(replaced)));

            Ran ran = build(into, "built", fetching(served, into.resolve("cache"), VERSION, checksums));

            assertThat(ran.ended()).isEqualTo(2);
            assertThat(ran.said()).contains("does not match the checksum this compiler was released with");
            Path generators = into.resolve("cache/generators");
            assertThat(Files.exists(generators) ? list(generators) : List.<Path>of())
                    .as("nothing is kept").isEmpty();
            assertThat(into.resolve("built/native")).as("no library is built for it").doesNotExist();
        }
    }

    @Test
    void aReleaseThatCarriesNoChecksumForAGeneratorTakesNoJar(@TempDir Path into) throws Exception {
        try (Served served = new Served()) {
            served.serveGenerator();

            Ran ran = build(into, "built", fetching(served, into.resolve("cache"), VERSION, Map.of()));

            assertThat(ran.ended()).isEqualTo(2);
            assertThat(ran.said()).contains("no checksum for the PHP generator");
            assertThat(served.asked).isEmpty();
        }
    }

    @Test
    void nothingIsFetchedWhereTheCommandIsOffline(@TempDir Path into) throws Exception {
        try (Served served = new Served()) {
            Map<String, String> checksums = served.serveGenerator();
            Fetching fetching = fetching(served, into.resolve("cache"), VERSION, checksums);

            Ran nothingKept = build(into, "a", fetching, "--offline");
            Ran fetched = build(into, "b", fetching);
            Ran kept = build(into, "c", fetching, "--offline");

            assertThat(nothingKept.ended()).isEqualTo(2);
            assertThat(nothingKept.said()).contains("offline")
                    .contains("org.souther-lang:souther-binding-php");
            assertThat(served.asked).as("offline asked for nothing, and the fetch once")
                    .containsEntry(JAR, 1);
            assertThat(fetched.ended()).as(fetched.said()).isZero();
            assertThat(kept.ended()).as(kept.said()).isZero();
        }
    }

    @Test
    void nothingIsFetchedFromABuildThatIsNotARelease(@TempDir Path into) throws Exception {
        try (Served served = new Served()) {
            Map<String, String> checksums = served.serveGenerator();

            for (String version : new String[] {null, "1.2.3-SNAPSHOT"}) {
                Ran ran = build(into, "v" + version, fetching(served, into.resolve("cache"), version,
                        checksums));

                assertThat(ran.ended()).isEqualTo(2);
                assertThat(ran.said()).contains("not a release");
            }
            assertThat(served.asked).isEmpty();
        }
    }

    @Test
    void fetchWritesNothingButFetchesWhatIsMissing(@TempDir Path into) throws Exception {
        try (Served served = new Served()) {
            Map<String, String> checksums = new java.util.HashMap<>(served.serveGenerator());
            byte[] rust = "a jar".getBytes(StandardCharsets.UTF_8);
            String rustJar = "/maven/org/souther-lang/souther-binding-rust/" + VERSION
                    + "/souther-binding-rust-" + VERSION + ".jar";
            served.files.put(rustJar, rust);
            checksums.put(ReleaseChecksums.generator("rust"), Fetching.sha256(rust));
            byte[] go = "a jar of Go".getBytes(StandardCharsets.UTF_8);
            served.files.put("/maven/org/souther-lang/souther-binding-go/" + VERSION
                    + "/souther-binding-go-" + VERSION + ".jar", go);
            checksums.put(ReleaseChecksums.generator("go"), Fetching.sha256(go));

            Ran ran = run(fetching(served, into.resolve("cache"), VERSION, checksums), "--fetch");

            assertThat(ran.ended()).as(ran.said()).isZero();
            assertThat(ran.printed()).contains("the driver is this build's own")
                    .contains("the PHP generator").contains("the Rust generator")
                    .contains("the Go generator");
            assertThat(into.resolve("cache/generators/souther-binding-php-" + VERSION + ".jar")).exists();
            assertThat(into.resolve("cache/generators/souther-binding-rust-" + VERSION + ".jar")).exists();
            assertThat(into.resolve("cache/generators/souther-binding-go-" + VERSION + ".jar")).exists();
        }
    }

    @Test
    void fetchIsACommandOfItsOwn() throws Exception {
        for (String[] command : new String[][] {{"--fetch", "--offline"}, {"--fetch", "m.sou"},
                {"--fetch", "--library", "out"}, {"--fetch", "-o", "a.o"}}) {
            assertThatThrownBy(() -> Main.read(command)).isInstanceOf(Main.NotACommand.class);
        }
    }

    @Test
    void theDriverIsFetchedOnceForThisPlatformAndKept(@TempDir Path into) throws Exception {
        try (Served served = new Served()) {
            byte[] bundle = bundle(Map.of(NativeBundle.DRIVER, "#!/bin/sh\n",
                    NativeBundle.ARCHIVE, "archive", NativeBundle.REQUIREMENTS, "-lm\n"));
            Fetching fetching = releasedWith(served, into, bundle);
            served.files.put(bundleAt(), bundle);

            Path driver = NativeBundle.locate(fetching);
            NativeBundle.locate(fetching);

            assertThat(driver).isExecutable().hasFileName(NativeBundle.DRIVER);
            assertThat(driver.resolveSibling(NativeBundle.ARCHIVE)).hasContent("archive");
            assertThat(driver.resolveSibling(NativeBundle.REQUIREMENTS)).hasContent("-lm\n");
            assertThat(served.asked).containsEntry(bundleAt(), 1);
        }
    }

    @Test
    void aBundleThatIsNotTheOneTheReleaseNamedIsRefusedAndNotKept(@TempDir Path into)
            throws Exception {
        try (Served served = new Served()) {
            byte[] named = bundle(Map.of(NativeBundle.DRIVER, "one", NativeBundle.ARCHIVE, "a",
                    NativeBundle.REQUIREMENTS, "r"));
            byte[] replaced = bundle(Map.of(NativeBundle.DRIVER, "another", NativeBundle.ARCHIVE, "a",
                    NativeBundle.REQUIREMENTS, "r"));
            Fetching fetching = releasedWith(served, into, named);
            served.files.put(bundleAt(), replaced);

            assertThatThrownBy(() -> NativeBundle.locate(fetching)).isInstanceOf(NotFetched.class)
                    .hasMessageContaining("does not match the checksum");
            assertThat(nothingKept(into)).isTrue();
        }
    }

    @Test
    void aBundleHoldsTheThreeFilesAndNoOtherOrMoreThanOnce(@TempDir Path into) throws Exception {
        Map<String, Map<String, byte[]>> refused = new LinkedHashMap<>();
        Map<String, byte[]> all = new LinkedHashMap<>();
        all.put(NativeBundle.DRIVER, bytes("d"));
        all.put(NativeBundle.ARCHIVE, bytes("a"));
        all.put(NativeBundle.REQUIREMENTS, bytes("r"));
        Map<String, byte[]> escaping = new LinkedHashMap<>(all);
        escaping.put("../escaped", bytes("outside"));
        refused.put("a path out of its directory", escaping);
        Map<String, byte[]> extra = new LinkedHashMap<>(all);
        extra.put("extra", bytes("x"));
        refused.put("a file it is not to hold", extra);
        Map<String, byte[]> missing = new LinkedHashMap<>(all);
        missing.remove(NativeBundle.REQUIREMENTS);
        refused.put("a file missing", missing);

        for (Map.Entry<String, Map<String, byte[]>> each : refused.entrySet()) {
            try (Served served = new Served()) {
                byte[] bundle = zip(each.getValue());
                Fetching fetching = releasedWith(served, into, bundle);
                served.files.put(bundleAt(), bundle);

                assertThatThrownBy(() -> NativeBundle.locate(fetching)).as(each.getKey())
                        .isInstanceOf(NotFetched.class);
                assertThat(nothingKept(into)).as(each.getKey()).isTrue();
            }
        }
    }

    @Test
    void aReleaseThatCarriesNoChecksumForThePlatformTakesNoBundle(@TempDir Path into)
            throws Exception {
        try (Served served = new Served()) {
            byte[] bundle = bundle(Map.of(NativeBundle.DRIVER, "d", NativeBundle.ARCHIVE, "a",
                    NativeBundle.REQUIREMENTS, "r"));
            served.files.put(bundleAt(), bundle);

            assertThatThrownBy(() -> NativeBundle.locate(
                    fetching(served, into.resolve("cache"), VERSION, Map.of())))
                    .isInstanceOf(NotFetched.class).hasMessageContaining("no checksum");
            assertThat(served.asked).isEmpty();
        }
    }

    /**
     * A released compiler is run in the middle of somebody's project, and an executable that the
     * project has where a clone keeps its driver is not the driver: it is code the compiler holds no
     * checksum for. The driver is the one named, or the one fetched and checked.
     */
    @Test
    void aDriverInTheDirectoryTheCommandRunsInIsNotTheOneItRuns(@TempDir Path into) throws Exception {
        String named = System.getProperty(NativeCompiler.DRIVER_PROPERTY);
        Path decoy = Path.of("native", "target", "debug", "souther-native-driver");
        try (Served served = new Served()) {
            byte[] bundle = bundle(Map.of(NativeBundle.DRIVER, "#!/bin/sh\n",
                    NativeBundle.ARCHIVE, "archive", NativeBundle.REQUIREMENTS, "-lm\n"));
            Fetching fetching = releasedWith(served, into, bundle);
            served.files.put(bundleAt(), bundle);
            Files.createDirectories(decoy.getParent());
            Files.writeString(decoy, "#!/bin/sh\nexit 0\n");
            assertThat(decoy.toFile().setExecutable(true)).isTrue();
            System.clearProperty(NativeCompiler.DRIVER_PROPERTY);
            try {
                assertThat(NativeCompiler.hasDriver()).as("nothing names a driver").isFalse();

                Ran ran = run(fetching, "--fetch");

                assertThat(ran.printed()).doesNotContain("this build's own")
                        .contains("the driver " + into.resolve("cache/native"));
                assertThat(served.asked).containsEntry(bundleAt(), 1);
            } finally {
                if (named != null) {
                    System.setProperty(NativeCompiler.DRIVER_PROPERTY, named);
                }
                Files.deleteIfExists(decoy);
                for (Path directory = decoy.getParent(); directory != null; directory = directory.getParent()) {
                    Files.deleteIfExists(directory);
                }
            }
        }
    }

    @Test
    void noDriverNamedIsARefusalThatSaysWhereOneIsNamed() throws Exception {
        String named = System.getProperty(NativeCompiler.DRIVER_PROPERTY);
        System.clearProperty(NativeCompiler.DRIVER_PROPERTY);
        try {
            assertThatThrownBy(() -> NativeCompiler.driven("{}")).isInstanceOf(IOException.class)
                    .hasMessageContaining(NativeCompiler.DRIVER_PROPERTY);
        } finally {
            if (named != null) {
                System.setProperty(NativeCompiler.DRIVER_PROPERTY, named);
            }
        }
    }

    @Test
    void theDriverIsNotFetchedWhereTheCommandIsOffline(@TempDir Path into) throws Exception {
        try (Served served = new Served()) {
            byte[] bundle = bundle(Map.of(NativeBundle.DRIVER, "d", NativeBundle.ARCHIVE, "a",
                    NativeBundle.REQUIREMENTS, "r"));
            Fetching fetching = releasedWith(served, into, bundle).withOffline(true);
            served.files.put(bundleAt(), bundle);

            assertThatThrownBy(() -> NativeBundle.locate(fetching)).isInstanceOf(NotFetched.class)
                    .hasMessageContaining("offline");
            assertThat(served.asked).isEmpty();
        }
    }

    @Test
    void aPlatformIsNamedAsItsBundleIs() throws Exception {
        assertThat(NativeBundle.platform("Mac OS X", "aarch64")).isEqualTo("macos-aarch64");
        assertThat(NativeBundle.platform("Mac OS X", "x86_64")).isEqualTo("macos-x86_64");
        assertThat(NativeBundle.platform("Linux", "amd64")).isEqualTo("linux-x86_64");
        assertThat(NativeBundle.platform("Linux", "aarch64")).isEqualTo("linux-aarch64");
        assertThatThrownBy(() -> NativeBundle.platform("Windows 11", "amd64"))
                .isInstanceOf(NotFetched.class);
        assertThatThrownBy(() -> NativeBundle.platform("Linux", "riscv64"))
                .isInstanceOf(NotFetched.class);
    }

    /** A fetching whose release carries the checksum of {@code bundle} for this platform. */
    private static Fetching releasedWith(Served served, Path into, byte[] bundle) throws Exception {
        return fetching(served, into.resolve("cache"), VERSION,
                Map.of(ReleaseChecksums.bundle(NativeBundle.platform()), Fetching.sha256(bundle)));
    }

    private static String bundleAt() throws Exception {
        return "/releases/v" + VERSION + "/souther-native-" + VERSION + "-" + NativeBundle.platform()
                + ".zip";
    }

    /** Whether the cache holds no driver and nothing half unpacked. */
    private static boolean nothingKept(Path into) throws IOException {
        Path native_ = into.resolve("cache/native");
        if (!Files.exists(native_)) {
            return true;
        }
        try (Stream<Path> kept = Files.walk(native_)) {
            return kept.noneMatch(each -> Files.isRegularFile(each)
                    || each.getFileName().toString().contains(".partial"));
        }
    }

    private static List<Path> list(Path directory) throws IOException {
        try (Stream<Path> all = Files.list(directory)) {
            return all.toList();
        }
    }

    private static Ran build(Path into, String name, Fetching fetching, String... options)
            throws Exception {
        Path model = into.resolve(name + "/model");
        Files.createDirectories(model);
        Files.writeString(model.resolve("money.sou"), MONEY, StandardCharsets.UTF_8);
        String[] command = Stream.concat(Stream.of(options), Stream.of("--library",
                into.resolve(name + "/native").toString(), "--php", into.resolve(name + "/out").toString(),
                "--namespace", "Acme", model.toString())).toArray(String[]::new);
        return run(fetching, command);
    }

    private static Ran run(Fetching fetching, String... command) throws NotInstalled {
        ByteArrayOutputStream printed = new ByteArrayOutputStream();
        ByteArrayOutputStream said = new ByteArrayOutputStream();
        int ended = Main.run(command, new PrintStream(printed, true, StandardCharsets.UTF_8),
                new PrintStream(said, true, StandardCharsets.UTF_8), Bindings.of(List.of()),
                fetching);
        return new Ran(ended, printed.toString(StandardCharsets.UTF_8),
                said.toString(StandardCharsets.UTF_8));
    }

    private static byte[] bytes(String text) {
        return text.getBytes(StandardCharsets.UTF_8);
    }

    private static byte[] bundle(Map<String, String> files) throws IOException {
        Map<String, byte[]> bytes = new LinkedHashMap<>();
        files.forEach((name, content) -> bytes.put(name, bytes(content)));
        return zip(bytes);
    }

    private static byte[] zip(Map<String, byte[]> files) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        try (ZipOutputStream zip = new ZipOutputStream(out)) {
            for (Map.Entry<String, byte[]> each : files.entrySet()) {
                zip.putNextEntry(new ZipEntry(each.getKey()));
                zip.write(each.getValue());
                zip.closeEntry();
            }
        }
        return out.toByteArray();
    }

    /** A jar of the generator a test serves, found the way a real one is: through its service file. */
    private static byte[] jar() throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        try (JarOutputStream jar = new JarOutputStream(out)) {
            jar.putNextEntry(new JarEntry("META-INF/services/souther.bindings.BindingGenerator"));
            jar.write(bytes(FetchedGenerator.class.getName() + "\n"));
            jar.closeEntry();
            String entry = FetchedGenerator.class.getName().replace('.', '/') + ".class";
            jar.putNextEntry(new JarEntry(entry));
            try (InputStream in = FetchedGenerator.class.getClassLoader().getResourceAsStream(entry)) {
                jar.write(in.readAllBytes());
            }
            jar.closeEntry();
        }
        return out.toByteArray();
    }
}
