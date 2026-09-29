package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Every rust-cache step in a workflow is keyed on what decides a Rust build and rust-cache does not
 * read itself: each virtual workspace root, and Cargo's config files.
 *
 * <p>rust-cache keys on the manifests of a workspace's packages and its lock, and not on a virtual
 * workspace's root or a Cargo config, and a key that matches in full is not saved again. A profile
 * set in {@code native/Cargo.toml} was therefore rebuilt on every CI run from a cache that never
 * held it (#133), with every check green. The key given is also part of the key a run falls back
 * to, so it holds no more than that: every {@code Cargo.toml} in it made a change rust-cache would
 * have restored most of into a build from nothing, which the next run after a key of every
 * manifest showed. So the key is held to exactly the roots this repository has, found here rather
 * than listed, and a root added later is a key to change.
 */
class EveryRustCacheIsKeptUnderWhatDecidesTheBuildTest {

    /** A step using rust-cache, and what it is written with up to the next step. */
    private static final Pattern STEP = Pattern.compile(
            "^([ ]*)- uses: Swatinem/rust-cache@\\S+[ ]*\\n((?:\\1  .*\\n|[ ]*\\n)*)",
            Pattern.MULTILINE);

    private static final Pattern WORKSPACE = Pattern.compile("^\\[workspace\\]\\s*$", Pattern.MULTILINE);
    private static final Pattern PACKAGE = Pattern.compile("^\\[package\\]\\s*$", Pattern.MULTILINE);

    @Test
    void everyRustCacheStepIsKeyedOnWhatRustCacheDoesNotRead() throws IOException {
        String key = key();
        List<String> keyedOtherwise = new ArrayList<>();
        int steps = 0;
        try (Stream<Path> workflows = Files.list(Repository.file(".github", "workflows"))) {
            for (Path workflow : workflows.filter(it -> it.toString().endsWith(".yml")).toList()) {
                Matcher step = STEP.matcher(Files.readString(workflow));
                while (step.find()) {
                    steps++;
                    if (!step.group(2).lines().map(String::strip).toList().contains(key)) {
                        keyedOtherwise.add(workflow.getFileName() + ": " + step.group().strip());
                    }
                }
            }
        }
        assertThat(steps).as("rust-cache steps found").isPositive();
        assertThat(keyedOtherwise).as("rust-cache steps not keyed `%s`", key).isEmpty();
    }

    /** The key every step is to carry: the virtual roots found, then Cargo's config files. */
    private static String key() throws IOException {
        Path root = Repository.root();
        List<Path> manifests = new ArrayList<>();
        Files.walkFileTree(root, new SimpleFileVisitor<>() {
            @Override
            public FileVisitResult preVisitDirectory(Path directory, BasicFileAttributes attributes) {
                return SKIPPED.contains(directory.getFileName().toString())
                        ? FileVisitResult.SKIP_SUBTREE : FileVisitResult.CONTINUE;
            }

            @Override
            public FileVisitResult visitFile(Path file, BasicFileAttributes attributes) {
                if (file.getFileName().toString().equals("Cargo.toml")) {
                    manifests.add(root.relativize(file));
                }
                return FileVisitResult.CONTINUE;
            }
        });
        List<String> hashed = new ArrayList<>();
        for (Path manifest : manifests.stream().sorted().toList()) {
            String text = Files.readString(root.resolve(manifest));
            if (WORKSPACE.matcher(text).find() && !PACKAGE.matcher(text).find()) {
                hashed.add("'" + manifest.toString().replace('\\', '/') + "'");
            }
        }
        assertThat(hashed).as("virtual workspace roots found").isNotEmpty();
        hashed.add("'**/.cargo/config.toml'");
        hashed.add("'**/.cargo/config'");
        return "key: ${{ hashFiles(" + String.join(", ", hashed) + ") }}";
    }

    /** What is built or checked out beside the sources, where a manifest is not the repository's. */
    private static final Set<String> SKIPPED =
            Set.of("target", ".git", ".wt", "vendor", "node_modules");
}
