package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.RecordComponent;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a document says is there is there: every relative link it follows, every function of the
 * runtime it names, and every type or member of the binding API or of the command it names.
 *
 * <p>A document explains and points; what a function takes, what a type holds and which exist are
 * stated where they are defined. So a document goes stale by naming what is no longer there, and
 * that is what this finds, over every Markdown file of the repository. A name is checked where it
 * is a code span that is exactly that name: {@code `souther_scope_open`}, {@code `BindingInput`},
 * {@code `Bindings.Generator`}, {@code `Declarations.copyTo`}. Anything else in a code span is
 * prose about code and not read.
 */
class EveryDocumentNamesWhatIsThereTest {

    private static final Path ROOT = Path.of("..").toAbsolutePath().normalize();

    private static final Pattern LINK = Pattern.compile("\\]\\(([^)\\s]+)\\)");
    private static final Pattern SPAN = Pattern.compile("`([^`\\n]+)`");
    private static final Pattern RUNTIME_NAME = Pattern.compile("souther_[a-z0-9_]*[a-z0-9]");
    private static final Pattern JAVA_NAME = Pattern.compile(
            "([A-Z][A-Za-z0-9]*)((?:\\.[A-Za-z][A-Za-z0-9_]*)*)(?:#([a-z][A-Za-z0-9]*))?(?:\\(.*\\))?");
    private static final List<String> PACKAGES = List.of("souther.bindings", "souther.nativecode");

    @Test
    void everyRelativeLinkReachesAFile() throws IOException {
        List<String> broken = new ArrayList<>();
        for (Path document : documents()) {
            Matcher link = LINK.matcher(read(document));
            while (link.find()) {
                String target = link.group(1).replaceAll("#.*$", "");
                if (target.isEmpty() || target.matches("[a-z]+:.*")) {
                    continue;
                }
                if (!Files.exists(document.getParent().resolve(target))) {
                    broken.add(ROOT.relativize(document) + " -> " + link.group(1));
                }
            }
        }
        assertThat(broken).as("links to nothing").isEmpty();
    }

    @Test
    void everyRuntimeNameIsOneTheAbiSpells() throws IOException {
        Set<String> spelt = new HashSet<>();
        // What the abi crate spells, what the header is written as, and what the generation records.
        Matcher each = RUNTIME_NAME.matcher(read(ROOT.resolve("native/crates/abi/src/lib.rs"))
                + read(ROOT.resolve("native/crates/compiler/src/interface.rs"))
                + read(currentRecord()));
        while (each.find()) {
            spelt.add(each.group());
        }
        List<String> unknown = new ArrayList<>();
        for (Path document : documents()) {
            for (String span : spans(document)) {
                if (RUNTIME_NAME.matcher(span).matches() && !spelt.contains(span)) {
                    unknown.add(ROOT.relativize(document) + ": " + span);
                }
            }
        }
        assertThat(unknown).as("runtime names the abi crate does not spell").isEmpty();
    }

    @Test
    void everyTypeOrMemberOfTheApiOrTheCommandNamedIsThere() throws IOException {
        List<String> unknown = new ArrayList<>();
        int checked = 0;
        for (Path document : documents()) {
            for (String span : spans(document)) {
                Matcher name = JAVA_NAME.matcher(span);
                if (!name.matches()) {
                    continue;
                }
                Class<?> type = topLevel(name.group(1));
                if (type == null) {
                    continue;
                }
                checked++;
                if (!resolves(type, name.group(2), name.group(3))) {
                    unknown.add(ROOT.relativize(document) + ": " + span);
                }
            }
        }
        assertThat(checked).as("names of the API and the command the documents use").isPositive();
        assertThat(unknown).as("types and members that are not there").isEmpty();
    }

    /** Whether {@code path}, dotted segments after {@code type}, and then {@code member}, name something. */
    private static boolean resolves(Class<?> type, String path, String member) {
        Class<?> at = type;
        List<String> segments = path.isEmpty() ? List.of()
                : List.of(path.substring(1).split("\\."));
        for (int i = 0; i < segments.size(); i++) {
            String segment = segments.get(i);
            Class<?> nested = nested(at, segment);
            if (nested != null) {
                at = nested;
            } else if (i == segments.size() - 1 && member == null) {
                return hasMember(at, segment);
            } else {
                return false;
            }
        }
        return member == null || hasMember(at, member);
    }

    private static Class<?> nested(Class<?> type, String name) {
        for (Class<?> each : type.getDeclaredClasses()) {
            if (each.getSimpleName().equals(name)) {
                return each;
            }
        }
        return null;
    }

    private static boolean hasMember(Class<?> type, String name) {
        for (Method method : type.getDeclaredMethods()) {
            if (method.getName().equals(name)) {
                return true;
            }
        }
        for (Method method : type.getMethods()) {
            if (method.getName().equals(name)) {
                return true;
            }
        }
        for (Field field : type.getDeclaredFields()) {
            if (field.getName().equals(name)) {
                return true;
            }
        }
        if (type.isRecord()) {
            for (RecordComponent component : type.getRecordComponents()) {
                if (component.getName().equals(name)) {
                    return true;
                }
            }
        }
        return false;
    }

    private static Class<?> topLevel(String name) {
        for (String pkg : PACKAGES) {
            try {
                return Class.forName(pkg + "." + name, false,
                        EveryDocumentNamesWhatIsThereTest.class.getClassLoader());
            } catch (ClassNotFoundException e) {
                // not in this package
            }
        }
        return null;
    }

    private static List<String> spans(Path document) throws IOException {
        List<String> spans = new ArrayList<>();
        Matcher span = SPAN.matcher(read(document));
        while (span.find()) {
            spans.add(span.group(1).strip());
        }
        return spans;
    }

    /** Every Markdown file of the repository that is its own, and none a build or a tool brought. */
    private static List<Path> documents() throws IOException {
        try (Stream<Path> all = Files.walk(ROOT)) {
            List<Path> documents = all
                    .filter(it -> it.toString().endsWith(".md"))
                    .filter(it -> ROOT.relativize(it).toString().split("[/\\\\]").length > 0)
                    .filter(it -> {
                        for (Path part : ROOT.relativize(it)) {
                            String name = part.toString();
                            if (name.startsWith(".") || name.equals("target") || name.equals("vendor")
                                    || name.equals("node_modules")) {
                                return false;
                            }
                        }
                        return true;
                    })
                    .sorted()
                    .toList();
            assertThat(documents).contains(ROOT.resolve("README.md"),
                    ROOT.resolve("docs/host-abi.md"), ROOT.resolve("docs/writing-a-binding.md"));
            return documents;
        }
    }

    /** The record of the newest generation, which is what the runtime is now. */
    private static Path currentRecord() throws IOException {
        try (Stream<Path> records = Files.list(ROOT.resolve("native/crates/abi/generations"))) {
            return records.filter(it -> it.getFileName().toString().matches("\\d+\\.txt"))
                    .max(Comparator.comparingInt(it ->
                            Integer.parseInt(it.getFileName().toString().replace(".txt", ""))))
                    .orElseThrow();
        }
    }

    private static String read(Path file) throws IOException {
        return Files.readString(file, StandardCharsets.UTF_8);
    }
}
