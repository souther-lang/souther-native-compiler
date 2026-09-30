package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

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
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What a document says is there is there: every relative link it follows, every name of the
 * library's C surface it uses, and every type or member of the binding API or of the command.
 *
 * <p>A document explains and points; what a function takes, what a type holds and which exist are
 * stated where they are defined. So a document goes stale by naming what is no longer there, and
 * that is what this finds, over every Markdown file of the repository. What is there is decided by
 * what defines it now, and never by what a reference happens to find: a name that is gone has to
 * fail, and not drop out of what is checked.
 *
 * <p>So a C name is held to what the current generation records and what the header of a library
 * built now declares, and not to source text, which keeps the history of names long gone. And a
 * type of the API or of the command is named, once in each document that names it, as a link to its
 * source file: the link breaks when the type is renamed or removed, and a document naming such a
 * type without linking it anywhere is refused while the type is there to be linked.
 */
class EveryDocumentNamesWhatIsThereTest {

    private static final Path ROOT = Path.of("..").toAbsolutePath().normalize();

    private static final Pattern LINK = Pattern.compile("\\[([^\\]\\n]*)\\]\\(([^)\\s]+)\\)");
    private static final Pattern SPAN = Pattern.compile("`([^`\\n]+)`");
    private static final Pattern C_NAME = Pattern.compile("souther_[a-z0-9_]*[a-z0-9]");
    private static final Pattern JAVA_NAME = Pattern.compile(
            "([A-Z][A-Za-z0-9]*)((?:\\.[A-Za-z][A-Za-z0-9_]*)*)(?:#([a-z][A-Za-z0-9]*))?(?:\\(.*\\))?");

    /** Where the types a document links to are defined: the binding API and the command. */
    private static final List<Path> SOURCES = List.of(
            ROOT.resolve("bindings/api/src/main/java/souther/bindings"),
            ROOT.resolve("compiler/src/main/java/souther/nativecode"));

    @Test
    void everyRelativeLinkReachesAFile() throws IOException {
        List<String> broken = new ArrayList<>();
        for (Path document : documents()) {
            for (Link link : links(document)) {
                if (!link.external() && !Files.exists(link.reaches())) {
                    broken.add(ROOT.relativize(document) + " -> " + link.target());
                }
            }
        }
        assertThat(broken).as("links to nothing").isEmpty();
    }

    @Test
    void everyCNameIsOneALibraryOfThisGenerationHas(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(Checked.of(List.of("""
                module m exposing ( one )

                let one: Int = 1
                """)), into.resolve("native"));
        Set<String> there = new HashSet<>();
        Matcher each = C_NAME.matcher(read(currentRecord())
                + read(library.declarations())
                + read(library.declarations().resolveSibling("souther.h")));
        while (each.find()) {
            there.add(each.group());
        }
        assertThat(there).as("what a library of this generation has")
                .contains("souther_abi_generation", "souther_scope_open", "souther_capability",
                        "souther_decimal");
        List<String> unknown = new ArrayList<>();
        for (Path document : documents()) {
            for (String span : spans(document)) {
                if (C_NAME.matcher(span).matches() && !there.contains(span)) {
                    unknown.add(ROOT.relativize(document) + ": " + span);
                }
            }
        }
        assertThat(unknown).as("C names no library of this generation has").isEmpty();
    }

    @Test
    void aTypeOfTheApiOrTheCommandIsLinkedToWhereItIsAndItsMembersAreThere() throws Exception {
        Set<String> types = sourceTypes();
        List<String> wrong = new ArrayList<>();
        int linked = 0;
        for (Path document : documents()) {
            String where = ROOT.relativize(document).toString();
            Set<String> linkedHere = new HashSet<>();
            for (Link link : links(document)) {
                Matcher name = JAVA_NAME.matcher(link.span());
                Class<?> defined = link.external() ? null : sourceClass(link.reaches());
                if (defined == null || !name.matches()) {
                    continue;
                }
                linked++;
                if (!name.group(1).equals(defined.getSimpleName())) {
                    wrong.add(where + ": `" + link.span() + "` links to " + link.target());
                } else if (!resolves(defined, name.group(2), name.group(3))) {
                    wrong.add(where + ": `" + link.span() + "` names nothing in " + link.target());
                } else {
                    linkedHere.add(name.group(1));
                }
            }
            for (String span : spans(document)) {
                Matcher name = JAVA_NAME.matcher(span);
                if (!name.matches() || !types.contains(name.group(1))) {
                    continue;
                }
                if (!linkedHere.contains(name.group(1))) {
                    wrong.add(where + ": `" + span + "` is not linked to its source anywhere here");
                } else if (!resolves(typeNamed(name.group(1)), name.group(2), name.group(3))) {
                    wrong.add(where + ": `" + span + "` names nothing there");
                }
            }
        }
        assertThat(linked).as("types the documents link to").isPositive();
        assertThat(wrong).as("types and members named as they are not").isEmpty();
    }

    /** A link, and the code span its text is where it is one. */
    private record Link(Path document, String text, String target) {

        boolean external() {
            return target.matches("[a-z]+:.*") || path().isEmpty();
        }

        String path() {
            return target.replaceAll("#.*$", "");
        }

        Path reaches() {
            return document.getParent().resolve(path()).normalize();
        }

        String span() {
            return text.startsWith("`") && text.endsWith("`") && text.length() > 2
                    ? text.substring(1, text.length() - 1).strip() : "";
        }
    }

    private static List<Link> links(Path document) throws IOException {
        List<Link> links = new ArrayList<>();
        Matcher link = LINK.matcher(read(document));
        while (link.find()) {
            links.add(new Link(document, link.group(1), link.group(2)));
        }
        return links;
    }

    /** The simple name of every top-level type the API and the command define, read off the sources. */
    private static Set<String> sourceTypes() throws IOException {
        Set<String> types = new TreeSet<>();
        for (Path source : SOURCES) {
            try (Stream<Path> files = Files.list(source)) {
                files.map(it -> it.getFileName().toString())
                        .filter(it -> it.endsWith(".java") && !it.equals("package-info.java"))
                        .forEach(it -> types.add(it.substring(0, it.length() - ".java".length())));
            }
        }
        assertThat(types).contains("BindingGenerator", "Manifest", "Bindings");
        return types;
    }

    /** The class {@code file} defines, where it is a source of the API or of the command. */
    private static Class<?> sourceClass(Path file) throws ClassNotFoundException {
        for (Path source : SOURCES) {
            // A link to a file that is not there is the link check's to report.
            if (file.getParent() != null && file.getParent().equals(source)
                    && file.getFileName().toString().endsWith(".java") && Files.exists(file)) {
                String name = file.getFileName().toString().replace(".java", "");
                return Class.forName(pkg(source) + "." + name, false,
                        EveryDocumentNamesWhatIsThereTest.class.getClassLoader());
            }
        }
        return null;
    }

    private static Class<?> typeNamed(String name) throws ClassNotFoundException {
        for (Path source : SOURCES) {
            if (Files.exists(source.resolve(name + ".java"))) {
                return Class.forName(pkg(source) + "." + name, false,
                        EveryDocumentNamesWhatIsThereTest.class.getClassLoader());
            }
        }
        throw new ClassNotFoundException(name);
    }

    private static String pkg(Path source) {
        return source.getParent().getFileName() + "." + source.getFileName();
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
