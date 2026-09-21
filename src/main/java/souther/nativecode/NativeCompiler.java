package souther.nativecode;

import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * A checked program as an object file for the machine this runs on.
 *
 * <p>The lowering is not here. This writes the program out, hands it to the driver, and takes back
 * what Cranelift wrote — a process and not a call into a library, because a program crosses once
 * per build and a panic on that side ends that process rather than this one.
 */
public final class NativeCompiler {

    /** Where the driver is, for a caller whose build puts it somewhere else. */
    public static final String DRIVER_PROPERTY = "souther.native.driver";

    private static final Path BUILT =
            Path.of("native", "target", "debug", "souther-native-driver");

    private NativeCompiler() {
    }

    /** The object holding every behavior the program declares. */
    public static byte[] compile(CheckedProgram program) throws IOException, InterruptedException {
        return driven(ProgramWriter.written(program));
    }

    private static byte[] driven(String document) throws IOException, InterruptedException {
        Path driver = driver();
        if (!Files.isExecutable(driver)) {
            throw new IOException("no driver at " + driver.toAbsolutePath()
                    + ", which `cargo build` in native/ writes");
        }

        // What the driver says goes to a file rather than to a pipe this side reads second: two
        // pipes read one after the other deadlock where the one not being read fills up first.
        Path said = Files.createTempFile("souther-native-", ".problems");
        try {
            Process process = new ProcessBuilder(driver.toString())
                    .redirectError(said.toFile())
                    .start();

            byte[] object;
            try (OutputStream to = process.getOutputStream()) {
                to.write(document.getBytes(StandardCharsets.UTF_8));
            }
            try (InputStream from = process.getInputStream()) {
                object = from.readAllBytes();
            }

            if (process.waitFor() != 0) {
                throw new IOException("the driver refused it: "
                        + Files.readString(said, StandardCharsets.UTF_8).strip());
            }
            return object;
        } finally {
            Files.deleteIfExists(said);
        }
    }

    private static Path driver() {
        String named = System.getProperty(DRIVER_PROPERTY);
        return named == null ? BUILT : Path.of(named);
    }
}
