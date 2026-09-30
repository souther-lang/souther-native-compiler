package souther.nativecode;

import souther.bindings.NotBindable;
import tools.jackson.core.JacksonException;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;
import tools.jackson.databind.node.ObjectNode;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.regex.Pattern;
import java.util.stream.Stream;

/**
 * A directory a binding is written to, which is what one generation wrote and nothing else.
 *
 * <p>A generator writes into an empty directory beside where the binding goes ({@link #staging}),
 * and the command puts it in place whole ({@link #commit}), so that the directory is only ever the
 * binding of one manifest: never a binding with a class the model no longer declares left from
 * the one before, and never half of one where a later name was refused. What it replaces has to be
 * a binding of the same generator, which the one file {@value #MARK} says; a directory holding
 * anything else, a binding of another host among it, is refused rather than deleted.
 *
 * <p>The mark is a versioned JSON document. Its {@code "format"}, {@code "version"} and
 * {@code "generator"} mean the same in every version, and are all that decides whether a directory
 * may be replaced ({@link #owner}), so that a command reading a mark a later one wrote still knows
 * whose it is; the rest of a mark of this version is read strictly ({@link #read}). It names the
 * generator by the id its jar
 * says, which is what owns the directory, so a generator moving to a newer version replaces what an
 * older one wrote. Beside it the mark records the artifact that wrote it: the coordinate and the
 * SHA-256 of a jar fetched from a repository, and only the SHA-256 of a jar on this machine, whose
 * path is where one run happened to find it and would carry a user's directories into the output.
 * The artifact is a record of what ran, and decides nothing.
 */
public final class BindingDirectory {

    /** The file that says a directory is a binding, and which generator wrote it. */
    public static final String MARK = ".souther-binding";

    private static final String FORMAT = "souther-binding";
    private static final int VERSION = 1;
    private static final Pattern SHA256 = Pattern.compile("[0-9a-f]{64}");

    /** A generator's id, of either namespace ({@link IdRule}). */
    private static final Pattern GENERATOR = Pattern.compile(IdRule.RESERVED.pattern() + "|"
            + IdRule.QUALIFIED.pattern());
    private static final JsonMapper JSON = JsonMapper.builder().build();

    /** The jar a binding was written by, as its mark records it. */
    public sealed interface Artifact {

        /** A jar fetched from a Maven repository: where from, and which bytes. */
        record Maven(String coordinate, String sha256) implements Artifact {
        }

        /** A jar read from this machine: which bytes, and not where it was. */
        record Local(String sha256) implements Artifact {
        }
    }

    /** What a mark says: which generator wrote the directory, and with which jar. */
    public record Mark(String generator, Artifact artifact) {
    }

    private final Path target;
    private final Path staging;
    private final Mark mark;

    private BindingDirectory(Path target, Path staging, Mark mark) {
        this.target = target;
        this.staging = staging;
        this.mark = mark;
    }

    /**
     * {@code target} as it is replaced, where it is absent, empty, or a binding {@code generator}
     * wrote.
     *
     * @throws NotBindable where {@code target} holds anything else
     */
    public static Path replaceable(Path target, String generator) throws IOException {
        Path absolute = target.toAbsolutePath().normalize();
        if (!Files.exists(absolute) || empty(absolute)) {
            return absolute;
        }
        String owner = owner(absolute);
        if (owner == null) {
            throw new NotBindable(absolute + " holds files a binding did not write,"
                    + " and a binding replaces the directory it is written to whole");
        }
        if (!owner.equals(generator)) {
            throw new NotBindable(absolute + " holds the binding the generator " + owner + " wrote,"
                    + " which " + generator + " does not replace");
        }
        return absolute;
    }

    /**
     * An empty directory beside {@code target} for the generator {@code mark} names to write the
     * binding into.
     *
     * @throws NotBindable where {@code target} holds what no generation of that generator wrote
     */
    public static BindingDirectory staging(Path target, Mark mark) throws IOException {
        Path absolute = replaceable(target, mark.generator());
        Path parent = absolute.getParent();
        Files.createDirectories(parent);
        Path staging = Files.createTempDirectory(parent, absolute.getFileName() + ".writing-");
        return new BindingDirectory(absolute, staging, mark);
    }

    /** What a generator writes into, and how, for {@link #written}. */
    @FunctionalInterface
    public interface Writing {
        void into(Path directory) throws IOException;
    }

    /**
     * {@code target} as {@code writing} wrote it, marked with {@code mark} and put in place whole, or
     * as it was where writing threw: what the command does with one binding, for a test that writes
     * one the same way.
     */
    public static Path written(Path target, Mark mark, Writing writing) throws IOException {
        BindingDirectory directory = staging(target, mark);
        try (Holding held = new Holding()) {
            held.hold(directory, directory::abandon);
            writing.into(directory.staging());
            directory.commit();
            return directory.target();
        }
    }

    /** Where the binding is written until it is put in place. */
    public Path staging() {
        return staging;
    }

    /** Where the binding goes. */
    public Path target() {
        return target;
    }

    /** Marks what was written as the generator's binding, and puts it in place of what was there. */
    public void commit() throws IOException {
        Files.writeString(staging.resolve(MARK), written(mark), StandardCharsets.UTF_8);
        if (!Files.exists(target)) {
            Files.move(staging, target, StandardCopyOption.ATOMIC_MOVE);
            return;
        }
        Path former = Files.createTempDirectory(target.getParent(),
                target.getFileName() + ".former-");
        Files.delete(former);
        Files.move(target, former, StandardCopyOption.ATOMIC_MOVE);
        try {
            Files.move(staging, target, StandardCopyOption.ATOMIC_MOVE);
        } catch (IOException e) {
            Files.move(former, target, StandardCopyOption.ATOMIC_MOVE);
            throw e;
        }
        remove(former);
    }

    /**
     * Drops what was written, where it was not put in place; after {@link #commit} there is nothing
     * left to drop, and this does nothing.
     */
    public void abandon() throws IOException {
        remove(staging);
    }

    private static boolean empty(Path directory) throws IOException {
        if (!Files.isDirectory(directory)) {
            return false;
        }
        try (Stream<Path> entries = Files.list(directory)) {
            return entries.findAny().isEmpty();
        }
    }

    /**
     * The generator that owns {@code directory}, as its mark says in any version of the mark; null
     * where it is not a directory, or holds no mark this format has ever written.
     */
    private static String owner(Path directory) throws IOException {
        Path mark = directory.resolve(MARK);
        if (!Files.isDirectory(directory) || !Files.isRegularFile(mark)) {
            return null;
        }
        return owner(Files.readString(mark, StandardCharsets.UTF_8));
    }

    /**
     * Who owns the directory a mark stands in, read from what every version of the mark says alike:
     * {@code "format"}, a {@code "version"} of 1 or later, and {@code "generator"}. Those members mean
     * the same in every version, so a directory a later command marked is owned by the same generator
     * here, and what else a later version says is not read. Null where it is not such a mark.
     */
    static String owner(String text) {
        JsonNode said;
        try {
            said = JSON.readTree(text);
        } catch (JacksonException e) {
            return null;
        }
        if (!said.isObject() || !said.path("format").isString()
                || !FORMAT.equals(said.get("format").asString()) || !said.path("version").isInt()
                || said.get("version").asInt() < 1 || !said.path("generator").isString()
                || !GENERATOR.matcher(said.get("generator").asString()).matches()) {
            return null;
        }
        if (said.get("version").asInt() == VERSION && read(text) == null) {
            // A mark of this very version is read whole, and one that is not what it says is not one.
            return null;
        }
        return said.get("generator").asString();
    }

    /** {@code mark} as the file says it. */
    static String written(Mark mark) {
        ObjectNode written = JSON.createObjectNode();
        written.put("format", FORMAT);
        written.put("version", VERSION);
        written.put("generator", mark.generator());
        ObjectNode artifact = written.putObject("artifact");
        switch (mark.artifact()) {
            case Artifact.Maven maven -> {
                artifact.put("kind", "maven");
                artifact.put("coordinate", maven.coordinate());
                artifact.put("sha256", maven.sha256());
            }
            case Artifact.Local local -> {
                artifact.put("kind", "local");
                artifact.put("sha256", local.sha256());
            }
        }
        return JSON.writerWithDefaultPrettyPrinter().writeValueAsString(written) + "\n";
    }

    /**
     * What {@code text} says, or null where it is not a mark of this format and version: a member
     * missing, one more than it has, or one of another type is not read as what it might have meant.
     */
    static Mark read(String text) {
        JsonNode said;
        try {
            said = JSON.readTree(text);
        } catch (JacksonException e) {
            return null;
        }
        if (!exactly(said, "format", "version", "generator", "artifact")
                || !said.get("format").isString() || !FORMAT.equals(said.get("format").asString())
                || !said.get("version").isInt() || said.get("version").asInt() != VERSION
                || !said.get("generator").isString() || said.get("generator").asString().isEmpty()) {
            return null;
        }
        JsonNode artifact = said.get("artifact");
        if (!artifact.isObject() || !artifact.path("kind").isString()) {
            return null;
        }
        Artifact read = switch (artifact.get("kind").asString()) {
            case "maven" -> exactly(artifact, "kind", "coordinate", "sha256")
                    && artifact.get("coordinate").isString() && sha256(artifact.get("sha256"))
                    ? new Artifact.Maven(artifact.get("coordinate").asString(),
                    artifact.get("sha256").asString()) : null;
            case "local" -> exactly(artifact, "kind", "sha256") && sha256(artifact.get("sha256"))
                    ? new Artifact.Local(artifact.get("sha256").asString()) : null;
            default -> null;
        };
        return read == null ? null : new Mark(said.get("generator").asString(), read);
    }

    private static boolean exactly(JsonNode node, String... names) {
        if (!node.isObject()) {
            return false;
        }
        Set<String> held = new HashSet<>(node.propertyNames());
        return held.equals(Set.of(names));
    }

    private static boolean sha256(JsonNode node) {
        return node.isString() && SHA256.matcher(node.asString()).matches();
    }

    private static void remove(Path directory) throws IOException {
        if (!Files.exists(directory)) {
            return;
        }
        try (Stream<Path> walked = Files.walk(directory)) {
            for (Path each : walked.sorted(Comparator.reverseOrder()).toList()) {
                Files.delete(each);
            }
        }
    }
}
