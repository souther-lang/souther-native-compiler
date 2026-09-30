package souther.nativecode;

import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * A checked program as an object file for the machine this runs on.
 *
 * <p>The lowering is not here. This writes the program out, hands it to the driver, and takes back
 * what Cranelift wrote — a process and not a call into a library, because a program crosses once
 * per build and a panic on that side ends that process rather than this one.
 */
public final class NativeCompiler {

    /**
     * Where the driver is: the one place it is looked for. A clone names the one Cargo built
     * ({@code scripts/souther-native} does, and so do the tests), and a released compiler names the one
     * it fetched and checked. It is not looked for in the directory the command is run in, which a
     * released compiler is run in the middle of somebody's project, and whose executable it would
     * then run in place of the one it holds a checksum for.
     */
    public static final String DRIVER_PROPERTY = "souther.native.driver";

    /**
     * What the driver answering this means: the program is one the language admits and the backend
     * does not write yet.
     *
     * <p>The driver's own {@code ended} module is the other half of this, and neither compiler
     * checks the two agree. What holds them together is that a program the driver refuses is
     * compiled the whole way through in a test, so the number being wrong is a red test rather
     * than a refusal quietly reported as something else.
     */
    private static final int NOT_LOWERED = 2;

    private NativeCompiler() {
    }

    /**
     * Whether a driver is named by {@link #DRIVER_PROPERTY}, and by nothing else. Where none is, a
     * released compiler fetches its own.
     */
    public static boolean hasDriver() {
        return System.getProperty(DRIVER_PROPERTY) != null;
    }

    /** The object holding every behavior the program declares. */
    public static byte[] compile(CheckedProgram program) throws IOException, InterruptedException {
        return driven(ProgramWriter.written(program));
    }

    /**
     * What a build for a host writes: the object; a header a C or C++ compiler includes; the
     * declarations it includes, which are C and nothing a preprocessor has to run over, for an FFI
     * that reads C declarations; a manifest describing the same functions in the model's terms;
     * and a shared library of the object and the runtime exporting those functions and nothing
     * else.
     */
    public record Library(Path object, Path header, Path declarations, Path manifest,
                          Path library) {}

    /** The program built for a host, into {@code into}, reaching no other build's object. */
    public static Library library(CheckedProgram program, Path into)
            throws IOException, InterruptedException {
        return library(program, List.of(), into);
    }

    /**
     * The program built for a host, into {@code into}.
     *
     * <p>{@code alongside} is every object another build wrote that the program reaches: the same
     * objects an executable of it would be linked with. Each carries what it offers a host, and
     * what the library offers is what all of them carry beside this program's object. A behavior
     * with no body is one of them too: the object of the build that declares it makes the
     * capability a host's implementation of it is handed over as, so nothing is left to link in
     * besides.
     *
     * <p>All of it is written by the driver, from what the objects' emission decided. Nothing here
     * reads the program to say what a host can call: that would be a second answer to a question
     * the driver already answered while writing each object.
     */
    public static Library library(CheckedProgram program, List<byte[]> alongside, Path into)
            throws IOException, InterruptedException {
        return library(ProgramWriter.written(program), alongside, into);
    }

    /**
     * As {@link #library(CheckedProgram, List, Path)}, handed to {@code driver} and not to the one
     * {@link #DRIVER_PROPERTY} names: for a caller that chose its driver itself, as the testkit does.
     */
    public static Library library(CheckedProgram program, List<byte[]> alongside, Path into,
                                  Path driver) throws IOException, InterruptedException {
        return library(ProgramWriter.written(program), alongside, into, driver);
    }

    /**
     * The driver of this release for this platform, fetched and kept where it is not, and held to the
     * checksum the release carries for it; refused where this is not a release.
     */
    public static Path releasedDriver() throws IOException {
        return NativeBundle.locate(Fetching.standard());
    }

    /**
     * The library a transport document is built into, reaching no other build's object.
     * Package-visible for a test that asks what a binding makes of a document no checked program of
     * today's language writes, as {@link #driven} is for an object.
     */
    static Library library(String document, Path into) throws IOException, InterruptedException {
        return library(document, List.of(), into);
    }

    private static Library library(String document, List<byte[]> alongside, Path into)
            throws IOException, InterruptedException {
        return library(document, alongside, into, driver());
    }

    private static Library library(String document, List<byte[]> alongside, Path into,
                                   Path driver) throws IOException, InterruptedException {
        Path handed = Files.createTempDirectory("souther-native-alongside");
        try {
            List<String> arguments = new ArrayList<>(
                    List.of("--library", into.toAbsolutePath().toString()));
            for (int at = 0; at < alongside.size(); at++) {
                Path object = handed.resolve(at + ".o");
                Files.write(object, alongside.get(at));
                arguments.add("--with");
                arguments.add(object.toString());
            }
            byte[] said = run(document, arguments, driver);
            // Where the driver wrote each, one to a line, which is how what a shared library is
            // called on this host is said by the side that named it.
            List<Path> written =
                    new String(said, StandardCharsets.UTF_8).lines().map(Path::of).toList();
            if (written.size() != 5) {
                throw new IOException("the driver said it wrote " + written);
            }
            return new Library(written.get(0), written.get(1), written.get(2), written.get(3),
                    written.get(4));
        } finally {
            try (var files = Files.list(handed)) {
                for (Path file : files.toList()) {
                    Files.delete(file);
                }
            }
            Files.delete(handed);
        }
    }

    /**
     * The object a transport document is compiled to. Package-visible for a test that asks what the
     * driver does with a document no checked program of today's language writes.
     */
    static byte[] driven(String document) throws IOException, InterruptedException {
        return run(document, List.of(), driver());
    }

    /** What the driver writes on stdout when handed the document with these arguments. */
    private static byte[] run(String document, List<String> arguments, Path driver)
            throws IOException, InterruptedException {
        if (!Files.isExecutable(driver)) {
            throw new IOException("no driver at " + driver.toAbsolutePath()
                    + ", which `cargo build` in native/ writes");
        }
        List<String> command = new ArrayList<>();
        command.add(driver.toString());
        command.addAll(arguments);

        // What the driver says goes to a file rather than to a pipe this side reads second: two
        // pipes read one after the other deadlock where the one not being read fills up first.
        Path said = Files.createTempFile("souther-native-", ".problems");
        try {
            Process process = new ProcessBuilder(command)
                    .redirectError(said.toFile())
                    .start();

            byte[] object;
            try (OutputStream to = process.getOutputStream()) {
                to.write(document.getBytes(StandardCharsets.UTF_8));
            }
            try (InputStream from = process.getInputStream()) {
                object = from.readAllBytes();
            }

            int ended = process.waitFor();
            if (ended == NOT_LOWERED) {
                throw new NotLowered(Files.readString(said, StandardCharsets.UTF_8).strip());
            }
            if (ended != 0) {
                throw new IOException("the driver could not do it: "
                        + Files.readString(said, StandardCharsets.UTF_8).strip());
            }
            return object;
        } finally {
            Files.deleteIfExists(said);
        }
    }

    private static Path driver() throws IOException {
        String named = System.getProperty(DRIVER_PROPERTY);
        if (named == null) {
            throw new IOException("no driver is named: " + DRIVER_PROPERTY + " is the one place it is"
                    + " looked for (a released compiler fetches its own, and scripts/souther-native"
                    + " names a clone's)");
        }
        return Path.of(named);
    }
}
