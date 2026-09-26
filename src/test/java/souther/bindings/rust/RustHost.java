package souther.bindings.rust;

import souther.nativecode.NativeCompiler;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * A Rust host of a library a test built, as an application would be one: a crate depending on the
 * binding generated from the library's manifest, with the runtime crate this repository holds
 * patched in for the one a registry would have, built with Cargo and run.
 *
 * <p>What the host prints is what a test compares. What Cargo and rustc say go apart from it, and a
 * run saying anything there fails: a warning in what the binding wrote, or in the host, is a
 * failure, since a binding a host cannot build without warnings is one it builds with
 * {@code -D warnings} at its peril.
 */
final class RustHost {

    /** Where the runtime crate stands. */
    static final Path RUNTIME = Path.of("bindings", "rust", "runtime").toAbsolutePath();

    /**
     * Where every host is built: one directory for all of them, so what they share (the runtime,
     * Raoh and what those depend on) is built once for a run of the tests.
     */
    private static final Path TARGET = Path.of("target", "rust-hosts").toAbsolutePath();

    private RustHost() {
    }

    /** The binding of {@code library} generated into {@code into} as the crate {@code crate}. */
    static RustBindings.Generated generated(NativeCompiler.Library library, Path into, String crate)
            throws IOException {
        return RustBindings.generate(library.manifest(), into, crate);
    }

    /**
     * What a host whose {@code main.rs} is {@code main} printed, built beside {@code binding} in
     * {@code into} and run with {@code arguments}, where it built and ran with nothing said.
     */
    static String ran(Path into, RustBindings.Generated binding, String crate, String main,
                      List<String> arguments) throws IOException, InterruptedException {
        Path host = into.resolve("host");
        Files.createDirectories(host.resolve("src"));
        Files.writeString(host.resolve("Cargo.toml"), """
                [package]
                name = "host"
                version = "0.0.0"
                edition = "2024"
                publish = false

                [dependencies]
                %s = { path = "%s" }

                [patch.crates-io]
                souther-binding-runtime = { path = "%s" }
                """.formatted(crate, toml(binding.root()), toml(RUNTIME)), StandardCharsets.UTF_8);
        Files.writeString(host.resolve("src").resolve("main.rs"), main, StandardCharsets.UTF_8);
        List<String> command = new ArrayList<>(List.of("cargo", "run", "--quiet",
                "--manifest-path", host.resolve("Cargo.toml").toString(), "--"));
        command.addAll(arguments);
        // What it says goes to a file rather than to a pipe read second: two pipes read one after
        // the other deadlock where the one not being read fills up first.
        Path said = Files.createTempFile("cargo-", ".said");
        try {
            ProcessBuilder builder = new ProcessBuilder(command).redirectError(said.toFile());
            builder.environment().put("CARGO_TARGET_DIR", TARGET.toString());
            // Nothing a developer's own settings add: a warning is a warning here whatever they
            // set, and colour would be bytes in what is compared.
            builder.environment().remove("RUSTFLAGS");
            builder.environment().put("CARGO_TERM_COLOR", "never");
            Process process = builder.start();
            String printed =
                    new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            int status = process.waitFor();
            String saidThere = Files.readString(said, StandardCharsets.UTF_8);
            if (status != 0 || !saidThere.isEmpty()) {
                throw new AssertionError("cargo run in " + host + " ended with " + status
                        + "\nprinted:\n" + printed + "\nsaid:\n" + saidThere);
            }
            return printed;
        } finally {
            Files.deleteIfExists(said);
        }
    }

    /** {@code path} as a TOML basic string holds it. */
    private static String toml(Path path) {
        return path.toString().replace("\\", "\\\\").replace("\"", "\\\"");
    }
}
