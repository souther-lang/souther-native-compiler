package souther.nativecode;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Predicate;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Where the repository is, for a test that reads what is not in its own module: the Rust half, the
 * language runtimes, and the other modules' tests.
 *
 * <p>A test runs in its module's directory and is told the root by Maven ({@code souther.repository}
 * in the surefire configuration), so a path names a place in the repository and not a number of
 * directories up from wherever a module happens to be.
 */
public final class Repository {

    private static final Pattern MODULE = Pattern.compile("<module>([^<]+)</module>");

    private Repository() {
    }

    /** The root of the clone. */
    public static Path root() {
        String named = System.getProperty("souther.repository");
        if (named == null || named.isBlank()) {
            throw new IllegalStateException("souther.repository is not set: the tests are run"
                    + " through Maven, which says where the repository is");
        }
        return Path.of(named).toAbsolutePath().normalize();
    }

    /** A place in the repository. */
    public static Path file(String first, String... more) {
        return root().resolve(Path.of(first, more));
    }

    /**
     * Every file of the repository's own whose name {@code named} takes, in order: what a build
     * or a tool writes — {@code target}, {@code vendor}, {@code node_modules}, and whatever is
     * hidden, {@code .git} and the worktrees in {@code .wt} among it — is never walked into, so a
     * file made or deleted there while a test reads is not read, and a build's output is not
     * walked at all.
     */
    public static List<Path> ownFiles(Predicate<String> named) {
        Path root = root();
        List<Path> found = new ArrayList<>();
        try {
            Files.walkFileTree(root, new SimpleFileVisitor<>() {
                @Override
                public FileVisitResult preVisitDirectory(Path directory, BasicFileAttributes attributes) {
                    String name = directory.getFileName().toString();
                    boolean brought = !directory.equals(root) && (name.startsWith(".")
                            || name.equals("target") || name.equals("vendor")
                            || name.equals("node_modules"));
                    return brought ? FileVisitResult.SKIP_SUBTREE : FileVisitResult.CONTINUE;
                }

                @Override
                public FileVisitResult visitFile(Path file, BasicFileAttributes attributes) {
                    String name = file.getFileName().toString();
                    if (!name.startsWith(".") && named.test(name)) {
                        found.add(file);
                    }
                    return FileVisitResult.CONTINUE;
                }
            });
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        found.sort(null);
        return found;
    }

    /**
     * The test sources of every module the root pom lists, so that a rule about all the tests is
     * held to the tests of a module added later as it is to the first one's.
     */
    public static List<Path> testSources() {
        try {
            Matcher module = MODULE.matcher(Files.readString(root().resolve("pom.xml")));
            List<Path> sources = new ArrayList<>();
            while (module.find()) {
                Path tests = root().resolve(module.group(1)).resolve("src/test/java");
                if (Files.isDirectory(tests)) {
                    sources.add(tests);
                }
            }
            return sources;
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }
}
