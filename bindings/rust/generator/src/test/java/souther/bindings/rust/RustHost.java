package souther.bindings.rust;

import souther.bindings.BindingInput;
import souther.bindings.Declarations;
import souther.bindings.Generated;
import souther.bindings.Manifest;
import souther.nativecode.NativeCompiler;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import souther.nativecode.Repository;

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
    static final Path RUNTIME = Repository.file("bindings", "rust", "runtime").toAbsolutePath();

    /**
     * Where every host is built: the runtime's own target. Within a run, what a host shares with
     * the runtime (the runtime itself, Raoh and what those depend on) is built once, by whichever
     * of the runtime's tests and the hosts gets to it first. Between runs, CI's cache of that
     * target keeps the dependencies and not the runtime: rust-cache drops a workspace's own crates
     * and whatever else is not one of its dependencies, the hosts among them.
     */
    private static final Path TARGET = RUNTIME.resolve("target");

    private RustHost() {
    }

    /** The binding of {@code library} generated into {@code into} as the crate {@code crate}. */
    static Generated generated(NativeCompiler.Library library, Path into, String crate)
            throws IOException {
        return RustBindings.generate(new BindingInput(Manifest.read(library.manifest()),
                Declarations.at(library.declarations())), into, crate);
    }

    /**
     * What a host whose {@code main.rs} is {@code main} printed, built beside {@code binding} in
     * {@code into} and run with {@code arguments}, where the binding passes Clippy with every
     * warning an error and the host built and ran with nothing said.
     *
     * <p>The two are one Cargo workspace, which is where the runtime crate is patched in, so that
     * Clippy reads what the generator wrote as a crate of the workspace's own: a lint it finds there
     * is the generator's to answer for, and none is allowed but the ones the crate says it allows.
     */
    static String ran(Path into, Generated binding, String crate, String main,
                      List<String> arguments) throws IOException, InterruptedException {
        handsOverOnlyWhatItChecks(binding);
        exposesNoNativeWord(binding);
        reservesEveryRootName(binding);
        workspace(into, binding, crate, main);
        String manifest = into.resolve("Cargo.toml").toString();
        cargo(List.of("clippy", "--quiet", "--manifest-path", manifest, "-p", crate, "--",
                "-D", "warnings"));
        List<String> run = new ArrayList<>(List.of("run", "--quiet", "--manifest-path", manifest,
                "-p", "host", "--"));
        run.addAll(arguments);
        return cargo(run);
    }

    /** Writes the workspace of the generated crate and a host whose {@code main.rs} is {@code main}. */
    private static void workspace(Path into, Generated binding, String crate,
                                  String main) throws IOException {
        Path host = into.resolve("host");
        Files.createDirectories(host.resolve("src"));
        Files.writeString(into.resolve("Cargo.toml"), """
                [workspace]
                resolver = "3"
                members = ["host", "%s"]

                [patch.crates-io]
                souther-binding-runtime = { path = "%s" }
                """.formatted(toml(into.relativize(binding.root())), toml(RUNTIME)),
                StandardCharsets.UTF_8);
        Files.writeString(host.resolve("Cargo.toml"), """
                [package]
                name = "host"
                version = "0.0.0"
                edition = "2024"
                publish = false

                [dependencies]
                %s = { path = "%s" }
                """.formatted(crate, toml(binding.root())), StandardCharsets.UTF_8);
        Files.writeString(host.resolve("src").resolve("main.rs"), main, StandardCharsets.UTF_8);
    }

    /** Where a function of the generated crate starts. */
    private static final java.util.regex.Pattern FUNCTION = java.util.regex.Pattern.compile(
            "(?m)^\\s*(?:pub(?:\\(crate\\))? )?(?:unsafe )?(?:extern \"C\" )?fn ");

    /**
     * Refuses a generated function that calls into the library and reaches a value's address
     * through what reads a value ({@code own()}): what a computation is handed goes through what
     * checks the runtime that made it ({@code word_in}), and a function doing both is one that
     * could hand over what it read without the check. Held here, on every crate a test generates,
     * so that the generator cannot come to write one without a test saying so.
     */
    private static void handsOverOnlyWhatItChecks(Generated binding)
            throws IOException {
        for (Path file : binding.files()) {
            if (!file.toString().endsWith(".rs")) {
                continue;
            }
            String written = Files.readString(file, StandardCharsets.UTF_8);
            java.util.regex.Matcher starts = FUNCTION.matcher(written);
            List<Integer> at = new ArrayList<>();
            while (starts.find()) {
                at.add(starts.start());
            }
            at.add(written.length());
            for (int one = 0; one + 1 < at.size(); one++) {
                String body = written.substring(at.get(one), at.get(one + 1));
                if (body.contains(".own()") && body.contains("run.call(")) {
                    throw new AssertionError(file + " calls into the library in a function that"
                            + " reads a value's address unchecked:\n" + body);
                }
            }
        }
    }

    /**
     * What a public item of the generated crate is written with: a line declaring one, up to where
     * its body or its fields begin. A native word or a handle the runtime made ({@code rt::Held},
     * {@code rt::Word}) written in one is a way for safe Rust to hold a native value under a type
     * that says nothing of what it is, and to put it under another: a value of one function type
     * in another's variant, an address of one type where a call reads another. What is public says
     * what a value is; the words stay in what is {@code pub(crate)} or hidden.
     */
    private static final java.util.regex.Pattern PUBLIC_ITEM = java.util.regex.Pattern.compile(
            "(?m)^(?<attributes>(?:\\s*#\\[[^\\n]*\\]\\n)*)\\s*pub (?!\\(crate\\))(?<head>[^\\n{;]*)");

    /**
     * Refuses a public item of a generated crate that names {@code rt::Held} or {@code rt::Word}
     * and is not hidden from the documentation, which is where a name for what the crate itself
     * calls (a constructor the library answers through) is kept out of what a host is told of.
     */
    private static void exposesNoNativeWord(Generated binding) throws IOException {
        for (Path file : binding.files()) {
            if (!file.toString().endsWith(".rs")) {
                continue;
            }
            java.util.regex.Matcher item =
                    PUBLIC_ITEM.matcher(Files.readString(file, StandardCharsets.UTF_8));
            while (item.find()) {
                String head = item.group("head");
                boolean hidden = item.group("attributes").contains("doc(hidden)");
                if (!hidden && (head.contains("rt::Held") || head.contains("rt::Word"))) {
                    throw new AssertionError(file + " shows a native value in what a host reads: "
                            + item.group().strip());
                }
            }
        }
    }

    /** A name the root of a generated crate declares in the namespace its modules are in. */
    private static final java.util.regex.Pattern ROOT_ITEM = java.util.regex.Pattern.compile(
            "(?m)^(?<pub>pub )?(?<kind>type|struct|enum|trait|union|mod) (?<name>\\w+)");

    /** What the root of a generated crate takes from the runtime, one {@code pub use} of a list. */
    private static final java.util.regex.Pattern ROOT_USE = java.util.regex.Pattern.compile(
            "(?m)^pub use souther_binding_runtime::\\{(?<names>[^}]*)}");

    /**
     * Refuses a name the root of a generated crate declares that {@link RustBindings#ROOT} does not
     * reserve: a top module of a model named so passes the generator and fails in rustc. A
     * {@code pub mod} alone is the model's own, one of its top modules.
     */
    private static void reservesEveryRootName(Generated binding) throws IOException {
        Path lib = binding.root().resolve("src").resolve("lib.rs");
        String written = Files.readString(lib, StandardCharsets.UTF_8);
        List<String> declared = new ArrayList<>();
        java.util.regex.Matcher item = ROOT_ITEM.matcher(written);
        while (item.find()) {
            if (!(item.group("pub") != null && item.group("kind").equals("mod"))) {
                declared.add(item.group("name"));
            }
        }
        java.util.regex.Matcher uses = ROOT_USE.matcher(written);
        if (!uses.find()) {
            throw new AssertionError(lib + " takes nothing from the runtime in the form this reads");
        }
        for (String name : uses.group("names").split(",")) {
            if (!name.isBlank()) {
                declared.add(name.strip());
            }
        }
        List<String> unreserved = declared.stream()
                .filter(it -> !RustBindings.ROOT.contains(it)).toList();
        if (!unreserved.isEmpty()) {
            throw new AssertionError(lib + " declares at its root what no top module is kept from: "
                    + unreserved);
        }
    }

    /**
     * What rustc said of a host that must not build, built beside {@code binding} the way
     * {@link #ran} builds one: where it built, the test fails, since what it asserts is that the
     * crate refuses it.
     *
     * @return what Cargo said, for the test to hold to the reason
     */
    static String refused(Path into, Generated binding, String crate, String main)
            throws IOException, InterruptedException {
        workspace(into, binding, crate, main);
        String manifest = into.resolve("Cargo.toml").toString();
        Path said = Files.createTempFile("cargo-", ".said");
        try {
            ProcessBuilder builder = new ProcessBuilder("cargo", "build", "--quiet",
                    "--manifest-path", manifest, "-p", "host").redirectError(said.toFile())
                    .redirectOutput(ProcessBuilder.Redirect.DISCARD);
            builder.environment().put("CARGO_TARGET_DIR", TARGET.toString());
            builder.environment().remove("RUSTFLAGS");
            builder.environment().put("CARGO_TERM_COLOR", "never");
            int status = builder.start().waitFor();
            String saidThere = Files.readString(said, StandardCharsets.UTF_8);
            if (status == 0) {
                throw new AssertionError("a host that must not build built, beside " + binding.root());
            }
            return saidThere;
        } finally {
            Files.deleteIfExists(said);
        }
    }

    /** What Cargo printed, run with {@code arguments}, where it ended well and said nothing else. */
    private static String cargo(List<String> arguments) throws IOException, InterruptedException {
        List<String> command = new ArrayList<>(List.of("cargo"));
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
                throw new AssertionError(command + " ended with " + status + "\nprinted:\n"
                        + printed + "\nsaid:\n" + saidThere);
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
