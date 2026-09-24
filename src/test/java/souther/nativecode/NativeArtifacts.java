package souther.nativecode;

import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * What the tests build, built once per what it is built from.
 *
 * <p>Each stage is decided by its own inputs and by nothing after it. An object is decided by the
 * transport document; an executable by the object, the objects linked beside it, and the C source
 * of the harness; which entry a run reaches is decided when it runs and is in neither. So the
 * object is kept under the document itself, and the executable under the document, the bytes
 * beside it and the harness source itself. Nothing is reduced to a digest or to a description of
 * what the harness says, because a key that describes has to be told each time the harness learns
 * to say something more, and a key that is the source is not.
 *
 * <p>Held for as long as the JVM runs the tests and removed when it stops, so no test owns
 * anything it was handed. Asking for one that is being built waits for that build instead of
 * starting a second.
 */
final class NativeArtifacts {

    /**
     * Bytes as a value: copied when made and when read out, and equal by what they hold. An array
     * handed across this boundary stays its owner's, so what the cache keys on, what it links and
     * what it hands back are the same bytes and nobody's later write reaches them.
     */
    static final class Bytes {
        private final byte[] held;

        Bytes(byte[] given) {
            this.held = given.clone();
        }

        byte[] copy() {
            return held.clone();
        }

        @Override
        public boolean equals(java.lang.Object other) {
            return other instanceof Bytes it && Arrays.equals(held, it.held);
        }

        @Override
        public int hashCode() {
            return Arrays.hashCode(held);
        }
    }

    /** What the linker was handed to make one executable. */
    private record Linked(String document, List<Bytes> alongside, String harness) {}

    /** An object, and the symbols it defines. */
    record Built(Bytes bytes, Set<String> defined) {
        Built {
            defined = Set.copyOf(defined);
        }
    }

    private static final ConcurrentMap<String, Built> OBJECTS = new ConcurrentHashMap<>();
    private static final ConcurrentMap<Linked, Path> EXECUTABLES = new ConcurrentHashMap<>();

    private static final ConcurrentMap<String, AtomicInteger> COMPILED = new ConcurrentHashMap<>();
    private static final ConcurrentMap<String, AtomicInteger> LINKED = new ConcurrentHashMap<>();

    private static final AtomicInteger NUMBER = new AtomicInteger();

    /** What every executable is linked against. */
    private static final Path RUNTIME =
            Path.of("native", "target", "debug", "libsouther_native_runtime.a");

    private static final Path ROOT = root();

    private NativeArtifacts() {}

    private static Path root() {
        try {
            Path root = Files.createTempDirectory("souther-native-artifacts");
            Runtime.getRuntime().addShutdownHook(new Thread(() -> remove(root)));
            return root;
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    /**
     * The program compiled to an object, for a test that needs one to link and is not asking
     * about the compiler. A test whose question is what compiling does calls
     * {@link NativeCompiler#compile} itself, so that what it observes is not an earlier answer.
     */
    static byte[] object(CheckedProgram program) throws IOException, InterruptedException {
        return built(program).bytes().copy();
    }

    static Built built(CheckedProgram program) throws IOException, InterruptedException {
        String document = ProgramWriter.written(program);
        try {
            return OBJECTS.computeIfAbsent(document, ignored -> {
                try {
                    COMPILED.computeIfAbsent(document, d -> new AtomicInteger()).incrementAndGet();
                    byte[] bytes = NativeCompiler.compile(program);
                    return new Built(new Bytes(bytes), definedIn(bytes));
                } catch (IOException e) {
                    throw new UncheckedIOException(e);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new UncheckedIOException(new IOException("interrupted while compiling"));
                }
            });
        } catch (UncheckedIOException e) {
            throw e.getCause();
        }
    }

    /** The executable the linker makes of these, which is one wherever and however often asked. */
    static Path executable(CheckedProgram program, List<Bytes> alongside, String harness)
            throws IOException, InterruptedException {
        String document = ProgramWriter.written(program);
        Linked key = new Linked(document, List.copyOf(alongside), harness);
        Path already = EXECUTABLES.get(key);
        if (already != null) {
            return already;
        }
        Bytes object = built(program).bytes();
        try {
            return EXECUTABLES.computeIfAbsent(key, ignored -> {
                try {
                    LINKED.computeIfAbsent(document, d -> new AtomicInteger()).incrementAndGet();
                    return link(object, alongside, harness);
                } catch (IOException e) {
                    throw new UncheckedIOException(e);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new UncheckedIOException(new IOException("interrupted while linking"));
                }
            });
        } catch (UncheckedIOException e) {
            throw e.getCause();
        }
    }

    /** How many times this program was compiled, for a test holding that to be once. */
    static int compilationsOf(CheckedProgram program) {
        AtomicInteger count = COMPILED.get(ProgramWriter.written(program));
        return count == null ? 0 : count.get();
    }

    /** How many executables were linked of this program, whatever they were linked with. */
    static int linksOf(CheckedProgram program) {
        AtomicInteger count = LINKED.get(ProgramWriter.written(program));
        return count == null ? 0 : count.get();
    }

    private static Path link(Bytes object, List<Bytes> alongside, String harness)
            throws IOException, InterruptedException {
        Path into = Files.createDirectory(ROOT.resolve(Integer.toString(NUMBER.getAndIncrement())));
        Path source = into.resolve("harness.c");
        Files.writeString(source, harness, StandardCharsets.UTF_8);
        Path program = into.resolve("program.o");
        Files.write(program, object.copy());
        Path executable = into.resolve("run");

        List<String> command = new ArrayList<>(List.of("cc", "-o", executable.toString(),
                source.toString(), program.toString()));
        for (int at = 0; at < alongside.size(); at++) {
            Path beside = into.resolve("alongside." + at + ".o");
            Files.write(beside, alongside.get(at).copy());
            command.add(beside.toString());
        }
        command.add(RUNTIME.toString());
        said(command);
        return executable;
    }

    /**
     * Every symbol an object defines, spelt the way the linker on this platform spells it.
     *
     * <p>What the object carries is asked of the object. A test that worked it out from what the
     * compiler is documented to emit would hold its own reading of that against itself.
     */
    private static Set<String> definedIn(byte[] bytes) throws IOException, InterruptedException {
        Path file = Files.createTempFile(ROOT, "symbols", ".o");
        Files.write(file, bytes);
        Set<String> defined = new HashSet<>();
        for (String line : said(List.of("nm", "-g", file.toString())).lines().toList()) {
            String[] fields = line.trim().split("\\s+");
            // `address type name`; an undefined symbol has no address and is not defined here.
            if (fields.length == 3 && !fields[1].equals("U")) {
                defined.add(fields[2]);
            }
        }
        Files.delete(file);
        return defined;
    }

    private static String said(List<String> command) throws IOException, InterruptedException {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError(command.get(0) + " failed: " + said);
        }
        return said;
    }

    private static void remove(Path root) {
        try (var walked = Files.walk(root)) {
            walked.sorted((a, b) -> b.getNameCount() - a.getNameCount()).forEach(it -> {
                try {
                    Files.deleteIfExists(it);
                } catch (IOException e) {
                    // Left for the operating system's own temporary cleanup.
                }
            });
        } catch (IOException e) {
            // The same.
        }
    }
}
